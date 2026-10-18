//! Qué tramo de cada línea visible se pinta resaltado, sin nada de GPUI:
//! la selección según el modo y las coincidencias de la búsqueda.

use std::ops::Range;

use metalmind_buffer::{Buffer, ByteOffset, Selection};
use metalmind_vim::{Mode, Pattern, Viewport, VisualBlock, VisualKind};

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
/// dos extremos van incluidos (el contrato de `metalmind-vim`): `v` pinta hasta
/// el carácter bajo el cursor y, si está sobre el fin de línea, el salto;
/// `V` pinta las líneas completas hasta el borde, también las vacías; un
/// bloque, lo que abarca en cada línea, medido con el `tabstop` del panel.
/// Fuera de Visual es una selección del ratón, con `head` exclusivo.
#[must_use]
pub fn selection_spans(
    buffer: &Buffer,
    selection: Selection,
    mode: Mode,
    viewport: &Viewport,
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
        Mode::Visual(VisualKind::Block) => {
            return block_spans(buffer, &selection, viewport, in_view);
        }
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

/// Los caracteres de cada línea que el bloque abarca, aunque sea en parte,
/// como un tab; nada en las que terminan antes de él.
fn block_spans(
    buffer: &Buffer,
    selection: &Selection,
    viewport: &Viewport,
    lines: Range<usize>,
) -> Vec<Span> {
    let block = VisualBlock::new(buffer, selection, viewport);
    lines
        .filter_map(|line| {
            let bytes = block.bytes_on(buffer, line);
            let line_start = buffer.line_start(line).get();
            (!bytes.is_empty()).then(|| Span {
                line,
                start: bytes.start.get() - line_start,
                end: SpanEnd::At(bytes.end.get() - line_start),
            })
        })
        .collect()
}

/// Una coincidencia de la búsqueda; `current` si el cursor está en ella,
/// como `CurSearch` de nvim.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Match {
    pub span: Span,
    pub current: bool,
}

/// Las coincidencias de `pattern` en las líneas `visible`. Las vacías (como
/// la de `^`) no se pintan.
#[must_use]
pub fn search_spans(
    pattern: &Pattern,
    buffer: &Buffer,
    visible: Range<usize>,
    cursor: ByteOffset,
) -> Vec<Match> {
    pattern
        .matches_in(buffer, visible)
        .into_iter()
        .filter(|found| !found.is_empty())
        .map(|found| {
            let line = buffer.line_of(found.start);
            let line_start = buffer.line_start(line).get();
            Match {
                span: Span {
                    line,
                    start: found.start.get() - line_start,
                    end: SpanEnd::At(found.end.get() - line_start),
                },
                current: found.contains(&cursor),
            }
        })
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
    use metalmind_buffer::{Goal, Selections};
    use metalmind_vim::{Key, Vim};
    use pretty_assertions::assert_eq;

    use super::*;

    const CHAR: Mode = Mode::Visual(VisualKind::Char);
    const LINE: Mode = Mode::Visual(VisualKind::Line);
    const BLOCK: Mode = Mode::Visual(VisualKind::Block);

    fn spans(text: &str, anchor: usize, head: usize, mode: Mode) -> Vec<(usize, usize, SpanEnd)> {
        spans_with_goal(text, anchor, head, mode, Goal::None)
    }

    fn spans_with_goal(
        text: &str,
        anchor: usize,
        head: usize,
        mode: Mode,
        goal: Goal,
    ) -> Vec<(usize, usize, SpanEnd)> {
        let buffer = Buffer::from(text);
        let selection = Selection {
            anchor: ByteOffset::new(anchor),
            head: ByteOffset::new(head),
            goal,
        };
        let viewport = Viewport::default();
        selection_spans(&buffer, selection, mode, &viewport, 0..buffer.line_count())
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
    fn un_bloque_pinta_lo_que_abarca_en_cada_linea() {
        assert_eq!(
            spans("abcd\na\tb\nxy", 0, 11, BLOCK),
            [
                (0, 0, SpanEnd::At(3)),
                (1, 0, SpanEnd::At(2)),
                (2, 0, SpanEnd::At(2)),
            ]
        );
        assert_eq!(
            spans("abcd\nx\nabcd", 1, 12, BLOCK),
            [(0, 1, SpanEnd::At(4)), (2, 1, SpanEnd::At(4))]
        );
    }

    #[test]
    fn un_bloque_con_dolar_llega_al_fin_de_cada_linea() {
        assert_eq!(
            spans_with_goal("ab\ncdef\ng", 1, 7, BLOCK, Goal::LineEnd),
            [(0, 1, SpanEnd::At(2)), (1, 1, SpanEnd::At(4))]
        );
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

    /// El patrón que deja `/` en el motor, como lo recibe la UI.
    fn searched(source: &str) -> Pattern {
        let mut vim = Vim::new();
        let mut buffer = Buffer::from("");
        let mut selections = Selections::single(Selection::cursor(ByteOffset::ZERO));
        let keys = std::iter::once('/').chain(source.chars()).map(Key::Char);
        for key in keys.chain([Key::Enter]) {
            vim.feed(key, &mut buffer, &mut selections).unwrap();
        }
        vim.search_highlight().unwrap().clone()
    }

    fn matches(
        pattern: &str,
        text: &str,
        visible: Range<usize>,
        cursor: usize,
    ) -> Vec<(usize, usize, usize, bool)> {
        let buffer = Buffer::from(text);
        search_spans(
            &searched(pattern),
            &buffer,
            visible,
            ByteOffset::new(cursor),
        )
        .into_iter()
        .map(|found| {
            let SpanEnd::At(end) = found.span.end else {
                panic!("{found:?}")
            };
            (found.span.line, found.span.start, end, found.current)
        })
        .collect()
    }

    #[test]
    fn solo_las_coincidencias_de_las_lineas_visibles() {
        let text = "uno\nuno dos uno\ntres\nuno\n";
        assert_eq!(
            matches("uno", text, 1..3, 0),
            [(1, 0, 3, false), (1, 8, 11, false)]
        );
        assert_eq!(matches("uno", text, 3..40, 0), [(3, 0, 3, false)]);
        assert_eq!(matches("nada", text, 0..4, 0), []);
    }

    #[test]
    fn la_coincidencia_del_cursor_es_la_actual() {
        let text = "año uno año\n";
        let current = |cursor| {
            matches("año", text, 0..1, cursor)
                .iter()
                .map(|found| found.3)
                .collect::<Vec<_>>()
        };
        assert_eq!(current(0), [true, false]);
        assert_eq!(current(2), [true, false]);
        assert_eq!(current(4), [false, false]);
        assert_eq!(current(9), [false, true]);
    }

    #[test]
    fn las_coincidencias_vacias_no_se_pintan() {
        assert_eq!(matches("^", "a\nb\n", 0..2, 0), []);
        assert_eq!(matches("x*", "axxb\n", 0..1, 1), [(0, 1, 3, true)]);
    }

    #[test]
    fn solo_las_lineas_visibles() {
        let buffer = Buffer::from("a\nb\nc\nd\n");
        let selection = Selection {
            anchor: ByteOffset::new(0),
            head: ByteOffset::new(6),
            goal: Goal::None,
        };
        let lines: Vec<usize> =
            selection_spans(&buffer, selection, LINE, &Viewport::default(), 1..3)
                .iter()
                .map(|span| span.line)
                .collect();
        assert_eq!(lines, [1, 2]);
    }
}
