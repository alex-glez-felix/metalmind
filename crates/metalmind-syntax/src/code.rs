//! Lo que tapa la sintaxis de Metalmind: bloques de código cercados, código
//! inline y comentarios `%% %%`. Cada función recibe dónde empieza la
//! construcción y devuelve dónde termina.

use std::collections::HashMap;
use std::ops::Range;

/// Fin de la línea que empieza en `start`, sin contar el salto.
pub(crate) fn line_end(text: &str, start: usize) -> usize {
    text[start..].find('\n').map_or(text.len(), |i| start + i)
}

/// Inicio de la línea donde está `at`.
pub(crate) fn line_start(text: &str, at: usize) -> usize {
    text[..at].rfind('\n').map_or(0, |newline| newline + 1)
}

/// Inicio de la línea siguiente a la que empieza en `start`.
fn next_line(text: &str, start: usize) -> usize {
    text[start..]
        .find('\n')
        .map_or(text.len(), |i| start + i + 1)
}

pub(crate) fn is_blank(line: &str) -> bool {
    line.trim().is_empty()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Fence {
    pub marker: u8,
    pub len: usize,
}

/// Una línea de cerca: tres o más `` ` `` o `~`. Se acepta cualquier sangría,
/// los `>` de una cita y los marcadores de lista (`-`, `*`, `+`, `1.`) porque las
/// cercas dentro de listas y citas son comunes en notas y aquí no se parsean
/// bloques.
pub(crate) fn fence(line: &str) -> Option<(Fence, &str)> {
    let mut rest = line.trim_start_matches([' ', '\t', '>']);
    while let Some(item) = strip_list_marker(rest) {
        rest = item.trim_start_matches([' ', '\t', '>']);
    }
    let marker = *rest.as_bytes().first()?;
    if marker != b'`' && marker != b'~' {
        return None;
    }
    let len = rest.bytes().take_while(|&b| b == marker).count();
    (len >= 3).then(|| (Fence { marker, len }, &rest[len..]))
}

/// Lo que sigue a `- `, `* `, `+ `, `1. ` o `1) ` al principio de `line`.
fn strip_list_marker(line: &str) -> Option<&str> {
    let digits = line.bytes().take_while(u8::is_ascii_digit).count();
    let marker = match line.as_bytes().get(digits) {
        Some(b'.' | b')') if (1..=9).contains(&digits) => digits + 1,
        Some(b'-' | b'*' | b'+') if digits == 0 => 1,
        _ => return None,
    };
    let after = &line[marker..];
    after.starts_with([' ', '\t']).then_some(after)
}

pub(crate) fn opening_fence(line: &str) -> Option<Fence> {
    let (fence, info) = fence(line)?;
    // CommonMark: el info string de una cerca de backticks no lleva backticks.
    (fence.marker != b'`' || !info.contains('`')).then_some(fence)
}

pub(crate) fn closes(open: &Fence, line: &str) -> bool {
    fence(line).is_some_and(|(close, rest)| {
        close.marker == open.marker && close.len >= open.len && is_blank(rest)
    })
}

/// Si la línea que empieza en `start` abre un bloque cercado, devuelve el fin
/// del bloque (tras la cerca de cierre, o el fin del texto si no cierra).
#[must_use]
pub fn fenced_block_end(text: &str, start: usize) -> Option<usize> {
    let open = opening_fence(&text[start..line_end(text, start)])?;
    let mut line = next_line(text, start);
    while line < text.len() {
        let end = line_end(text, line);
        if closes(&open, &text[line..end]) {
            return Some(next_line(text, line));
        }
        line = next_line(text, line);
    }
    Some(text.len())
}

fn backtick_run(text: &str, at: usize) -> usize {
    text[at..].bytes().take_while(|&b| b == b'`').count()
}

/// Inicio de la primera línea en blanco o de cerca tras `from`: el código
/// inline no la cruza.
fn paragraph_end(text: &str, from: usize) -> usize {
    let mut at = from;
    while let Some(i) = text[at..].find('\n') {
        at += i + 1;
        let line = &text[at..line_end(text, at)];
        if is_blank(line) || opening_fence(line).is_some() {
            return at;
        }
    }
    text.len()
}

/// Código inline: una racha de backticks cierra con la siguiente del mismo
/// largo en el mismo párrafo. Las rachas del párrafo se indexan por largo una
/// sola vez, así un párrafo lleno de rachas sin pareja sigue siendo lineal.
#[derive(Default)]
pub(crate) struct CodeSpans {
    paragraph: Range<usize>,
    starts_by_len: HashMap<usize, Vec<usize>>,
}

impl CodeSpans {
    /// El fin del código inline que abre la racha en `at`; si no cierra, la
    /// racha es texto y se devuelve su fin.
    pub(crate) fn end(&mut self, text: &str, at: usize) -> usize {
        self.find(text, at).0
    }

    /// Como [`CodeSpans::end`], y si la racha cerró.
    pub(crate) fn find(&mut self, text: &str, at: usize) -> (usize, bool) {
        let len = backtick_run(text, at);
        let run_end = at + len;
        if !self.paragraph.contains(&at) {
            self.index(text, at);
        }
        self.starts_by_len
            .get(&len)
            .and_then(|starts| starts.get(starts.partition_point(|&start| start < run_end)))
            .map_or((run_end, false), |&close| (close + len, true))
    }

    fn index(&mut self, text: &str, from: usize) {
        self.paragraph = from..paragraph_end(text, from);
        self.starts_by_len.clear();
        let mut at = from;
        while let Some(i) = text[at..self.paragraph.end].find('`') {
            let start = at + i;
            let len = backtick_run(&text[..self.paragraph.end], start);
            self.starts_by_len.entry(len).or_default().push(start);
            at = start + len;
        }
    }
}

/// Comentario `%% %%` desde `at`. Un `%%` sin cierre comenta hasta el final,
/// igual que una cerca sin cierre.
pub(crate) fn comment_end(text: &str, at: usize) -> usize {
    let body = at + 2;
    text[body..].find("%%").map_or(text.len(), |i| body + i + 2)
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn unclosed_fence_runs_to_the_end() {
        let text = "```rust\nlet x = 1;\n";
        assert_eq!(fenced_block_end(text, 0), Some(text.len()));
    }

    #[test]
    fn a_shorter_fence_does_not_close() {
        let text = "````\n```\n````\nfuera";
        assert_eq!(fenced_block_end(text, 0), Some(14));
    }

    #[test]
    fn a_fence_can_open_a_list_item() {
        let text = "- ```\n  [[a]]\n  ```\nfuera";
        assert_eq!(fenced_block_end(text, 0), Some(text.len() - "fuera".len()));
        assert_eq!(fenced_block_end("12. ~~~\n~~~\n", 0), Some(12));
    }

    #[test]
    fn a_rule_or_a_marker_without_space_is_not_a_fence() {
        assert_eq!(fenced_block_end("-```\n", 0), None);
        assert_eq!(fenced_block_end("- - -\n", 0), None);
    }

    #[test]
    fn backticks_in_the_info_string_are_not_a_fence() {
        assert_eq!(fenced_block_end("```a`b```", 0), None);
    }

    fn code_span_end(text: &str, at: usize) -> usize {
        CodeSpans::default().end(text, at)
    }

    #[test]
    fn code_span_needs_a_run_of_the_same_length() {
        assert_eq!(code_span_end("``a`b`` c", 0), 7);
        assert_eq!(code_span_end("``a` c", 0), 2);
    }

    #[test]
    fn code_span_does_not_cross_a_blank_line() {
        assert_eq!(code_span_end("`a\nb` c", 0), 5);
        assert_eq!(code_span_end("`a\n\nb` c", 0), 1);
    }

    #[test]
    fn an_unclosed_length_is_remembered_only_within_its_paragraph() {
        let text = "`` a ` b ``x\n\n`` c ``";
        let mut spans = CodeSpans::default();
        assert_eq!(spans.end(text, 5), 6);
        assert_eq!(spans.end(text, 0), 11);
        let mut spans = CodeSpans::default();
        assert_eq!(spans.end("` a\n\n` b `", 0), 1);
        assert_eq!(spans.end("` a\n\n` b `", 5), 10);
    }
}
