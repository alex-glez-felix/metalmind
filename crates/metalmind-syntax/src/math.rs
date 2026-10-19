//! Fórmulas LaTeX, con las reglas de pandoc que sigue Obsidian
//! ([08](../../../docs/research/08-matematicas.md) §7):
//!
//! - `$` abre una fórmula en línea si lo sigue algo que no es espacio, y
//!   cierra en el siguiente `$` de la misma línea que sigue a algo que no
//!   es espacio y al que no sigue un dígito: `$5 y $10` no es fórmula.
//! - `$$` abre un bloque que llega hasta el siguiente `$$`, aunque cruce
//!   líneas; si no cierra, llega hasta el final, como un comentario `%%`.
//! - `\$` es un dólar: no abre ni cierra. Dentro de una fórmula, un `\`
//!   seguido de puntuación ASCII se salta entero, así `\$` y `\\` son de
//!   LaTeX.
//!
//! Lo que tapa una fórmula (código, comentarios, links, tags) lo decide el
//! orden: lo que abre primero gana, como con el código inline.

use std::ops::Range;

use crate::code;

/// Una fórmula que encontró el scanner, en bytes sobre su texto.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Math {
    /// Con sus `$`.
    pub span: Range<usize>,
    /// El LaTeX, sin los `$`.
    pub content: Range<usize>,
    /// `$$…$$`.
    pub display: bool,
    /// Si llegó a su `$$`: un bloque sin cerrar es texto de LaTeX que no
    /// se compone.
    pub closed: bool,
}

/// Las fórmulas en línea de una línea. Los `$` que pueden cerrar una se
/// indexan una sola vez por línea, así una línea llena de `$` sin pareja
/// (precios) sigue siendo lineal. Que un `$` esté escapado no depende de
/// dónde abrió la fórmula: el `$` que abre corta cualquier racha de `\`.
#[derive(Default)]
pub(crate) struct InlineMath {
    line: Range<usize>,
    closes: Vec<usize>,
}

impl InlineMath {
    /// La fórmula que abre el `$` de `at`, si cierra en su línea.
    pub(crate) fn at(&mut self, text: &str, at: usize) -> Option<Math> {
        let content = at + 1;
        let first = text[content..].chars().next()?;
        if first.is_whitespace() {
            return None;
        }
        if !self.line.contains(&at) {
            self.index(text, at);
        }
        let next = self.closes.partition_point(|&close| close <= content);
        let close = *self.closes.get(next)?;
        Some(Math {
            span: at..close + 1,
            content: content..close,
            display: false,
            closed: true,
        })
    }

    fn index(&mut self, text: &str, at: usize) {
        self.line = code::line_start(text, at)..code::line_end(text, at);
        self.closes.clear();
        let bytes = text.as_bytes();
        let mut index = self.line.start;
        while index < self.line.end {
            match bytes[index] {
                b'\\' if bytes.get(index + 1).is_some_and(u8::is_ascii_punctuation) => index += 2,
                b'$' => {
                    if closes_inline(text, self.line.start, index) {
                        self.closes.push(index);
                    }
                    index += 1;
                }
                _ => index += 1,
            }
        }
    }
}

/// Si el `$` de `at` puede cerrar: lo precede algo que no es espacio y no
/// lo sigue un dígito.
fn closes_inline(text: &str, line_start: usize, at: usize) -> bool {
    let before = text[line_start..at].chars().next_back();
    let after = text.as_bytes().get(at + 1);
    before.is_some_and(|c| !c.is_whitespace()) && !after.is_some_and(u8::is_ascii_digit)
}

/// El primer `$$` de `text[from..to]` que no está escapado.
pub(crate) fn display_close(text: &str, from: usize, to: usize) -> Option<usize> {
    let bytes = text.as_bytes();
    let mut index = from;
    while index + 1 < to {
        match bytes[index] {
            b'\\' if bytes.get(index + 1).is_some_and(u8::is_ascii_punctuation) => index += 2,
            b'$' if bytes[index + 1] == b'$' => return Some(index),
            _ => index += 1,
        }
    }
    None
}

/// El bloque que abre el `$$` de `at`, buscando su cierre hasta `limit`.
/// Si no cierra, llega hasta `limit`.
pub(crate) fn display_at(text: &str, at: usize, limit: usize) -> Math {
    let content = at + 2;
    match display_close(text, content, limit) {
        Some(close) => Math {
            span: at..close + 2,
            content: content..close,
            display: true,
            closed: true,
        },
        None => Math {
            span: at..limit,
            content: content..limit,
            display: true,
            closed: false,
        },
    }
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;

    use super::*;

    fn inline(text: &str) -> Option<&str> {
        let math = InlineMath::default().at(text, 0)?;
        Some(&text[math.content])
    }

    #[test]
    fn inline_needs_no_space_inside_its_dollars() {
        assert_eq!(inline("$x$"), Some("x"));
        assert_eq!(inline("$ x$"), None);
        assert_eq!(inline("$x $"), None);
        assert_eq!(inline("$x $y$"), Some("x $y"));
        assert_eq!(inline("$x\n$"), None);
    }

    #[test]
    fn a_dollar_before_a_digit_does_not_close() {
        assert_eq!(inline("$5 y $10"), None);
        assert_eq!(inline("$a$1 $b$"), Some("a$1 $b"));
    }

    #[test]
    fn an_escaped_dollar_does_not_close() {
        assert_eq!(inline(r"$\$$"), Some(r"\$"));
        assert_eq!(inline(r"$a\$"), None);
        assert_eq!(inline(r"$a\\$"), Some(r"a\\"));
    }

    #[test]
    fn display_skips_escapes_and_runs_to_the_limit() {
        let text = r"$$a\$$b$$";
        assert_eq!(display_at(text, 0, text.len()).content, 2..7);
        let open = display_at("$$a", 0, 3);
        assert_eq!((open.content, open.closed), (2..3, false));
    }
}
