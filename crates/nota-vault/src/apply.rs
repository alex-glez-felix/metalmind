//! Aplicar unos [`Changes`] al disco y deshacerlos como una sola operación
//! (05 §5). Cada archivo se escribe de forma atómica; el conjunto no, así que
//! un fallo a medias dice qué quedó hecho para poder deshacerlo.

use std::fs;
use std::io;
use std::path::Path;

use crate::changes::{Changes, NewFile, Rename};
use crate::edit::StaleEdit;
use crate::file::{self, FileError, create_atomic, replace_file};
use crate::ops::{self, FileOpError};
use crate::path::VaultPath;

/// Unos cambios ya aplicados, con lo que hace falta para deshacerlos.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Applied {
    files: Vec<Rewritten>,
    created: Vec<Created>,
    renamed: Vec<Renamed>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Renamed {
    rename: Rename,
    /// El texto con que se movió, si es un archivo: deshacer no lo devuelve
    /// a su lugar si cambió después. Una carpeta se mueve de vuelta entera.
    text: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Rewritten {
    /// La ruta de antes de los renombres.
    path: VaultPath,
    before: String,
    after: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Created {
    file: NewFile,
    /// Las carpetas que hubo que crear para él, de la raíz hacia adentro.
    folders: Vec<VaultPath>,
}

impl Applied {
    /// Los archivos reescritos, con sus rutas de antes de renombrar.
    pub fn rewritten(&self) -> impl Iterator<Item = &VaultPath> {
        self.files.iter().map(|file| &file.path)
    }

    pub fn created(&self) -> impl Iterator<Item = &VaultPath> {
        self.created.iter().map(|created| &created.file.path)
    }

    pub fn renamed(&self) -> impl Iterator<Item = &Rename> {
        self.renamed.iter().map(|renamed| &renamed.rename)
    }

    /// Dónde está ahora lo que estaba en `path` antes de aplicar.
    fn now(&self, path: &VaultPath) -> VaultPath {
        self.renamed().fold(path.clone(), |path, rename| {
            path.rebase(&rename.from, &rename.to).unwrap_or(path)
        })
    }
}

/// Por qué no se aplicaron unos cambios.
#[derive(Debug, thiserror::Error)]
pub enum ApplyError {
    /// Un archivo ya no dice lo que una edición espera: cambió desde que se
    /// indexó. No se escribió nada.
    #[error(transparent)]
    Stale(#[from] StaleEdit),
    /// No se pudo leer un archivo, ya no existe o un destino está ocupado.
    /// No se escribió nada.
    #[error(transparent)]
    Access(#[from] AccessError),
    /// Se paró a medias; `applied` dice qué quedó hecho y sirve para deshacerlo.
    #[error("se paró a medias: {cause}")]
    Interrupted {
        applied: Box<Applied>,
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
    /// Otro programa lo guardó después de comprobar los cambios; no se tocó.
    #[error("{0} cambió mientras se aplicaban los cambios")]
    Changed(VaultPath),
    #[error(transparent)]
    Access(AccessError),
    #[error(transparent)]
    Write(FileError),
    #[error(transparent)]
    Create(AccessError),
    #[error(transparent)]
    Rename(FileOpError),
}

/// Lo que hizo [`crate::Vault::undo`], con las rutas de antes de aplicar:
/// lo que cambió después no se toca, y lo que no se pudo leer, escribir o
/// borrar queda listado.
#[derive(Debug, Default)]
pub struct UndoReport {
    pub restored: Vec<VaultPath>,
    /// Archivos reescritos o creados que cambiaron después de aplicar.
    pub changed: Vec<VaultPath>,
    pub failed: Vec<(VaultPath, AccessError)>,
    /// Los renombres que no se pudieron deshacer.
    pub renames: Vec<FileOpError>,
}

/// Comprueba todos los cambios contra el disco y los aplica en orden:
/// ediciones, archivos nuevos y renombres.
pub(crate) fn apply(root: &Path, changes: &Changes) -> Result<Applied, ApplyError> {
    let mut files = Vec::with_capacity(changes.edits.len());
    for edit in &changes.edits {
        let before = read(root, &edit.path)?;
        let (after, _) = edit.apply(&before)?;
        files.push(Rewritten {
            path: edit.path.clone(),
            before,
            after,
        });
    }
    // Crear y renombrar lo vuelven a comprobar sin carrera; esto evita escribir para nada.
    for path in changes
        .creates
        .iter()
        .map(|file| &file.path)
        .chain(changes.renames.iter().map(|rename| &rename.to))
    {
        ops::check_free(root, path).map_err(AccessError::from)?;
    }
    let mut applied = Applied {
        files: Vec::with_capacity(files.len()),
        created: Vec::with_capacity(changes.creates.len()),
        renamed: Vec::with_capacity(changes.renames.len()),
    };
    for file in files {
        if let Err(cause) = rewrite(root, &file) {
            // Escribir puede fallar ya reemplazado el archivo, al bajar la carpeta a disco.
            if read(root, &file.path).is_ok_and(|text| text == file.after) {
                applied.files.push(file);
            }
            return interrupted(applied, cause);
        }
        applied.files.push(file);
    }
    for file in &changes.creates {
        match create(root, file) {
            Ok(created) => applied.created.push(created),
            Err(failed) => {
                applied.created.extend(failed.created);
                return interrupted(applied, Interruption::Create(failed.cause));
            }
        }
    }
    for rename in &changes.renames {
        let text = moved_text(root, &applied, &rename.from);
        if let Err(error) = ops::rename(root, &rename.from, &rename.to) {
            return interrupted(applied, Interruption::Rename(error));
        }
        applied.renamed.push(Renamed {
            rename: rename.clone(),
            text,
        });
    }
    Ok(applied)
}

/// El texto de `path` si es un archivo, tal como lo deja lo ya aplicado.
fn moved_text(root: &Path, applied: &Applied, path: &VaultPath) -> Option<String> {
    let current = applied.now(path);
    let rewritten = applied
        .files
        .iter()
        .find(|file| applied.now(&file.path) == current);
    match rewritten {
        Some(file) => Some(file.after.clone()),
        None => read(root, &current).ok(),
    }
}

/// Sin locks de archivo (06 §2), comprobar justo antes de escribir es lo que
/// deja más chica la ventana para un guardado de otro programa.
fn rewrite(root: &Path, file: &Rewritten) -> Result<(), Interruption> {
    match read(root, &file.path) {
        Ok(text) if text == file.before => {}
        Ok(_) => return Err(Interruption::Changed(file.path.clone())),
        Err(error) => return Err(Interruption::Access(error)),
    }
    write(root, &file.path, &file.after).map_err(Interruption::Write)
}

/// Crear falló; `created` está si el archivo igual quedó en disco (falló al
/// bajar la carpeta a disco), para poder deshacerlo.
struct CreateFailed {
    created: Option<Created>,
    cause: AccessError,
}

fn create(root: &Path, file: &NewFile) -> Result<Created, CreateFailed> {
    let failed = |cause: AccessError| CreateFailed {
        created: None,
        cause,
    };
    ops::check_free(root, &file.path).map_err(|error| failed(error.into()))?;
    let folders = ops::create_parents(root, &file.path).map_err(|error| failed(error.into()))?;
    let created = Created {
        file: file.clone(),
        folders,
    };
    match create_atomic(&file.path.to_fs(root), &file.text) {
        Ok(()) => Ok(created),
        Err(FileError::Write { source, .. }) if source.kind() == io::ErrorKind::AlreadyExists => {
            remove_folders(root, &created.folders);
            Err(failed(FileOpError::AlreadyExists(file.path.clone()).into()))
        }
        Err(error) if read(root, &file.path).is_ok_and(|text| text == file.text) => {
            Err(CreateFailed {
                created: Some(created),
                cause: error.into(),
            })
        }
        Err(error) => {
            remove_folders(root, &created.folders);
            Err(failed(error.into()))
        }
    }
}

fn interrupted(applied: Applied, cause: Interruption) -> Result<Applied, ApplyError> {
    Err(ApplyError::Interrupted {
        applied: Box::new(applied),
        cause,
    })
}

/// Devuelve cada archivo a como estaba, borra los creados y deshace los
/// renombres, salvo lo que cambió desde que se aplicó. Como al aplicar, los
/// textos van antes que los renombres: guardar encima del destino de un
/// rename en el mismo lote del watcher le hace perder el origen a
/// `notify-debouncer-full` 0.7 (#54).
pub(crate) fn undo(root: &Path, applied: &Applied) -> Result<UndoReport, FileOpError> {
    for rename in applied.renamed() {
        ops::check_free(root, &rename.from)?;
    }
    let mut report = UndoReport::default();
    // Antes de devolver los textos: un archivo movido que cambió se queda donde está.
    let moved_back: Vec<&Rename> = applied
        .renamed
        .iter()
        .filter(|renamed| {
            let unchanged = renamed
                .text
                .as_ref()
                .is_none_or(|text| read(root, &renamed.rename.to).is_ok_and(|now| &now == text));
            if !unchanged {
                report.changed.push(renamed.rename.from.clone());
            }
            unchanged
        })
        .map(|renamed| &renamed.rename)
        .collect();
    for file in &applied.files {
        let path = file.path.clone();
        match read(root, &applied.now(&file.path)) {
            Ok(text) if text == file.after => {
                match write(root, &applied.now(&path), &file.before) {
                    Ok(()) => report.restored.push(path),
                    Err(error) => report.failed.push((path, error.into())),
                }
            }
            Ok(_) | Err(AccessError::Path(FileOpError::NotFound(_))) => {
                if !report.changed.contains(&path) {
                    report.changed.push(path);
                }
            }
            Err(error) => report.failed.push((path, error)),
        }
    }
    for created in &applied.created {
        let path = created.file.path.clone();
        let now = applied.now(&path);
        match read(root, &now) {
            Ok(text) if text == created.file.text => match remove(root, &now) {
                Ok(()) => {
                    remove_folders(root, &created.folders);
                    report.restored.push(path);
                }
                Err(error) => report.failed.push((path, error.into())),
            },
            Ok(_) | Err(AccessError::Path(FileOpError::NotFound(_))) => report.changed.push(path),
            Err(error) => report.failed.push((path, error)),
        }
    }
    for rename in moved_back.into_iter().rev() {
        if let Err(error) = ops::rename(root, &rename.to, &rename.from) {
            report.renames.push(error);
        }
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

/// Sin seguir un enlace que haya aparecido después de leer el archivo.
fn write(root: &Path, path: &VaultPath, text: &str) -> Result<(), FileError> {
    replace_file(&path.to_fs(root), text)
}

fn remove(root: &Path, path: &VaultPath) -> Result<(), FileOpError> {
    let full = ops::existing(root, path)?;
    fs::remove_file(full).map_err(|source| FileOpError::Io {
        action: "borrar",
        path: path.clone(),
        source,
    })
}

/// Borra las carpetas que se crearon para un archivo, si quedaron vacías.
fn remove_folders(root: &Path, folders: &[VaultPath]) {
    for folder in folders.iter().rev() {
        if fs::remove_dir(folder.to_fs(root)).is_err() {
            return;
        }
    }
}
