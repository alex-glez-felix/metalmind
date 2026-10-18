//! El vault en disco: abrir una carpeta, sus índices de archivos, links y
//! tags al día con lo que pasa en disco, crear, mover y borrar, el guardado
//! atómico de cada nota y aplicar y deshacer cambios en varios archivos.

mod apply;
mod changes;
mod edit;
mod file;
mod index;
mod links;
mod ops;
mod order;
mod path;
mod refresh;
mod scan;
pub mod tags;
mod vault;
mod watch;

pub use apply::{AccessError, Applied, ApplyError, Interruption, UndoReport};
pub use changes::{Changes, NewFile, Rename};
pub use edit::{FileEdits, StaleEdit, TextEdit};
pub use file::{FileError, read, write_atomic};
pub use index::{Entry, EntryKind, VaultIndex};
pub use links::{
    Destination, IndexedLink, LinkContext, LinkIndex, LinkSubpath, LinkTarget, NoteLink,
    RenameConflict, RenamePlan, RenamePlanError,
};
pub use ops::FileOpError;
pub use path::{InvalidPath, InvalidReason, VaultPath};
pub use refresh::VaultChange;
pub use scan::walk_notes;
pub use vault::{Vault, VaultError, VaultOptions, VaultUpdate};
