//! La lista de notas plegable de la dirección D (#164): las notas de una
//! fuente (el buzón, una vista guardada, un tag, una carpeta o todas) con su título, extracto,
//! tags y fecha, a la izquierda de la nota. El modelo no sabe de GPUI; la
//! vista lo pinta y traduce la entrada.

mod cards;
mod date;
mod field;
mod handlers;
mod input;
mod model;
mod render;
mod source;
mod tagging;
mod view;
mod view_name;
mod views;

pub use self::cards::{Card, read_all};
pub use self::model::NoteList;
pub use self::source::{FolderError, ListSource, folder_to_show};
pub use self::tagging::tag_edit;
pub use self::view::{NoteListEvent, NoteListView};
pub use self::view_name::{ViewNameProblem, ViewNamePurpose, ViewNameQuestion};
pub use self::views::{DraftError, ViewDraft, ViewsError};
pub use self::views::{change as change_views, load as load_views, named as view_named};
