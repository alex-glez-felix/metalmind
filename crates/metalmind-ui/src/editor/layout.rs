use std::ops::Range;
use std::sync::Arc;

use gpui_kit::{Bounds, Pixels, Point, RenderImage, ShapedLine, SharedString, point, px};

use super::scroll::{Scroll, to_line};

/// El área del texto, lo que mide una línea de texto y desde dónde se ve.
#[derive(Clone, Copy, Debug)]
pub struct Viewport {
    pub bounds: Bounds<Pixels>,
    /// Lo que mide una línea de texto; un heading mide más.
    pub line_height: Pixels,
    pub scroll: Scroll,
    /// Sin `wrap`, lo que la ventana se corrió a lo ancho.
    pub left: Pixels,
}

impl Viewport {
    pub fn new(bounds: Bounds<Pixels>, line_height: Pixels, scroll: Scroll) -> Self {
        Self {
            bounds,
            line_height,
            scroll,
            left: px(0.),
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
    /// Arriba a la izquierda de su primera fila; sin `wrap`, corrido a la
    /// izquierda lo que la ventana se corrió a lo ancho.
    pub origin: Point<Pixels>,
    /// Lo que mide la línea: sus filas y la fórmula de un bloque debajo.
    pub height: Pixels,
    /// El tamaño de su letra: más grande en un heading.
    pub font_size: Pixels,
    /// Las filas de pantalla, de arriba abajo; sin `wrap`, una.
    pub rows: Vec<LaidRow>,
    /// Las fórmulas que se pintan, las de los trozos y la de debajo.
    pub formulas: Vec<PlacedFormula>,
    /// Las fórmulas que no compusieron, con el mensaje para el ratón.
    pub errors: Vec<FormulaError>,
    /// Es parte de un bloque de código cercado: lleva fondo.
    pub code_block: bool,
}

/// Una fila de una línea: los bytes que muestra y dónde va cada trozo.
#[derive(Clone, Debug)]
pub struct LaidRow {
    pub source: Range<usize>,
    /// Desde arriba de la línea.
    pub top: Pixels,
    /// Lo que mide la fila del texto: más si tiene una fórmula alta.
    pub height: Pixels,
    /// De izquierda a derecha: texto y fórmulas que reemplazan su fuente.
    /// Con el cursor en la línea, solo texto.
    pub pieces: Vec<Piece>,
}

/// Un trozo de una fila: los bytes de la fuente que pinta y dónde va.
#[derive(Clone, Debug)]
pub struct Piece {
    pub source: Range<usize>,
    /// Desde el inicio de la fila.
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

impl LaidRow {
    /// Hasta dónde llega lo pintado.
    pub fn width(&self) -> Pixels {
        self.pieces
            .last()
            .map_or(px(0.), |piece| piece.x + piece.width)
    }

    /// Dónde cae el byte `index` de la línea, desde el inicio de la fila.
    /// Dentro de una fórmula, en su borde izquierdo.
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

    /// El byte más cercano a `x`; sobre una fórmula, su principio. A la
    /// derecha de todo, el final de la línea si es la última fila y, si no,
    /// su último carácter: el final de una fila es el principio de la otra.
    pub fn closest_index_for_x(&self, x: Pixels, last: bool) -> usize {
        let Some(piece) = self.pieces.iter().find(|piece| x < piece.x + piece.width) else {
            return if last {
                self.source.end
            } else {
                self.last_char()
            };
        };
        match &piece.text {
            Some(text) => piece.source.start + text.closest_index_for_x(x - piece.x),
            None => piece.source.start,
        }
    }

    fn last_char(&self) -> usize {
        let Some(piece) = self.pieces.last() else {
            return self.source.start;
        };
        let inside = piece.text.as_ref().and_then(|text| {
            let glyphs = text.runs.iter().flat_map(|run| &run.glyphs);
            glyphs.map(|glyph| glyph.index).next_back()
        });
        piece.source.start + inside.unwrap_or(0)
    }
}

impl LaidLine {
    /// Los bytes de la línea.
    pub fn len(&self) -> usize {
        self.rows.last().map_or(0, |row| row.source.end)
    }

    /// La fila del byte `index`: el que empieza una fila es de ella, y el
    /// fin de línea, de la última.
    pub fn row_index(&self, index: usize) -> usize {
        self.rows
            .partition_point(|row| row.source.start <= index)
            .saturating_sub(1)
    }

    pub fn row_of(&self, index: usize) -> Option<&LaidRow> {
        self.rows.get(self.row_index(index))
    }

    /// Arriba a la izquierda de `row`, en la ventana.
    pub fn row_origin(&self, row: &LaidRow) -> Point<Pixels> {
        point(self.origin.x, self.origin.y + row.top)
    }

    /// Dónde cae el byte `index` en la ventana, arriba de su fila, y lo
    /// que mide esa fila.
    pub fn place(&self, index: usize) -> Option<(Point<Pixels>, Pixels)> {
        let row = self.row_of(index)?;
        let origin = self.row_origin(row);
        Some((
            point(origin.x + row.x_for_index(index), origin.y),
            row.height,
        ))
    }

    /// El byte más cercano a `position`: arriba o abajo de las filas, la
    /// primera o la última.
    pub fn closest_index(&self, position: Point<Pixels>) -> usize {
        let y = position.y - self.origin.y;
        let last = self.rows.len().saturating_sub(1);
        let index = self
            .rows
            .iter()
            .position(|row| y < row.top + row.height)
            .unwrap_or(last);
        self.rows.get(index).map_or(0, |row| {
            row.closest_index_for_x(position.x - self.origin.x, index == last)
        })
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
    /// del texto pintado se toma la primera o la última fila. A los lados del
    /// área del texto, lo que se ve en su borde: sin `wrap`, una línea larga
    /// sigue fuera de ella, recortada.
    pub fn hit(&self, position: Point<Pixels>) -> Option<(usize, usize)> {
        let laid = self
            .lines
            .iter()
            .find(|laid| position.y < laid.origin.y + laid.height)
            .or_else(|| self.lines.last())?;
        let bounds = self.viewport.bounds;
        let inside = point(position.x.clamp(bounds.left(), bounds.right()), position.y);
        Some((laid.line, laid.closest_index(inside)))
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

    fn row(source: Range<usize>, top: f32, height: f32) -> LaidRow {
        LaidRow {
            source,
            top: px(top),
            height: px(height),
            pieces: Vec::new(),
        }
    }

    fn line(line: usize, top: f32, rows: Vec<LaidRow>) -> LaidLine {
        LaidLine {
            line,
            origin: point(px(0.), px(top)),
            height: rows.iter().map(|row| row.height).sum(),
            font_size: px(10.),
            rows,
            formulas: Vec::new(),
            errors: Vec::new(),
            code_block: false,
        }
    }

    fn layout(lines: &[(usize, f32)]) -> FrameLayout {
        let bounds = Bounds::new(point(px(0.), px(0.)), size(px(100.), px(100.)));
        let mut top = 0.;
        FrameLayout {
            viewport: Viewport::new(bounds, px(10.), Scroll::default()),
            lines: lines
                .iter()
                .map(|&(number, height)| {
                    let laid = line(number, top, vec![row(0..0, 0., height)]);
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

    fn text(source: Range<usize>, x: f32, width: f32) -> Piece {
        Piece {
            width: px(width),
            source,
            x: px(x),
            text: Some(ShapedLine::default()),
        }
    }

    /// `ab` de 10 cada letra, una fórmula de 30 en los bytes 2..7 y `c`.
    fn with_formula() -> LaidRow {
        let mut laid = row(0..8, 0., 10.);
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
        let mut laid = with_formula();
        assert_eq!(laid.width(), px(60.));
        assert_eq!(laid.x_for_index(2), px(20.));
        assert_eq!(laid.x_for_index(4), px(20.), "dentro: el borde izquierdo");
        assert_eq!(laid.x_for_index(7), px(50.));
        assert_eq!(
            laid.x_for_end(4),
            px(50.),
            "un tramo que acaba dentro la cubre"
        );
        assert_eq!(laid.x_for_end(2), px(20.));
        assert_eq!(laid.closest_index_for_x(px(35.), true), 2);
        assert_eq!(laid.closest_index_for_x(px(500.), true), 8);
        laid.pieces.pop();
        laid.source = 0..7;
        assert_eq!(
            laid.closest_index_for_x(px(500.), true),
            7,
            "a la derecha de una fórmula que cierra la fila, su final"
        );
    }

    /// Sin `wrap`, corrida 50 a lo ancho: una línea de 30 letras de 10 que
    /// pasa por los dos lados del área del texto, de 50 a 150.
    fn sideways() -> FrameLayout {
        let bounds = Bounds::new(point(px(50.), px(0.)), size(px(100.), px(100.)));
        let mut laid = line(0, 0., vec![row(0..30, 0., 10.)]);
        laid.rows[0].pieces = (0..30_u16)
            .map(|at| Piece {
                source: usize::from(at)..usize::from(at) + 1,
                x: px(10. * f32::from(at)),
                width: px(10.),
                text: None,
            })
            .collect();
        FrameLayout {
            viewport: Viewport::new(bounds, px(10.), Scroll::default()),
            lines: vec![laid],
        }
    }

    #[test]
    fn un_clic_a_los_lados_del_texto_cae_en_lo_que_se_ve() {
        let painted = sideways();
        let byte_at = |x: f32| painted.hit(point(px(x), px(5.))).map(|(_, byte)| byte);
        assert_eq!(byte_at(75.), Some(7), "dentro, donde cae");
        assert_eq!(byte_at(10.), Some(5), "a la izquierda, el borde");
        assert_eq!(byte_at(400.), Some(15), "a la derecha, el borde");
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

    #[test]
    fn cada_byte_en_su_fila_y_el_raton_en_la_de_su_altura() {
        let laid = line(
            3,
            100.,
            vec![
                row(0..10, 0., 10.),
                row(10..18, 10., 25.),
                row(18..20, 35., 10.),
            ],
        );
        let rows: Vec<usize> = [0, 9, 10, 17, 18, 20]
            .map(|index| laid.row_index(index))
            .to_vec();
        assert_eq!(rows, [0, 0, 1, 1, 2, 2], "el fin de línea, en la última");
        assert_eq!(laid.len(), 20);
        assert_eq!(laid.height, px(45.));
        assert_eq!(
            laid.place(12).map(|(origin, height)| (origin.y, height)),
            Some((px(110.), px(25.)))
        );
        let row_at = |y: f32| laid.row_index(laid.closest_index(point(px(999.), px(y))));
        assert_eq!([95., 105., 130., 136., 300.].map(row_at), [0, 0, 1, 2, 2]);
        assert_eq!(
            laid.closest_index(point(px(999.), px(105.))),
            0,
            "sin trozos, el principio de la fila"
        );
    }
}
