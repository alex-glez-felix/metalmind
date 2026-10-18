//! Un picker al estilo de Zed y Telescope: una ventana flotante con un input
//! y la lista de lo que coincide. El estado vive en [`PickerState`], sin
//! GPUI; la vista solo lo pinta y le pasa la entrada.

mod actions;
mod highlight;
mod input_line;
mod model;
mod view;

pub(crate) use self::actions::bind_keys;
pub use self::input_line::{InputLine, LineEdit};
pub use self::model::{Chosen, Job, Matches, Open, PickerState, Step};
pub(crate) use self::view::{Haystack, PickerEvent, PickerItem, PickerOptions, PickerView};
