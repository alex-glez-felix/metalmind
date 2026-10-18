//! Qué tramo de cada línea visible se pinta resaltado, sin nada de GPUI:
//! la selección según el modo y las coincidencias de la búsqueda.

use std::ops::Range;

use nota_buffer::{Buffer, ByteOffset, Selection};
use nota_vim::{Mode, VisualKind};

/// Un tramo resaltado de una línea, en bytes desde su inicio.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Span {
    pub line: usize,
    pub start: usize,
    pub end: SpanEnd,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SpanEnd {
    /// Hasta este byte, sin incluirlo.
    At(usize),
    /// Hasta el fin del texto y una celda más: el salto de línea, como lo
    /// pinta vim.
    PastText,
    /// Hasta el borde derecho del panel.
    Edge,
}

/// Lo que se pinta de `selection` en las líneas `visible`. En Visual, los
/// dos extremos van incluidos (el contrato de `nota-vim`): `v` pinta hasta
/// el carácter bajo el cursor y, si está sobre el fin de línea, el salto;
/// `V` pinta las líneas completas hasta el borde, también las vacías. Fuera
/// de Visual es una selección del ratón, con `head` exclusivo.
#[must_use]
pub fn selection_spans(
    buffer: &Buffer,
    selection: Selection,
    mode: Mode,
    visible: Range<usize>,
) -> Vec<Span> {
    let (start, end) = (
        selection.anchor.min(selection.head),
        selection.anchor.max(selection.head),
    );
    let lines = buffer.line_of(start)..=buffer.line_of(end);
    let in_view = *lines.start().max(&visible.start)..(*lines.end() + 1).min(visible.end);
    let start_column = |line: usize| {
        if line == *lines.start() {
            start.get() - buffer.line_start(line).get()
        } else {
            0
        }
    };
    let last_end = match mode {
        Mode::Visual(VisualKind::Line) => {
            return in_view
                .map(|line| Span {
                    line,
                    start: 0,
                    end: SpanEnd::Edge,
                })
                .collect();
        }
        Mode::Visual(VisualKind::Char) => inclusive_end(buffer, end),
        _ if selection.is_cursor() => return Vec::new(),
        _ => SpanEnd::At(end.get() - buffer.line_start(*lines.end()).get()),
    };
    in_view
        .map(|line| Span {
            line,
            start: start_column(line),
            end: if line == *lines.end() {
                last_end
            } else {
                SpanEnd::PastText
            },
        })
        .filter(|span| span.end != SpanEnd::At(span.start))
        .collect()
}

/// Hasta el carácter en `offset` incluido, o el salto si está al final.
fn inclusive_end(buffer: &Buffer, offset: ByteOffset) -> SpanEnd {
    let line = buffer.line_of(offset);
    if offset >= buffer.line_end(line) {
        SpanEnd::PastText
    } else {
        SpanEnd::At(buffer.next_boundary(offset).get() - buffer.line_start(line).get())
    }
}

#[cfg(test)]
mod tests {
    use nota_buffer::Goal;
    use pretty_assertions::assert_eq;

    use super::*;

    const CHAR: Mode = Mode::Visual(VisualKind::Char);
    const LINE: Mode = Mode::Visual(VisualKind::Line);

    fn spans(text: &str, anchor: usize, head: usize, mode: Mode) -> Vec<(usize, usize, SpanEnd)> {
        let buffer = Buffer::from(text);
        let selection = Selection {
            anchor: ByteOffset::new(anchor),
            head: ByteOffset::new(head),
            goal: Goal::None,
        };
        selection_spans(&buffer, selection, mode, 0..buffer.line_count())
            .into_iter()
            .map(|span| (span.line, span.start, span.end))
            .collect()
    }

    #[test]
    fn v_incluye_el_caracter_bajo_el_cursor() {
        assert_eq!(spans("hola", 1, 1, CHAR), [(0, 1, SpanEnd::At(2))]);
        assert_eq!(spans("hola", 1, 2, CHAR), [(0, 1, SpanEnd::At(3))]);
        assert_eq!(spans("hola", 3, 0, CHAR), [(0, 0, SpanEnd::At(4))]);
        assert_eq!(spans("año", 0, 1, CHAR), [(0, 0, SpanEnd::At(3))]);
    }

    #[test]
    fn v_sobre_el_fin_de_linea_pinta_el_salto() {
        assert_eq!(spans("ab\ncd", 1, 2, CHAR), [(0, 1, SpanEnd::PastText)]);
        assert_eq!(spans("", 0, 0, CHAR), [(0, 0, SpanEnd::PastText)]);
        assert_eq!(spans("ab", 0, 2, CHAR), [(0, 0, SpanEnd::PastText)]);
    }

    #[test]
    fn v_en_varias_lineas_pinta_los_saltos_intermedios() {
        assert_eq!(
            spans("ab\n\ncd", 1, 4, CHAR),
            [
                (0, 1, SpanEnd::PastText),
                (1, 0, SpanEnd::PastText),
                (2, 0, SpanEnd::At(1)),
            ]
        );
    }

    #[test]
    fn mayuscula_v_pinta_lineas_completas_hasta_el_borde() {
        let full = |line| (line, 0, SpanEnd::Edge);
        assert_eq!(
            spans("ab\n\ncd\nef", 5, 1, LINE),
            [full(0), full(1), full(2)]
        );
        assert_eq!(spans("\n", 0, 0, LINE), [full(0)]);
    }

    #[test]
    fn el_raton_no_incluye_el_head() {
        assert_eq!(spans("hola", 1, 3, Mode::Normal), [(0, 1, SpanEnd::At(3))]);
        assert_eq!(
            spans("ab\ncd", 1, 3, Mode::Normal),
            [(0, 1, SpanEnd::PastText)]
        );
        assert_eq!(spans("hola", 2, 2, Mode::Normal), []);
    }

    #[test]
    fn solo_las_lineas_visibles() {
        let buffer = Buffer::from("a\nb\nc\nd\n");
        let selection = Selection {
            anchor: ByteOffset::new(0),
            head: ByteOffset::new(6),
            goal: Goal::None,
        };
        let lines: Vec<usize> = selection_spans(&buffer, selection, LINE, 1..3)
            .iter()
            .map(|span| span.line)
            .collect();
        assert_eq!(lines, [1, 2]);
    }
}
