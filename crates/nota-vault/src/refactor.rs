//! Aplicar un [`RenamePlan`] al disco y deshacerlo: reescribir los links de
//! cada archivo y renombrar, como una sola operación (05 §5). Cada archivo
//! se escribe con [`write_atomic`]; el conjunto no es atómico, así que un
//! fallo a medias dice qué quedó hecho para poder deshacerlo.

use std::fs;
use std::path::Path;

use crate::file::{self, FileError, write_atomic};
use crate::links::{RenamePlan, StaleEdit};
use crate::ops::{self, FileOpError};
use crate::path::VaultPath;

/// Un plan ya aplicado, con lo que hace falta para deshacerlo: el contenido
/// de antes y el escrito de cada archivo tocado.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppliedRename {
    pub from: VaultPath,
    pub to: VaultPath,
    /// `false` si se paró antes de renombrar.
    pub renamed: bool,
    files: Vec<Rewritten>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Rewritten {
    /// La ruta de antes del rename.
    path: VaultPath,
    before: String,
    after: String,
}

impl AppliedRename {
    /// Los archivos reescritos, en sus rutas de antes del rename.
    pub fn files(&self) -> impl Iterator<Item = &VaultPath> {
        self.files.iter().map(|file| &file.path)
    }
}

/// Por qué no se aplicó un plan.
#[derive(Debug, thiserror::Error)]
pub enum ApplyRenameError {
    /// Un archivo cambió desde que se indexó. No se escribió nada.
    #[error(transparent)]
    Stale(#[from] StaleEdit),
    /// No se pudo leer un archivo del plan, ya no existe o el destino está
    /// ocupado. No se escribió nada.
    #[error(transparent)]
    Access(#[from] AccessError),
    /// Se paró a medias; `applied` dice qué quedó hecho y sirve para deshacerlo.
    #[error("se paró a medias: {cause}")]
    Interrupted {
        applied: Box<AppliedRename>,
        cause: Interruption,
    },
}

#[derive(Debug, thiserror::Error)]
pub enum AccessError {
    #[error(transparent)]
    Path(#[from] FileOpError),
    #[error(transparent)]
    File(#[from] FileError),
}

#[derive(Debug, thiserror::Error)]
pub enum Interruption {
    /// Otro programa lo guardó después de comprobar el plan; no se tocó.
    #[error("{0} cambió mientras se aplicaba el renombre")]
    Changed(VaultPath),
    #[error(transparent)]
    Access(AccessError),
    #[error(transparent)]
    Write(FileError),
    #[error(transparent)]
    Rename(FileOpError),
}

/// Lo que hizo [`crate::Vault::undo_rename`], con las rutas de antes del
/// rename: los archivos que cambiaron después de aplicar el plan no se
/// tocan, y los que no se pudieron leer o escribir quedan listados.
#[derive(Debug, Default)]
pub struct UndoReport {
    pub restored: Vec<VaultPath>,
    pub changed: Vec<VaultPath>,
    pub failed: Vec<(VaultPath, AccessError)>,
    /// Si no se pudo deshacer el rename tras devolver los textos.
    pub rename: Option<FileOpError>,
}

/// Comprueba todo el plan contra el disco, escribe cada archivo y renombra.
pub(crate) fn apply(root: &Path, plan: &RenamePlan) -> Result<AppliedRename, ApplyRenameError> {
    let mut files = Vec::with_capacity(plan.files.len());
    for edits in &plan.files {
        let before = read(root, &edits.path)?;
        let (after, _) = edits.apply(&before)?;
        files.push(Rewritten {
            path: edits.path.clone(),
            before,
            after,
        });
    }
    // El rename lo vuelve a comprobar sin carrera; esto evita escribir para nada.
    ops::check_free(root, &plan.to).map_err(AccessError::from)?;
    let mut applied = AppliedRename {
        from: plan.from.clone(),
        to: plan.to.clone(),
        renamed: false,
        files: Vec::with_capacity(files.len()),
    };
    for file in files {
        // Sin locks de archivo (06 §2), comprobar justo antes de escribir es
        // lo que deja más chica la ventana para un guardado de otro programa.
        match read(root, &file.path) {
            Ok(text) if text == file.before => {}
            Ok(_) => return interrupted(applied, Interruption::Changed(file.path)),
            Err(error) => return interrupted(applied, Interruption::Access(error)),
        }
        if let Err(error) = write(root, &file.path, &file.after) {
            // `write_atomic` puede fallar ya reemplazado el archivo, al bajar la carpeta a disco.
            if read(root, &file.path).is_ok_and(|text| text == file.after) {
                applied.files.push(file);
            }
            return interrupted(applied, Interruption::Write(error));
        }
        applied.files.push(file);
    }
    if let Err(error) = ops::rename(root, &plan.from, &plan.to) {
        return interrupted(applied, Interruption::Rename(error));
    }
    applied.renamed = true;
    Ok(applied)
}

fn interrupted(
    applied: AppliedRename,
    cause: Interruption,
) -> Result<AppliedRename, ApplyRenameError> {
    Err(ApplyRenameError::Interrupted {
        applied: Box::new(applied),
        cause,
    })
}

/// Vuelve cada archivo a como estaba, salvo los que cambiaron desde que se
/// aplicó, y deshace el rename. Como al aplicar, los textos van antes que
/// el rename: guardar encima del destino de un rename en el mismo lote del
/// watcher le hace perder el origen a `notify-debouncer-full` 0.7 (#54).
pub(crate) fn undo(root: &Path, applied: &AppliedRename) -> Result<UndoReport, FileOpError> {
    if applied.renamed {
        ops::check_free(root, &applied.from)?;
    }
    let mut report = UndoReport::default();
    for file in &applied.files {
        let now = if applied.renamed {
            file.path
                .rebase(&applied.from, &applied.to)
                .unwrap_or_else(|| file.path.clone())
        } else {
            file.path.clone()
        };
        let path = file.path.clone();
        match read(root, &now) {
            Ok(text) if text == file.after => match write(root, &now, &file.before) {
                Ok(()) => report.restored.push(path),
                Err(error) => report.failed.push((path, error.into())),
            },
            Ok(_) | Err(AccessError::Path(FileOpError::NotFound(_))) => report.changed.push(path),
            Err(error) => report.failed.push((path, error)),
        }
    }
    if applied.renamed {
        report.rename = ops::rename(root, &applied.to, &applied.from).err();
    }
    Ok(report)
}

/// El texto de un archivo que existe y al que no se llega por un enlace.
fn read(root: &Path, path: &VaultPath) -> Result<String, AccessError> {
    let full = ops::existing(root, path)?;
    if fs::symlink_metadata(&full).is_ok_and(|metadata| metadata.file_type().is_symlink()) {
        return Err(FileOpError::Symlink(path.clone()).into());
    }
    file::read(&full)?.ok_or_else(|| FileOpError::NotFound(path.clone()).into())
}

fn write(root: &Path, path: &VaultPath, text: &str) -> Result<(), FileError> {
    write_atomic(&path.to_fs(root), |out| out.write_all(text.as_bytes()))
}
