//! El vault en disco: abrir una carpeta, su índice de archivos al día con lo
//! que pasa en disco, los links entre notas, crear, mover y borrar, y el
//! guardado atómico de cada nota.

mod file;
mod index;
mod links;
mod ops;
mod order;
mod path;
mod refresh;
mod scan;
mod vault;
mod watch;

pub use file::{FileError, read, write_atomic};
pub use index::{Entry, EntryKind, VaultIndex};
pub use links::{
    Destination, FileEdits, IndexedLink, LinkContext, LinkIndex, LinkSubpath, LinkTarget, NoteLink,
    RenameConflict, RenamePlan, RenamePlanError, StaleEdit, TextEdit,
};
pub use ops::FileOpError;
pub use path::{InvalidPath, InvalidReason, VaultPath};
pub use refresh::VaultChange;
pub use scan::walk_notes;
pub use vault::{Vault, VaultError, VaultUpdate};
