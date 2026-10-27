//! Arma una línea ya medida: sus filas con su texto y sus fórmulas, en su
//! lugar.

use std::ops::Range;

use gpui_kit::{Bounds, Pixels, SharedString, Window, point, px, size};
use metalmind_syntax::{MathPlace, MathSpot};

use super::run_slicer::RunSlicer;
use super::{Line, MathFrame, Measured, RowBox};
use crate::editor::layout::{FormulaError, LaidLine, LaidRow, Piece};
use crate::editor::replaced::Part;
use crate::math::{FormulaBox, Geometry, ImageKey, MathError, half};

/// Arma una línea de arriba abajo y cada fila de izquierda a derecha.
pub(super) struct Builder<'l, 'm> {
    pub(super) line: &'l Line<'l>,
    pub(super) math: &'l MathFrame<'m>,
    pub(super) laid: LaidLine,
    /// Las imágenes que esperan sus fórmulas, por índice en `laid.formulas`.
    pub(super) keys: Vec<(usize, ImageKey)>,
    /// La letra de la línea, para su línea base.
    pub(super) baseline: Geometry,
}

impl Builder<'_, '_> {
    /// La fila `row` con lo de `split` que cae en ella: el texto recortado
    /// a sus bytes y las fórmulas que empiezan ahí.
    pub(super) fn row(
        &mut self,
        row: &RowBox,
        split: &[Part],
        measured: &[Measured<'_>],
        runs: &mut RunSlicer<'_>,
        window: &Window,
    ) {
        let mut laid = LaidRow {
            source: row.source.clone(),
            top: row.top,
            height: row.height,
            pieces: Vec::new(),
        };
        let mut x = px(0.);
        let (start, end) = (row.source.start, row.source.end);
        for part in split {
            match part {
                Part::Text(range) => {
                    let range = range.start.max(start)..range.end.min(end);
                    if !range.is_empty() || (range.start == start && start == end) {
                        x = self.text(&mut laid, range, x, runs, window);
                    }
                }
                Part::Formula { spot, source } => {
                    let starts_here = row.source.contains(&source.start);
                    if let Some((spot, Ok(formula))) = measured.get(*spot).filter(|_| starts_here) {
                        x = self.formula(&mut laid, spot, *formula, source.clone(), x);
                    }
                }
            }
        }
        self.laid.rows.push(laid);
    }

    fn text(
        &self,
        row: &mut LaidRow,
        range: Range<usize>,
        x: Pixels,
        runs: &mut RunSlicer<'_>,
        window: &Window,
    ) -> Pixels {
        let text = SharedString::from(self.line.text[range.clone()].to_owned());
        let runs = runs.take(range.clone());
        let shaped = window
            .text_system()
            .shape_line(text, self.laid.font_size, &runs, None);
        let width = shaped.width();
        row.pieces.push(Piece {
            source: range,
            x,
            width,
            text: Some(shaped),
        });
        x + width
    }

    /// Una fórmula en lugar de su fuente: alineada con el texto de su fila
    /// o, si es lo único de la línea, centrada.
    fn formula(
        &mut self,
        row: &mut LaidRow,
        spot: &MathSpot,
        formula: FormulaBox,
        source: Range<usize>,
        x: Pixels,
    ) -> Pixels {
        let width = px(formula.width);
        let height = f32::from(row.height);
        let (left, top) = if spot.place == MathPlace::Alone {
            let top = self.math.geometry.top(formula, spot.place, height);
            (centered(self.line.width, width), top)
        } else {
            (x, self.baseline.baseline(height) - formula.height)
        };
        row.pieces.push(Piece {
            source,
            x: left,
            width,
            text: None,
        });
        self.place(spot, formula, left, row.top + px(top));
        left + width
    }

    /// La fórmula de un bloque, centrada debajo de las filas del texto.
    pub(super) fn below(&mut self, spot: &MathSpot, formula: FormulaBox) {
        let rows = self.line.rows_height();
        let space = f32::from(self.laid.height - rows);
        let top = rows + px(self.math.geometry.top(formula, spot.place, space));
        let left = centered(self.line.width, px(formula.width));
        self.place(spot, formula, left, top);
    }

    fn place(&mut self, spot: &MathSpot, formula: FormulaBox, left: Pixels, top: Pixels) {
        let key = ImageKey::new(&spot.latex, spot.display, self.math.look);
        self.keys.push((self.laid.formulas.len(), key));
        let padding = px(self.math.look.padding);
        let placed = self.line.place_formula(formula, left, top, padding);
        self.laid.formulas.push(placed);
    }

    /// Lo que ocupa en su fila la fuente de una fórmula que no compone.
    pub(super) fn error(&mut self, spot: &MathSpot, error: &MathError) {
        let Some(row) = self.laid.row_of(spot.range.start) else {
            return;
        };
        let origin = self.laid.row_origin(row);
        let left = row.x_for_index(spot.range.start);
        let right = if spot.range.end <= row.source.end {
            row.x_for_index(spot.range.end)
        } else {
            row.width()
        };
        let bounds = Bounds::new(
            point(origin.x + left, origin.y),
            size(right - left, row.height),
        );
        self.laid.errors.push(FormulaError {
            bounds,
            message: error.to_string().into(),
        });
    }
}

/// Centrada en `area`; si no cabe, desde el borde izquierdo, como el texto
/// que pasa del borde.
fn centered(area: Pixels, width: Pixels) -> Pixels {
    px(half(f32::from(area - width))).max(px(0.))
}

/// La letra de un heading es más grande: su línea base baja con ella.
pub(super) fn scaled(geometry: Geometry, factor: f32) -> Geometry {
    Geometry {
        ascent: geometry.ascent * factor,
        descent: geometry.descent * factor,
        ..geometry
    }
}

#[cfg(test)]
mod tests {
    use gpui_kit::px;
    use pretty_assertions::assert_eq;

    use super::centered;

    #[test]
    fn una_formula_mas_ancha_que_el_area_empieza_en_el_borde() {
        assert_eq!(centered(px(100.), px(40.)), px(30.));
        assert_eq!(centered(px(100.), px(300.)), px(0.));
    }
}
