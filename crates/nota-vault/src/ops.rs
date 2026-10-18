//! Crear, renombrar/mover y borrar desde el árbol. Nada toca el índice: el
//! watcher ve el cambio en disco y lo publica como cualquier otro.

use std::fs::{self, OpenOptions};
use std::io;
use std::path::{Path, PathBuf};

use rustix::fs::{CWD, RenameFlags, renameat_with};

use crate::index::EntryKind;
use crate::path::VaultPath;

/// La papelera del vault, la misma de Obsidian.
const TRASH: &str = ".trash";

#[derive(Debug, thiserror::Error)]
pub enum FileOpError {
    #[error("{0} no es una nota: tiene que terminar en .md")]
    NotANote(VaultPath),
    #[error("la raíz del vault no se puede mover ni borrar")]
    Root,
    #[error("{0} pasa por un enlace simbólico; podría salir del vault")]
    Symlink(VaultPath),
    #[error("la papelera {TRASH}/ del vault es un enlace simbólico")]
    TrashIsSymlink,
    #[error("{0} no se puede mover dentro de sí misma")]
    IntoItself(VaultPath),
    #[error("{0} es un archivo, no una carpeta")]
    NotAFolder(VaultPath),
    #[error("{0} ya existe")]
    AlreadyExists(VaultPath),
    #[error("{0} no existe")]
    NotFound(VaultPath),
    #[error("no se pudo {action} {path}: {source}")]
    Io {
        action: &'static str,
        path: VaultPath,
        source: io::Error,
    },
}

pub(crate) fn create_note(root: &Path, path: &VaultPath) -> Result<(), FileOpError> {
    if EntryKind::of_file(path) != EntryKind::Note {
        return Err(FileOpError::NotANote(path.clone()));
    }
    let target = prepare_target(root, path, "crear")?;
    OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(target)
        .map(drop)
        .map_err(|source| io_error("crear", path, source))
}

pub(crate) fn create_folder(root: &Path, path: &VaultPath) -> Result<(), FileOpError> {
    let target = prepare_target(root, path, "crear")?;
    fs::create_dir(target).map_err(|source| io_error("crear", path, source))
}

/// Mueve `from` a `to` sin pisar nada: si `to` ya existe, falla.
pub(crate) fn rename(root: &Path, from: &VaultPath, to: &VaultPath) -> Result<(), FileOpError> {
    if from.is_root() {
        return Err(FileOpError::Root);
    }
    if to.starts_with(from) && to != from {
        return Err(FileOpError::IntoItself(from.clone()));
    }
    let source = existing(root, from)?;
    let target = prepare_target(root, to, "mover")?;
    rename_no_replace(&source, &target).map_err(|error| match error.kind() {
        io::ErrorKind::AlreadyExists => FileOpError::AlreadyExists(to.clone()),
        io::ErrorKind::NotFound => FileOpError::NotFound(from.clone()),
        _ => io_error("mover", from, error),
    })
}

/// Manda `path` a `.trash/` en la raíz del vault. Si ya hay algo con ese
/// nombre, prueba `nombre 1.md`, `nombre 2.md`, … Devuelve dónde quedó.
pub(crate) fn trash(root: &Path, path: &VaultPath) -> Result<PathBuf, FileOpError> {
    let source = existing(root, path)?;
    let trash = trash_dir(root, path)?;
    for attempt in 0..u32::MAX {
        let target = trash.join(trash_name(path.name(), attempt));
        match rename_no_replace(&source, &target) {
            Ok(()) => return Ok(target),
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
            Err(error) => return Err(io_error("borrar", path, error)),
        }
    }
    Err(FileOpError::AlreadyExists(path.clone()))
}

/// `nota.md`, `nota 1.md`, `nota 2.md`, …
fn trash_name(name: &str, attempt: u32) -> String {
    if attempt == 0 {
        return name.to_owned();
    }
    match name.rsplit_once('.') {
        Some((stem, ext)) if !stem.is_empty() => format!("{stem} {attempt}.{ext}"),
        _ => format!("{name} {attempt}"),
    }
}

fn trash_dir(root: &Path, path: &VaultPath) -> Result<PathBuf, FileOpError> {
    let trash = root.join(TRASH);
    let failed = |source| FileOpError::Io {
        action: "crear la papelera para",
        path: path.clone(),
        source,
    };
    match fs::symlink_metadata(&trash) {
        Ok(metadata) if metadata.file_type().is_symlink() => Err(FileOpError::TrashIsSymlink),
        Ok(_) => Ok(trash),
        Err(error) if error.kind() == io::ErrorKind::NotFound => match fs::create_dir(&trash) {
            Ok(()) => Ok(trash),
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => Ok(trash),
            Err(error) => Err(failed(error)),
        },
        Err(error) => Err(failed(error)),
    }
}

/// La ruta en disco de algo que ya existe y se puede mover: no es la raíz
/// ni se llega a ello por un enlace.
pub(crate) fn existing(root: &Path, path: &VaultPath) -> Result<PathBuf, FileOpError> {
    if path.is_root() {
        return Err(FileOpError::Root);
    }
    check_ancestors(root, path)?;
    let full = path.to_fs(root);
    match fs::symlink_metadata(&full) {
        Ok(_) => Ok(full),
        Err(error) => Err(io_error("leer", path, error)),
    }
}

/// La ruta en disco donde crear `path`, con sus carpetas ya creadas.
fn prepare_target(
    root: &Path,
    path: &VaultPath,
    action: &'static str,
) -> Result<PathBuf, FileOpError> {
    if path.is_root() {
        return Err(FileOpError::AlreadyExists(path.clone()));
    }
    check_ancestors(root, path)?;
    let full = path.to_fs(root);
    if let Some(parent) = full.parent() {
        fs::create_dir_all(parent).map_err(|source| io_error(action, path, source))?;
    }
    Ok(full)
}

/// Ninguna carpeta entre la raíz y `path` puede ser un enlace simbólico (por
/// ahí una ruta válida del vault escribiría fuera de él) ni un archivo.
fn check_ancestors(root: &Path, path: &VaultPath) -> Result<(), FileOpError> {
    let mut ancestors: Vec<VaultPath> = path.ancestors().filter(|dir| !dir.is_root()).collect();
    ancestors.reverse();
    for dir in ancestors {
        match fs::symlink_metadata(dir.to_fs(root)) {
            Ok(metadata) if metadata.file_type().is_symlink() => {
                return Err(FileOpError::Symlink(path.clone()));
            }
            Ok(metadata) if !metadata.is_dir() => return Err(FileOpError::NotAFolder(dir)),
            Ok(_) => {}
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
            Err(error) => return Err(io_error("leer", &dir, error)),
        }
    }
    Ok(())
}

/// `rename(2)` pisa el destino en silencio; `RENAME_NOREPLACE` lo hace fallar
/// de forma atómica, sin la carrera de mirar antes si existe.
fn rename_no_replace(from: &Path, to: &Path) -> io::Result<()> {
    renameat_with(CWD, from, CWD, to, RenameFlags::NOREPLACE).map_err(io::Error::from)
}

fn io_error(action: &'static str, path: &VaultPath, source: io::Error) -> FileOpError {
    match source.kind() {
        io::ErrorKind::AlreadyExists => FileOpError::AlreadyExists(path.clone()),
        io::ErrorKind::NotFound => FileOpError::NotFound(path.clone()),
        _ => FileOpError::Io {
            action,
            path: path.clone(),
            source,
        },
    }
}
