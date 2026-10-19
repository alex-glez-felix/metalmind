//! El vault en disco: abrir una carpeta, sus índices de archivos, links y
//! tags al día con lo que pasa en disco, crear, capturar, mover y borrar, el
//! guardado atómico de cada nota, aplicar y deshacer cambios en varios
//! archivos, los adjuntos y el historial de versiones de cada nota.

mod apply;
mod attachment;
mod capture;
mod cause;
mod changes;
mod edit;
mod file;
mod history;
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
mod worker;

pub use apply::{AccessError, Applied, ApplyError, Interruption, UndoReport};
pub use attachment::{
    AttachmentError, DEFAULT_ATTACHMENTS_FOLDER, FileKind, NewAttachment, SavedAttachment,
};
pub use capture::{CaptureError, Captured, NewNote};
pub use changes::{Changes, NewFile, Rename};
pub use edit::{FileEdits, StaleEdit, TextEdit};
pub use file::{FileError, create_atomic, read, write_atomic};
pub use history::{
    AUTOSAVE_GROUP, ChunkKind, ContentHash, DeletedId, DeletedNote, DiffChunk, History,
    HistoryError, Pruned, RestoreEdit, RestorePlan, TextDiff, Version, VersionId, VersionKind,
};
pub use index::{Entry, EntryKind, VaultIndex};
pub use links::{
    Destination, IndexedLink, LinkContext, LinkIndex, LinkSubpath, LinkTarget, NoteLink,
    RenameConflict, RenamePlan, RenamePlanError,
};
pub use ops::FileOpError;
pub use path::{InvalidPath, InvalidReason, VaultPath};
pub use refresh::VaultChange;
pub use scan::walk_notes;
pub use vault::{
    HistoryOptions, PendingVersion, SaveError, SaveKind, Vault, VaultError, VaultOptions,
    VaultRoot, VaultUpdate, VersionError,
};
