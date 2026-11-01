//! Un picker al estilo de Zed y Telescope: una ventana flotante con un input
//! y la lista de lo que coincide. La vista es una sola, [`PickerView`]; qué
//! lista y cómo busca lo pone su delegado: [`Fuzzy`] filtra una lista con
//! [`PickerState`], sin GPUI. Un delegado puede mostrar además una
//! [`Preview`] de la fila marcada.

pub(crate) mod fit;
mod fuzzy;
mod highlight;
mod input_line;
mod model;
mod narrowing;
mod preview;
mod view;

pub(crate) use self::fuzzy::{Fuzzy, Haystack, PickerItem};
pub use self::input_line::{InputLine, LineEdit};
pub use self::model::{Chosen, Job, Matches, Open, PickerState, Step};
pub use self::narrowing::Narrowing;
pub use self::preview::{Preview, PreviewLine, lines_that_fit, preview_width, visible_lines};
pub(crate) use self::view::{
    Overflow, PickerDelegate, PickerEvent, PickerOptions, PickerRow, PickerView,
};
