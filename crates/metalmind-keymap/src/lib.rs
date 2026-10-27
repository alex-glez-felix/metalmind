//! El registro de acciones de Metalmind, sin GUI: el nombre de cada acción
//! (las de edición del motor vim y las de la ventana) y los atajos de
//! todos los contextos, en un solo [`Keymap`].
//!
//! El motor vim recibe su tabla con [`Keymap::engine`]: sus acciones, y las
//! de la ventana que se teclean en el editor (`space f`, `ctrl-w v`, `:w`)
//! como nombres que le devuelve a quien lo aloja. La ventana registra los
//! demás atajos ([`Keymap::view_bindings`]) en su framework, que solo los
//! transporta. `:`, la paleta y los atajos invocan lo mismo: una
//! [`Invocation`] resuelta por nombre.

mod action;
mod command;
mod defaults;
mod keymap;
mod keystroke;
mod named;
mod view;

pub use action::{Action, Invocation, Owner, UnknownAction};
pub use command::Command;
pub use keymap::{Binding, Keymap, Keys};
pub use keystroke::{KeyName, Keystroke};
pub use view::{
    EditorAction, ListAction, PanelAction, PickerAction, TagChangeAction, View, context,
};
