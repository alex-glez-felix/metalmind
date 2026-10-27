//! Vistas de Metalmind sobre GPUI. Es el único crate, junto al binario, que
//! depende de la GUI: la lógica vive en el núcleo y en `workspace`, que
//! no importa GPUI, y aquí solo se pinta y se traduce la entrada.

mod actions;
mod clipboard;
mod completion;
mod disk;
mod document;
mod editor;
mod file_tree;
mod finder;
mod focus;
mod focus_indicator;
mod frames;
mod ime;
pub mod math;
mod new_note;
mod palette;
mod panel;
pub mod picker;
mod session;
mod sidebar;
mod tag_change;
mod tag_outline;
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
pub use panel::Outline;
pub use session::last_vault;
pub use theme::{NoMonoFont, Theme};
pub use window::{OpenError, Startup, open, open_window};
