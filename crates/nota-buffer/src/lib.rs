//! Buffer de texto de `nota`.
//!
//! Todo offset es un [`ByteOffset`]; la conversión a caracteres o columnas
//! visibles es trabajo del borde (UI o motor vim). Las ediciones se agrupan
//! en transacciones que forman un árbol de historial: deshacer y editar abre
//! una rama nueva sin perder la anterior.

mod buffer;
mod edit;
mod error;
mod history;
mod offset;
mod selection;

pub use buffer::Buffer;
pub use edit::{Bias, Edit, EditMap};
pub use error::BufferError;
pub use history::{HistorySeq, Transaction};
pub use offset::ByteOffset;
pub use selection::{Goal, PerSelection, Selection, Selections};
