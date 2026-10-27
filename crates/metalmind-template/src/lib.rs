//! Plantillas de notas: archivos markdown en una carpeta del vault con
//! `{{date}}`, `{{time}}`, `{{title}}` y `{{date:FORMATO}}`; la plantilla de
//! cada tag, que declara su página; y la nota del día.
//!
//! Todo es puro salvo [`Templates::load`] y [`capture`]: la UI lee las
//! plantillas una vez y pide borradores sin tocar el disco, y `metalmind add`
//! lee solo las carpetas de plantillas y de tags y crea la nota. La
//! plantilla de cada tag sale del `TagGraph` de las páginas de tag.

mod capture;
mod daily;
mod draft;
mod format;
mod gap;
mod load;
mod note_path;
mod render;
mod tags;
mod tags_field;
#[cfg(test)]
mod test_vault;

pub use capture::{AddError, Folders, FromTemplate, capture};
pub use daily::{DailyError, DailyNote, daily_note};
pub use draft::{Draft, DraftProblem, NewNote, Pick, Templates, draft};
pub use format::{DateFormat, FormatError};
pub use load::{Loaded, templates_folder};
pub use note_path::{NameError, NotePath};
pub use render::{Context, Rendered, VariableProblem, render};
pub use tags::{TagChoice, tag_of_template, tag_template, tags_asking_for};
pub use tags_field::{TagsNotAdded, with_tags};
