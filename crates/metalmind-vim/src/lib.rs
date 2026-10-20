//! Motor vim de Metalmind, sin GUI ni I/O.
//!
//! Las teclas pasan por un [`Keymap`] declarativo que las traduce a
//! [`Action`]s con nombre; [`Vim`] las ejecuta sobre un
//! [`metalmind_buffer::Buffer`] y sus selecciones. La paleta invoca las
//! mismas acciones con [`Vim::execute`]. El motor solo conoce sus acciones de
//! edición: los atajos y los comandos de `:` que quien lo aloja le agrega al
//! keymap (guardar, abrir, los paneles) vuelven como un [`ActionName`] en
//! [`KeyOutcome::Dispatch`]. `:s` y la búsqueda (`/`, `n`, `*`) los resuelve
//! el motor, que deja el patrón activo en [`Vim::search_highlight`] para que
//! la UI lo resalte.

mod action;
mod block;
mod columns;
mod command;
mod cursor;
mod dot;
mod error;
mod ex;
mod find;
mod history;
mod indent;
mod insert;
mod jumplist;
mod key;
mod keymap;
mod layout;
mod lines;
mod macros;
mod mark;
mod markdown;
mod matching;
mod mode;
mod motion;
mod object;
mod operator;
mod paste;
mod register;
mod scroll;
mod search;
mod shift;
mod span;
mod substitute;
mod text_object;
mod transform;
mod travel;
mod vim;
mod visual;
mod word;

pub use action::{Action, InsertAt, PasteSide};
pub use block::VisualBlock;
pub use command::{ActionName, CommandError, HostCommand, Invocation, RangeRule};
pub use error::VimError;
pub use ex::GlobalMiss;
pub use find::CharSearch;
pub use indent::Indent;
pub use jumplist::{FileId, Jump, JumpList, Landing};
pub use key::Key;
pub use keymap::{Binding, Bound, Context, Keymap};
pub use layout::{BufferLines, Layout};
pub use mark::{Marks, PlacedMark};
pub use markdown::Markdown;
pub use mode::{Mode, Prompt, VisualKind};
pub use motion::Motion;
pub use operator::Operator;
pub use register::{Clipboard, SystemRegister};
pub use scroll::{LineHeights, Place, Scroll, Uniform, Viewport};
pub use search::{Pattern, SearchError, Wrap};
pub use substitute::Substitution;
pub use text_object::TextObject;
pub use vim::{KeyOutcome, Vim};
