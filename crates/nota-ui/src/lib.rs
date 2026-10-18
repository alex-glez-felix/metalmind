//! Vistas de `nota` sobre GPUI. Es el único crate, junto al binario, que
//! depende de la GUI: la lógica vive en el núcleo y en `workspace`, que
//! no importa GPUI, y aquí solo se pinta y se traduce la entrada.

mod actions;
mod clipboard;
mod document;
mod editor;
mod theme;
mod window;
mod workspace;
mod workspace_view;

pub use document::{Document, FileStats};
pub use theme::{NoMonoFont, Theme};
pub use window::{OpenError, open};
