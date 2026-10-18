//! Plantillas de notas: archivos markdown en una carpeta del vault con
//! `{{date}}`, `{{time}}`, `{{title}}` y `{{date:FORMATO}}`; la plantilla de
//! cada tag, que declara su página; y la nota del día.
//!
//! Todo es puro salvo [`Templates::load`]: la UI y la CLI leen las
//! plantillas una vez y piden borradores sin tocar el disco. La plantilla de
//! cada tag sale del `TagGraph` que ya mantiene el vault.

mod daily;
mod draft;
mod format;
mod load;
mod render;
mod tags;
mod tags_field;
#[cfg(test)]
mod test_vault;

pub use daily::{DailyError, DailyNote, daily_note};
pub use draft::{Draft, DraftProblem, NewNote, Pick, Templates, draft};
pub use format::{DateFormat, FormatError};
pub use load::Loaded;
pub use render::{Context, Rendered, VariableProblem, render};
pub use tags::{TagChoice, tag_template};
pub use tags_field::TagsNotAdded;
