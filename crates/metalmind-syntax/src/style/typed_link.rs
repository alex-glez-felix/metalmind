//! El wikilink que se está escribiendo, para el autocompletado de `[[`.

use std::ops::Range;

use super::{LineKind, LineStyle, Role};
use crate::link::wikilink_close;

/// Un `[[` con lo escrito de su destino hasta el cursor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypedWikilink {
    /// Lo escrito del destino, en bytes de la línea; termina en el cursor.
    pub target: Range<usize>,
    /// Si un `]]` ya lo cierra: aceptar no lo vuelve a escribir.
    pub closed: bool,
}

impl LineStyle {
    /// El destino de wikilink que termina en el byte `at` de `line`, la
    /// línea de este estilo: tras su `[[` (o `![[`), sin `|`, `#` ni otro
    /// `[[` de por medio. Si un `]]` lo cierra, el cursor tiene que estar al
    /// final del destino. Nunca en código, fórmulas, comentarios ni el
    /// frontmatter.
    #[must_use]
    pub fn typed_wikilink(&self, line: &str, at: usize) -> Option<TypedWikilink> {
        if matches!(self.kind, LineKind::Frontmatter | LineKind::Fence(_)) {
            return None;
        }
        let before = line.get(..at)?;
        let open = before.rfind("[[")?;
        let body = open + 2;
        if before[body..].contains(['|', '#']) || before[body..].contains("]]") {
            return None;
        }
        if self.is_verbatim(open) {
            return None;
        }
        let closed = match wikilink_close(line, body) {
            Some(close) if close >= at => {
                let end = line[at..close].find(['|', '#']).map_or(close, |i| at + i);
                if end != at {
                    return None;
                }
                true
            }
            Some(_) => return None,
            None => false,
        };
        Some(TypedWikilink {
            target: body..at,
            closed,
        })
    }

    /// Si el byte `at` cae en algo donde un `[[` no es un link.
    fn is_verbatim(&self, at: usize) -> bool {
        self.spans.iter().any(|span| {
            span.range.contains(&at)
                && matches!(
                    span.style.role,
                    Role::Code | Role::Token(_) | Role::Math | Role::Comment
                )
        })
    }
}
