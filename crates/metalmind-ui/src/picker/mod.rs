//! Un picker al estilo de Zed y Telescope: una ventana flotante con un input
//! y la lista de lo que coincide. La vista es una sola, [`PickerView`]; qué
//! lista y cómo busca lo pone su delegado: [`Fuzzy`] filtra una lista con
//! [`PickerState`], sin GPUI.

pub(crate) mod fit;
mod fuzzy;
mod highlight;
mod input_line;
mod model;
mod narrowing;
mod view;

pub(crate) use self::fuzzy::{Fuzzy, Haystack, PickerItem};
pub use self::input_line::{InputLine, LineEdit};
pub use self::model::{Chosen, Job, Matches, Open, PickerState, Step};
pub use self::narrowing::Narrowing;
pub(crate) use self::view::{
    Overflow, PickerDelegate, PickerEvent, PickerOptions, PickerRow, PickerView,
};
