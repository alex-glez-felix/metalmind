//! Buffer de texto de Metalmind.
//!
//! Todo offset es un [`ByteOffset`]; la conversión a caracteres o columnas
//! visibles es trabajo del borde (UI o motor vim). Las ediciones se agrupan
//! en transacciones que forman un árbol de historial: deshacer y editar abre
//! una rama nueva sin perder la anterior.

mod anchor;
mod buffer;
mod edit;
mod error;
mod history;
mod marked;
mod offset;
mod selection;
mod splice;

pub use anchor::{LineAnchor, LineChange, Point};
pub use buffer::Buffer;
pub use edit::{Bias, Edit, EditMap};
pub use error::BufferError;
pub use history::{HistorySeq, Transaction};
pub use offset::ByteOffset;
pub use selection::{Goal, PerSelection, Selection, Selections};
pub use splice::{LineEdit, Splice};
