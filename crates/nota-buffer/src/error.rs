use std::ops::Range;

use crate::ByteOffset;

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum BufferError {
    #[error("el rango {start}..{end} está fuera del buffer (largo {len})")]
    OutOfBounds {
        start: ByteOffset,
        end: ByteOffset,
        len: ByteOffset,
    },
    #[error("el rango {start}..{end} está invertido")]
    InvertedRange { start: ByteOffset, end: ByteOffset },
    #[error("el offset {0} no cae en un límite de carácter")]
    NotCharBoundary(ByteOffset),
    #[error("las ediciones {first:?} y {second:?} se solapan")]
    OverlappingEdits {
        first: Range<ByteOffset>,
        second: Range<ByteOffset>,
    },
    #[error("no hay una transacción abierta")]
    NoOpenTransaction,
    #[error("ya hay una transacción abierta")]
    TransactionAlreadyOpen,
}
