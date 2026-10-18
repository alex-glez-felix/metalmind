//! Las notas en disco: abrir un vault, su índice de archivos al día con lo
//! que pasa en disco y el guardado atómico de cada nota.

mod file;
mod index;
mod ops;
mod path;
mod refresh;
mod scan;
mod vault;
mod watch;

pub use file::{FileError, read, write_atomic};
pub use index::{Entry, EntryKind, VaultIndex};
pub use ops::FileOpError;
pub use path::{InvalidPath, InvalidReason, VaultPath};
pub use refresh::VaultChange;
pub use vault::{Vault, VaultError, VaultUpdate};
