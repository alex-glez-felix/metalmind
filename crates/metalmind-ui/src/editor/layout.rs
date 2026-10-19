use std::ops::Range;

use gpui_kit::{Bounds, Pixels, Point, ShapedLine};

use super::scroll::{Scroll, to_line};

/// El área del texto, lo que mide una línea de texto y desde dónde se ve.
#[derive(Clone, Copy, Debug)]
pub struct Viewport {
    pub bounds: Bounds<Pixels>,
    /// Lo que mide una línea de texto; un heading mide más.
    pub line_height: Pixels,
    pub scroll: Scroll,
}

impl Viewport {
    pub fn new(bounds: Bounds<Pixels>, line_height: Pixels, scroll: Scroll) -> Self {
        Self {
            bounds,
            line_height,
            scroll,
        }
    }

    /// Las líneas de texto que caben enteras.
    pub fn full_rows(&self) -> usize {
        to_line(f64::from(self.bounds.size.height / self.line_height)).max(1)
    }
}

#[derive(Clone, Debug)]
pub struct LaidLine {
    pub line: usize,
    pub origin: Point<Pixels>,
    /// Lo que mide la línea: más que el texto si es un heading.
    pub height: Pixels,
    /// El tamaño de su letra: más grande en un heading.
    pub font_size: Pixels,
    pub shaped: ShapedLine,
    /// Es parte de un bloque de código cercado: lleva fondo.
    pub code_block: bool,
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
        let laid = self
            .lines
            .iter()
            .find(|laid| position.y < laid.origin.y + laid.height)
            .or_else(|| self.lines.last())?;
        let index = laid.shaped.closest_index_for_x(position.x - laid.origin.x);
        Some((laid.line, index))
    }
}

#[cfg(test)]
mod tests {
    use gpui_kit::{point, px, size};

    use super::*;

    fn layout(lines: &[(usize, f32)]) -> FrameLayout {
        let bounds = Bounds::new(point(px(0.), px(0.)), size(px(100.), px(100.)));
        let mut top = 0.;
        FrameLayout {
            viewport: Viewport::new(bounds, px(10.), Scroll::default()),
            lines: lines
                .iter()
                .map(|&(line, height)| {
                    let laid = LaidLine {
                        line,
                        origin: point(px(0.), px(top)),
                        height: px(height),
                        font_size: px(10.),
                        shaped: ShapedLine::default(),
                        code_block: false,
                    };
                    top += height;
                    laid
                })
                .collect(),
        }
    }

    #[test]
    fn encuentra_cada_linea_pintada_y_ninguna_mas() {
        let painted = layout(&[(40, 10.), (41, 10.), (43, 10.), (44, 10.)]);
        for line in [40, 41, 43, 44] {
            assert_eq!(painted.line(line).map(|laid| laid.line), Some(line));
        }
        for line in [0, 39, 42, 45] {
            assert!(painted.line(line).is_none(), "{line}");
        }
        assert_eq!(painted.visible_lines(), 40..45);
        assert_eq!(layout(&[]).visible_lines(), 0..0);
    }

    #[test]
    fn el_raton_cae_en_la_linea_alta_por_toda_su_altura() {
        let painted = layout(&[(0, 10.), (1, 25.), (2, 10.)]);
        let line_at = |y: f32| painted.hit(point(px(0.), px(y))).map(|(line, _)| line);
        assert_eq!(line_at(5.), Some(0));
        assert_eq!(line_at(12.), Some(1));
        assert_eq!(line_at(34.), Some(1));
        assert_eq!(line_at(35.), Some(2));
        assert_eq!(line_at(90.), Some(2));
    }
}
