use std::ops::Range;

use crate::code::line_end;
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
    /// del `- valor` o el valor con su coma.
    pub removal: Range<usize>,
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
        let removal = match place {
            Place::List => list_removal(text, &span),
            Place::Text => span.clone(),
        };
        Some(Self {
            text: trimmed.to_owned(),
            span,
            literal,
            removal,
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

fn line_start(text: &str, at: usize) -> usize {
    text[..at].rfind('\n').map_or(0, |newline| newline + 1)
}

/// En una lista en bloque, la línea entera; en una lista entre corchetes,
/// el valor con la coma que lo separa del anterior o, si es el primero, del
/// siguiente. Así quitar dos valores seguidos nunca toca la misma coma.
fn list_removal(text: &str, span: &Range<usize>) -> Range<usize> {
    let start = line_start(text, span.start);
    if text[start..span.start].trim() == "-" {
        let end = line_end(text, span.end);
        return start..(end + 1).min(text.len());
    }
    let before = text[..span.start].trim_end_matches([' ', '\t']);
    if before.ends_with(',') {
        return before.len() - 1..span.end;
    }
    let after = skip_blanks(text, span.end);
    if text[after..].starts_with(',') {
        return span.start..skip_blanks(text, after + 1);
    }
    span.clone()
}

fn skip_blanks(text: &str, at: usize) -> usize {
    at + (text[at..].len() - text[at..].trim_start_matches([' ', '\t']).len())
}
