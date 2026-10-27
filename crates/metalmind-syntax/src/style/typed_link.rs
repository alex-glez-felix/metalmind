//! El wikilink que se está escribiendo, para el autocompletado de `[[`.

use std::ops::Range;

use super::{LineKind, LineStyle, Role, Span};
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
    /// línea de este estilo: tras su `[[` (o `![[`) sin escapar, sin `|`,
    /// `#` ni otro `[[` de por medio. Si un `]]` lo cierra, el cursor tiene
    /// que estar al final del destino. Como el scanner, nunca dentro de otro
    /// link (el destino de uno markdown), de código, fórmulas, comentarios
    /// ni el frontmatter.
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
        if is_escaped(before, open)
            || self.is_verbatim(open..at)
            || self.is_in_other_link(line, open)
        {
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

    /// Si `range` toca algo donde un `[[` no es un link: código, fórmulas
    /// o comentarios. Uno que empieza justo al final no lo toca, salvo que
    /// su marcador de apertura sea lo último de `range`.
    fn is_verbatim(&self, range: Range<usize>) -> bool {
        let inside = self.spans.iter().any(|span| {
            is_verbatim(span) && span.range.start < range.end && range.start < span.range.end
        });
        let just_opened = self.spans.windows(2).any(|pair| {
            pair[0].style.role == Role::Markup
                && pair[0].range.end == range.end
                && pair[1].range.start == range.end
                && is_verbatim(&pair[1])
        });
        inside || just_opened
    }

    /// Si el `[[` en `open` es parte de otro link de la línea y no el
    /// principio de uno propio.
    fn is_in_other_link(&self, line: &str, open: usize) -> bool {
        let own =
            |start: usize| start == open || (start + 1 == open && line[start..].starts_with('!'));
        self.links
            .iter()
            .any(|link| link.range.contains(&open) && !own(link.range.start))
    }
}

fn is_verbatim(span: &Span) -> bool {
    matches!(
        span.style.role,
        Role::Code | Role::Token(_) | Role::Math | Role::Comment
    )
}

/// Si el `[[` en `open` va tras un `\` que no está escapado él mismo.
fn is_escaped(before: &str, open: usize) -> bool {
    let slashes = before[..open]
        .bytes()
        .rev()
        .take_while(|&byte| byte == b'\\')
        .count();
    slashes % 2 == 1
}
