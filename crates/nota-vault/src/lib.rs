//! El vault en disco: abrir una carpeta, sus índices de archivos, links y
//! tags al día con lo que pasa en disco, crear, mover y borrar, y el
//! guardado atómico de cada nota.

mod file;
mod index;
mod links;
mod ops;
mod order;
mod path;
mod refactor;
mod refresh;
mod scan;
pub mod tags;
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
pub use refactor::{AccessError, AppliedRename, ApplyRenameError, Interruption, UndoReport};
pub use refresh::VaultChange;
pub use scan::walk_notes;
pub use vault::{Vault, VaultError, VaultOptions, VaultUpdate};
