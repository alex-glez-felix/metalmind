//! Motor vim de `nota`, sin GUI ni I/O.
//!
//! Las teclas pasan por un [`Keymap`] declarativo que las traduce a
//! [`Action`]s con nombre; [`Vim`] las ejecuta sobre un
//! [`nota_buffer::Buffer`] y sus selecciones. La paleta y `:` podrán invocar
//! las mismas acciones con [`Vim::execute`]. Los [`Command`]s de `:` que
//! hacen I/O (guardar, cerrar) se devuelven a quien aloja el editor; `:s` y
//! la búsqueda (`/`, `n`, `*`) los resuelve el motor, que deja el patrón
//! activo en [`Vim::search_highlight`] para que la UI lo resalte.

mod action;
mod command;
mod cursor;
mod dot;
mod error;
mod ex;
mod find;
mod history;
mod insert;
mod key;
mod keymap;
mod lines;
mod mode;
mod motion;
mod object;
mod operator;
mod paste;
mod range;
mod register;
mod search;
mod span;
mod substitute;
mod text_object;
mod travel;
mod vim;
mod word;

pub use action::{Action, InsertAt, PasteSide};
pub use command::{Command, CommandError};
pub use error::VimError;
pub use find::CharSearch;
pub use key::Key;
pub use keymap::{Binding, Context, Keymap};
pub use mode::{Mode, Prompt};
pub use motion::Motion;
pub use operator::Operator;
pub use search::{Pattern, SearchError, Wrap};
pub use substitute::Substitution;
pub use text_object::TextObject;
pub use vim::{KeyOutcome, Vim};
