//! Motor vim de `nota`, sin GUI ni I/O.
//!
//! Las teclas pasan por un [`Keymap`] declarativo que las traduce a
//! [`Action`]s con nombre; [`Vim`] las ejecuta sobre un
//! [`nota_buffer::Buffer`] y sus selecciones. La paleta y `:` podrán invocar
//! las mismas acciones con [`Vim::execute`]. Los [`Command`]s de `:` que
//! hacen I/O (guardar, cerrar) se devuelven a quien aloja el editor.

mod action;
mod command;
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
pub use command::{Command, CommandError};
pub use error::VimError;
pub use key::Key;
pub use keymap::{Binding, Context, Keymap};
pub use motion::Motion;
pub use operator::Operator;
pub use text_object::TextObject;
pub use vim::{KeyOutcome, Mode, Vim};
