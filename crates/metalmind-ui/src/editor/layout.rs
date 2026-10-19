use std::ops::Range;
use std::sync::Arc;

use gpui_kit::{Bounds, Pixels, Point, RenderImage, ShapedLine, SharedString, px};

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
    /// Lo que mide la línea: más que el texto si es un heading o tiene una
    /// fórmula alta, y con la fórmula de un bloque debajo.
    pub height: Pixels,
    /// Lo que mide la fila del texto: `height` sin lo que suma debajo la
    /// fórmula de un bloque.
    pub text_height: Pixels,
    /// El tamaño de su letra: más grande en un heading.
    pub font_size: Pixels,
    /// La línea de izquierda a derecha: texto y fórmulas que reemplazan su
    /// fuente. Con el cursor encima, un solo trozo de texto.
    pub pieces: Vec<Piece>,
    /// Las fórmulas que se pintan, las de los trozos y la de debajo.
    pub formulas: Vec<PlacedFormula>,
    /// Las fórmulas que no compusieron, con el mensaje para el ratón.
    pub errors: Vec<FormulaError>,
    /// Es parte de un bloque de código cercado: lleva fondo.
    pub code_block: bool,
}

/// Un trozo de la línea: los bytes de la fuente que pinta y dónde va.
#[derive(Clone, Debug)]
pub struct Piece {
    pub source: Range<usize>,
    /// Desde el inicio de la línea.
    pub x: Pixels,
    pub width: Pixels,
    /// El texto; `None` si es una fórmula, que pinta [`PlacedFormula`].
    pub text: Option<ShapedLine>,
}

/// Una fórmula con su lugar en la ventana. Sin imagen todavía, el lugar se
/// reserva vacío.
#[derive(Clone, Debug)]
pub struct PlacedFormula {
    pub bounds: Bounds<Pixels>,
    pub image: Option<Arc<RenderImage>>,
}

/// Una fórmula que no compone: su fuente se ve en el color de error y, con
/// el ratón encima, el mensaje de RaTeX.
#[derive(Clone, Debug)]
pub struct FormulaError {
    pub bounds: Bounds<Pixels>,
    pub message: SharedString,
}

impl LaidLine {
    /// Hasta dónde llega lo pintado.
    pub fn width(&self) -> Pixels {
        self.pieces
            .last()
            .map_or(px(0.), |piece| piece.x + piece.width)
    }

    /// Los bytes de la línea.
    pub fn len(&self) -> usize {
        self.pieces.last().map_or(0, |piece| piece.source.end)
    }

    /// Dónde cae el byte `index`, desde el inicio de la línea. Dentro de
    /// una fórmula, en su borde izquierdo.
    pub fn x_for_index(&self, index: usize) -> Pixels {
        let Some(piece) = self
            .pieces
            .iter()
            .find(|piece| index < piece.source.end)
            .or(self.pieces.last())
        else {
            return px(0.);
        };
        match &piece.text {
            Some(text) => piece.x + text.x_for_index(index.saturating_sub(piece.source.start)),
            None if index >= piece.source.end => piece.x + piece.width,
            None => piece.x,
        }
    }

    /// Dónde termina un tramo que acaba en el byte `index`: dentro de una
    /// fórmula, en su borde derecho, así lo que toca una fórmula la cubre
    /// entera.
    pub fn x_for_end(&self, index: usize) -> Pixels {
        match self
            .pieces
            .iter()
            .find(|piece| piece.source.start < index && index <= piece.source.end)
        {
            Some(piece) if piece.text.is_none() => piece.x + piece.width,
            _ => self.x_for_index(index),
        }
    }

    /// El byte más cercano a `x`; sobre una fórmula, su principio.
    pub fn closest_index_for_x(&self, x: Pixels) -> usize {
        let Some(piece) = self
            .pieces
            .iter()
            .find(|piece| x < piece.x + piece.width)
            .or(self.pieces.last())
        else {
            return 0;
        };
        match &piece.text {
            Some(text) => piece.source.start + text.closest_index_for_x(x - piece.x),
            None => piece.source.start,
        }
    }

    /// El mensaje de la fórmula rota bajo `position`, si la hay.
    pub fn error_at(&self, position: Point<Pixels>) -> Option<&FormulaError> {
        self.errors
            .iter()
            .find(|error| error.bounds.contains(&position))
    }
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
        let index = laid.closest_index_for_x(position.x - laid.origin.x);
        Some((laid.line, index))
    }

    /// La fórmula rota bajo `position`, para mostrar su mensaje.
    pub fn error_at(&self, position: Point<Pixels>) -> Option<&FormulaError> {
        self.lines.iter().find_map(|laid| laid.error_at(position))
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
                        text_height: px(height),
                        font_size: px(10.),
                        pieces: Vec::new(),
                        formulas: Vec::new(),
                        errors: Vec::new(),
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

    /// `ab` de 10 cada letra, una fórmula de 30 en los bytes 2..7 y `c`.
    fn with_formula() -> LaidLine {
        let mut laid = layout(&[(0, 10.)]).lines.remove(0);
        let text = |source: Range<usize>, x: f32, width: f32| Piece {
            width: px(width),
            source,
            x: px(x),
            text: Some(ShapedLine::default()),
        };
        laid.pieces = vec![
            text(0..2, 0., 20.),
            Piece {
                source: 2..7,
                x: px(20.),
                width: px(30.),
                text: None,
            },
            text(7..8, 50., 10.),
        ];
        laid
    }

    #[test]
    fn una_formula_ocupa_su_ancho_y_un_clic_va_a_su_principio() {
        let laid = with_formula();
        assert_eq!((laid.len(), laid.width()), (8, px(60.)));
        assert_eq!(laid.x_for_index(2), px(20.));
        assert_eq!(laid.x_for_index(4), px(20.), "dentro: el borde izquierdo");
        assert_eq!(laid.x_for_index(7), px(50.));
        assert_eq!(
            laid.x_for_end(4),
            px(50.),
            "un tramo que acaba dentro la cubre"
        );
        assert_eq!(laid.x_for_end(2), px(20.));
        assert_eq!(laid.closest_index_for_x(px(35.)), 2);
        assert_eq!(laid.closest_index_for_x(px(500.)), 7);
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
