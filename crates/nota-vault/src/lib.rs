//! Las notas en disco: abrir un vault, su índice de archivos y el guardado
//! atómico de cada nota.

mod file;
mod index;
mod path;
mod scan;
mod vault;

pub use file::{FileError, read, write_atomic};
pub use index::{Entry, EntryKind, VaultIndex};
pub use path::{InvalidPath, InvalidReason, VaultPath};
pub use vault::{Vault, VaultError};
