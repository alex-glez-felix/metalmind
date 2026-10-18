use std::ops::Range;

use crate::code::{line_end, line_start};
use crate::tag::is_valid_tag_name;

/// Un texto del frontmatter y dónde está en la nota, para poder reescribirlo
/// sin tocar el resto del YAML.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Value {
    /// Sin espacios alrededor.
    pub text: String,
    /// El escalar entero, comillas incluidas.
    pub span: Range<usize>,
    /// Dónde aparece `text` byte a byte en la nota; `None` si el YAML lo
    /// escribe distinto (escapes, varias líneas, un alias `*a`).
    pub literal: Option<Range<usize>>,
    /// Lo que se borra para sacar el valor de su lista sin romperla: la línea
    /// del `- valor`, o el valor con la coma que lo separa del anterior (del
    /// siguiente si es el primero).
    pub removal: Range<usize>,
    /// Lo que se borra si también se borran todos los de antes en su lista:
    /// el valor con la coma que lo sigue. Así quitar los primeros de
    /// `[a, b, c]` nunca toca dos veces la misma coma.
    pub removal_first: Range<usize>,
}

/// Cómo estaba escrito el campo del valor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Place {
    List,
    Text,
}

impl Value {
    /// `raw` es el valor tal como lo leyó el YAML; `span`, el escalar en `text`.
    pub(super) fn new(text: &str, raw: &str, span: Range<usize>, place: Place) -> Option<Self> {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            return None;
        }
        let literal = literal(text, raw, &span).map(|inner| {
            let start = inner.start + (raw.len() - raw.trim_start().len());
            start..start + trimmed.len()
        });
        let (removal, removal_first) = match place {
            Place::List => list_removals(text, &span),
            Place::Text => (span.clone(), span.clone()),
        };
        Some(Self {
            text: trimmed.to_owned(),
            span,
            literal,
            removal,
            removal_first,
        })
    }

    /// El valor como nombre de tag: sin el `#` del principio, que se ignora,
    /// y solo si es un nombre válido.
    #[must_use]
    pub fn as_tag(&self) -> Option<Self> {
        let name = self.text.strip_prefix('#').unwrap_or(&self.text);
        if !is_valid_tag_name(name) {
            return None;
        }
        let hash = self.text.len() - name.len();
        Some(Self {
            text: name.to_owned(),
            literal: self
                .literal
                .as_ref()
                .map(|literal| literal.start + hash..literal.end),
            ..self.clone()
        })
    }
}

/// El rango de `raw` dentro del escalar, si está escrito tal cual: sin
/// comillas o entre comillas sin escapes.
fn literal(text: &str, raw: &str, span: &Range<usize>) -> Option<Range<usize>> {
    let source = &text[span.clone()];
    let quoted = source.len() >= 2
        && (source.starts_with('"') && source.ends_with('"')
            || source.starts_with('\'') && source.ends_with('\''));
    let inner = if quoted {
        span.start + 1..span.end - 1
    } else {
        span.clone()
    };
    (text[inner.clone()] == *raw).then_some(inner)
}

/// [`Value::removal`] y [`Value::removal_first`]. En una lista en bloque,
/// las dos son la línea entera. Entre corchetes, la coma puede estar en otra
/// línea (`[a,\n b]`).
fn list_removals(text: &str, span: &Range<usize>) -> (Range<usize>, Range<usize>) {
    let start = line_start(text, span.start);
    if text[start..span.start].trim() == "-" {
        let line = start..(line_end(text, span.end) + 1).min(text.len());
        return (line.clone(), line);
    }
    let after = span.end + (text[span.end..].len() - text[span.end..].trim_start().len());
    let first = if text[after..].starts_with(',') {
        span.start..skip_blanks(text, after + 1)
    } else {
        span.clone()
    };
    let before = text[..span.start].trim_end();
    let removal = if before.ends_with(',') {
        before.len() - 1..span.end
    } else {
        first.clone()
    };
    (removal, first)
}

fn skip_blanks(text: &str, at: usize) -> usize {
    at + (text[at..].len() - text[at..].trim_start_matches([' ', '\t']).len())
}
