//! El registro de acciones de Metalmind, sin GUI: el nombre de cada acción
//! (las de edición del motor vim y los comandos de la ventana) y sus
//! atajos, en un solo [`Keymap`].
//!
//! El motor vim recibe su tabla con [`Keymap::engine`]: sus acciones, y las
//! de la ventana que se teclean en el editor (`space f`, `ctrl-w v`, `:w`)
//! como nombres que le devuelve a quien lo aloja, que los resuelve con
//! [`Invocation::resolve`].

mod action;
mod command;
mod defaults;
mod keymap;
mod named;

pub use action::{Action, Invocation, UnknownAction};
pub use command::Command;
pub use keymap::{Binding, Keymap};
