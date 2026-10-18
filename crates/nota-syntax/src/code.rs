//! Lo que tapa la sintaxis de nota: bloques de código cercados, código
//! inline y comentarios `%% %%`. Cada función recibe dónde empieza la
//! construcción y devuelve dónde termina.

/// Fin de la línea que empieza en `start`, sin contar el salto.
pub(crate) fn line_end(text: &str, start: usize) -> usize {
    text[start..].find('\n').map_or(text.len(), |i| start + i)
}

/// Inicio de la línea siguiente a la que empieza en `start`.
fn next_line(text: &str, start: usize) -> usize {
    text[start..]
        .find('\n')
        .map_or(text.len(), |i| start + i + 1)
}

fn is_blank(line: &str) -> bool {
    line.trim().is_empty()
}

struct Fence {
    marker: u8,
    len: usize,
}

/// Una línea de cerca: tres o más `` ` `` o `~`. Se acepta cualquier sangría y
/// los `>` de una cita porque las cercas dentro de listas y citas son comunes
/// en notas y aquí no se parsean bloques.
fn fence(line: &str) -> Option<(Fence, &str)> {
    let rest = line.trim_start_matches([' ', '\t', '>']);
    let marker = *rest.as_bytes().first()?;
    if marker != b'`' && marker != b'~' {
        return None;
    }
    let len = rest.bytes().take_while(|&b| b == marker).count();
    (len >= 3).then(|| (Fence { marker, len }, &rest[len..]))
}

fn opening_fence(line: &str) -> Option<Fence> {
    let (fence, info) = fence(line)?;
    // CommonMark: el info string de una cerca de backticks no lleva backticks.
    (fence.marker != b'`' || !info.contains('`')).then_some(fence)
}

fn closes(open: &Fence, line: &str) -> bool {
    fence(line).is_some_and(|(close, rest)| {
        close.marker == open.marker && close.len >= open.len && is_blank(rest)
    })
}

/// Si la línea que empieza en `start` abre un bloque cercado, devuelve el fin
/// del bloque (tras la cerca de cierre, o el fin del texto si no cierra).
pub(crate) fn fenced_block_end(text: &str, start: usize) -> Option<usize> {
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

/// El cierre de la racha de `len` backticks que termina en `from`: otra del
/// mismo largo dentro del mismo párrafo. Si no hay, el fin del párrafo.
fn code_span_close(text: &str, from: usize, len: usize) -> Result<usize, usize> {
    let mut at = from;
    while let Some(i) = text[at..].find(['`', '\n']) {
        let found = at + i;
        if text.as_bytes()[found] == b'`' {
            let run = backtick_run(text, found);
            if run == len {
                return Ok(found + run);
            }
            at = found + run;
            continue;
        }
        at = found + 1;
        let line = &text[at..line_end(text, at)];
        if is_blank(line) || opening_fence(line).is_some() {
            return Err(at);
        }
    }
    Err(text.len())
}

/// Código inline. Recuerda los largos de racha que no cierran en el párrafo:
/// ninguna racha posterior de ese largo cierra tampoco, y sin esto un párrafo
/// lleno de backticks sueltos se recorrería una vez por racha.
#[derive(Default)]
pub(crate) struct CodeSpans {
    unclosed: Vec<(usize, usize)>,
}

impl CodeSpans {
    /// El fin del código inline que abre la racha en `at`; si no cierra, la
    /// racha es texto y se devuelve su fin.
    pub(crate) fn end(&mut self, text: &str, at: usize) -> usize {
        let len = backtick_run(text, at);
        let run_end = at + len;
        self.unclosed
            .retain(|&(_, paragraph_end)| at < paragraph_end);
        if self.unclosed.iter().any(|&(unclosed, _)| unclosed == len) {
            return run_end;
        }
        code_span_close(text, run_end, len).unwrap_or_else(|paragraph_end| {
            self.unclosed.push((len, paragraph_end));
            run_end
        })
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
