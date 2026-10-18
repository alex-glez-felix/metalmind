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

    pub fn line(&self, line: usize) -> Option<&LaidLine> {
        self.lines.iter().find(|laid| laid.line == line)
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
