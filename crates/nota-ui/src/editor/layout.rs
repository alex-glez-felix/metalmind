use std::ops::Range;

use gpui_kit::{Bounds, Pixels, Point, ShapedLine, point, px};

use super::scroll::{Scroll, to_line};

/// Qué filas caben y dónde empieza cada una, según el scroll.
#[derive(Clone, Copy, Debug)]
pub struct Viewport {
    pub bounds: Bounds<Pixels>,
    pub line_height: Pixels,
    pub first_line: usize,
    /// Desplazamiento de la primera fila, en `(-line_height, 0]`.
    pub offset: Pixels,
}

impl Viewport {
    pub fn new(bounds: Bounds<Pixels>, line_height: Pixels, scroll: Scroll) -> Self {
        #[expect(
            clippy::cast_possible_truncation,
            reason = "la fracción está en [0, 1) y la altura de línea es chica"
        )]
        let hidden = (scroll.fraction() * f64::from(line_height)) as f32;
        Self {
            bounds,
            line_height,
            first_line: scroll.first_line(),
            offset: -px(hidden),
        }
    }

    /// Filas completamente visibles.
    pub fn full_rows(&self) -> usize {
        to_line(f64::from(self.bounds.size.height / self.line_height)).max(1)
    }

    /// Filas a pintar, contando las parciales de arriba y abajo.
    pub fn painted_rows(&self) -> usize {
        self.full_rows() + 2
    }

    pub fn row_origin(&self, row: usize) -> Point<Pixels> {
        let row = f32::from(u16::try_from(row).unwrap_or(u16::MAX));
        point(
            self.bounds.left(),
            self.bounds.top() + self.offset + self.line_height * row,
        )
    }
}

#[derive(Clone, Debug)]
pub struct LaidLine {
    pub line: usize,
    pub origin: Point<Pixels>,
    pub shaped: ShapedLine,
}

/// Lo que se pintó en el último frame; sirve para el ratón y el IME.
#[derive(Clone, Debug)]
pub struct FrameLayout {
    pub viewport: Viewport,
    pub lines: Vec<LaidLine>,
}

impl FrameLayout {
    /// Las líneas pintadas, de la primera a la última.
    pub fn visible_lines(&self) -> Range<usize> {
        match (self.lines.first(), self.lines.last()) {
            (Some(first), Some(last)) => first.line..last.line + 1,
            _ => 0..0,
        }
    }

    /// Las líneas pintadas van en orden, así que se busca en log n: el
    /// resaltado de la búsqueda la llama una vez por coincidencia.
    pub fn line(&self, line: usize) -> Option<&LaidLine> {
        let index = self
            .lines
            .binary_search_by_key(&line, |laid| laid.line)
            .ok()?;
        self.lines.get(index)
    }

    /// Línea y byte dentro de ella más cercanos a `position`; arriba o abajo
    /// del texto pintado se toma la primera o la última fila.
    pub fn hit(&self, position: Point<Pixels>) -> Option<(usize, usize)> {
        let line_height = self.viewport.line_height;
        let laid = self
            .lines
            .iter()
            .find(|laid| position.y < laid.origin.y + line_height)
            .or_else(|| self.lines.last())?;
        let index = laid.shaped.closest_index_for_x(position.x - laid.origin.x);
        Some((laid.line, index))
    }
}

#[cfg(test)]
mod tests {
    use gpui_kit::size;

    use super::*;

    fn layout(lines: &[usize]) -> FrameLayout {
        let bounds = Bounds::new(point(px(0.), px(0.)), size(px(100.), px(100.)));
        FrameLayout {
            viewport: Viewport::new(bounds, px(10.), Scroll::default()),
            lines: lines
                .iter()
                .map(|&line| LaidLine {
                    line,
                    origin: point(px(0.), px(0.)),
                    shaped: ShapedLine::default(),
                })
                .collect(),
        }
    }

    #[test]
    fn encuentra_cada_linea_pintada_y_ninguna_mas() {
        let painted = layout(&[40, 41, 43, 44]);
        for line in [40, 41, 43, 44] {
            assert_eq!(painted.line(line).map(|laid| laid.line), Some(line));
        }
        for line in [0, 39, 42, 45] {
            assert!(painted.line(line).is_none(), "{line}");
        }
        assert_eq!(painted.visible_lines(), 40..45);
        assert_eq!(layout(&[]).visible_lines(), 0..0);
    }
}
