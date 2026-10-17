//! Vistas de `nota` sobre GPUI. Es el único crate, junto al binario, que
//! depende de la GUI: la lógica vive en el núcleo y aquí solo se pinta y se
//! traduce la entrada.

mod actions;
mod document;
mod editor;
mod theme;
mod window;

pub use document::{Document, DocumentError, FileStats};
pub use editor::Editor;
pub use theme::{NoMonoFont, Theme};
pub use window::{OpenError, open};
