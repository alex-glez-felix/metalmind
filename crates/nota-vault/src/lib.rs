//! Las notas en disco. Por ahora, leer un archivo y guardarlo de forma
//! atómica; el walk, el watcher y el índice del vault llegan en M2.

mod file;

pub use file::{FileError, read, write_atomic};
