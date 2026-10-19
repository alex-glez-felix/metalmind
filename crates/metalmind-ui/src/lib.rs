//! Vistas de Metalmind sobre GPUI. Es el único crate, junto al binario, que
//! depende de la GUI: la lógica vive en el núcleo y en `workspace`, que
//! no importa GPUI, y aquí solo se pinta y se traduce la entrada.

mod actions;
mod clipboard;
mod document;
mod editor;
mod file_tree;
mod finder;
mod frames;
mod ime;
mod palette;
mod panel;
pub mod picker;
mod session;
mod sidebar;
pub mod text_search;
mod theme;
mod tooltip;
mod utf16;
mod window;
mod workspace;
mod workspace_view;

pub use document::{Document, FileStats, SaveError};
pub use file_tree::{FileTree, Row};
pub use frames::FrameClock;
pub use session::last_vault;
pub use theme::{NoMonoFont, Theme};
pub use window::{OpenError, Startup, open};
