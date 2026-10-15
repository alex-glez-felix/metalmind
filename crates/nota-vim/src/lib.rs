//! Motor vim de `nota`, sin GUI ni I/O.
//!
//! Las teclas pasan por un [`Keymap`] declarativo que las traduce a
//! [`Action`]s con nombre; [`Vim`] las ejecuta sobre un
//! [`nota_buffer::Buffer`] y sus selecciones. La paleta y `:` podrán invocar
//! las mismas acciones con [`Vim::execute`].

mod action;
mod cursor;
mod error;
mod insert;
mod key;
mod keymap;
mod lines;
mod motion;
mod operator;
mod paste;
mod register;
mod span;
mod text_object;
mod vim;
mod word;

pub use action::{Action, InsertAt, PasteSide};
pub use error::{KeymapError, VimError};
pub use key::Key;
pub use keymap::{Context, Keymap};
pub use motion::Motion;
pub use operator::Operator;
pub use text_object::TextObject;
pub use vim::{KeyOutcome, Mode, Vim};
