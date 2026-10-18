//! El texto de un error de serde-saphyr sin la posición ni el fragmento del
//! archivo: el [`Diagnostic`](crate::Diagnostic) ya lleva línea y columna.

use std::borrow::Cow;

use serde_saphyr::localizer::Localizer;
use serde_saphyr::{Error, Location, MessageFormatter, SnippetMode, UserMessageFormatter};

pub(super) fn render(error: &Error) -> String {
    error.render_with_options(serde_saphyr::render_options! {
        formatter: &WithoutLocation,
        snippets: SnippetMode::Off,
    })
}

struct WithoutLocation;

impl Localizer for WithoutLocation {
    fn attach_location<'a>(&self, base: Cow<'a, str>, _: Location) -> Cow<'a, str> {
        base
    }
}

impl MessageFormatter for WithoutLocation {
    fn localizer(&self) -> &dyn Localizer {
        self
    }

    fn format_message<'a>(&self, error: &'a Error) -> Cow<'a, str> {
        UserMessageFormatter.format_message(error)
    }
}
