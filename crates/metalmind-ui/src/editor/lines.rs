//! Las líneas visibles con su estilo: dónde va cada una según lo que mide,
//! con qué letra y en qué colores, sus fórmulas y el fondo de los bloques
//! de código.

use std::ops::Range;

use gpui_kit::{
    Bounds, Font, PaintQuad, Pixels, SharedString, TextRun, Window, fill, point, px, size,
};
use metalmind_syntax::{LineKind, LineStyle, MathPlace, MathSpot};
use metalmind_vim::{Layout, LineHeights};

use super::layout::{FormulaError, LaidLine, Piece, PlacedFormula, Viewport};
use super::links::Links;
use super::markdown::runs;
use super::replaced::{Part, parts};
use super::rows::place;
use super::text::{line_font, text_runs};
use crate::math::{FormulaBox, Formulas, Geometry, ImageKey, Look, MathError};
use crate::theme::Theme;
use crate::workspace::{PaneView, SourceLines};

/// La letra del editor, resuelta para este frame.
pub struct Typeface {
    pub font: Font,
    pub size: Pixels,
}

/// Lo que mide la línea `line` en píxeles: del layout del documento o, sin
/// él, una línea de texto.
pub fn line_height(view: &PaneView<'_>, line_height: Pixels, line: usize) -> f32 {
    match view.document.layout() {
        Some(layout) => units(layout.height(line)),
        None => f32::from(line_height),
    }
}

fn units(value: usize) -> f32 {
    f32::from(u16::try_from(value).unwrap_or(u16::MAX))
}

/// Con qué se pintan las fórmulas en este frame.
pub struct MathFrame<'a> {
    pub formulas: &'a Formulas,
    pub geometry: Geometry,
    pub look: Look,
    pub source: SourceLines,
}

/// Una imagen que la línea `line` (índice en lo pintado) espera para su
/// fórmula `formula`.
pub struct ImageRequest {
    pub line: usize,
    pub formula: usize,
    pub key: ImageKey,
}

/// Las líneas visibles y las imágenes de fórmulas que necesitan.
pub struct Shaped {
    pub lines: Vec<LaidLine>,
    pub images: Vec<ImageRequest>,
}

pub fn shape_visible_lines(
    view: &PaneView<'_>,
    viewport: &Viewport,
    typeface: &Typeface,
    theme: &Theme,
    links: &Links,
    math: &MathFrame<'_>,
    window: &Window,
) -> Shaped {
    let styles = view.document.layout().map(Layout::styles);
    let height = f32::from(viewport.bounds.size.height);
    let placed = place(viewport.scroll, view.buffer().last_line(), height, |line| {
        line_height(view, viewport.line_height, line)
    });
    let mut shaped = Shaped {
        lines: Vec::with_capacity(placed.len()),
        images: Vec::new(),
    };
    for placed in placed {
        let text = match view.document.line_text(placed.line) {
            Ok(text) => text,
            Err(error) => {
                log::error!("línea {} ilegible: {error}", placed.line);
                continue;
            }
        };
        let style = styles.and_then(|styles| styles.line(placed.line));
        let line = Line {
            number: placed.line,
            text: &text,
            style,
            show_source: math.source.contains(placed.line),
            origin: point(
                viewport.bounds.left(),
                viewport.bounds.top() + px(placed.top),
            ),
            height: px(placed.height),
            width: viewport.bounds.size.width,
        };
        let index = shaped.lines.len();
        let (laid, keys) = line.lay(typeface, theme, links, math, window);
        shaped
            .images
            .extend(keys.into_iter().map(|(formula, key)| ImageRequest {
                line: index,
                formula,
                key,
            }));
        shaped.lines.push(laid);
    }
    shaped
}

/// Una línea visible antes de darle forma.
struct Line<'a> {
    number: usize,
    text: &'a str,
    style: Option<&'a LineStyle>,
    show_source: bool,
    origin: gpui_kit::Point<Pixels>,
    height: Pixels,
    /// El ancho del área de texto, para centrar un bloque.
    width: Pixels,
}

/// Una fórmula de la línea con lo que mide, o por qué no compone.
type Measured<'s> = (&'s MathSpot, Result<FormulaBox, MathError>);

impl Line<'_> {
    fn lay(
        &self,
        typeface: &Typeface,
        theme: &Theme,
        links: &Links,
        math: &MathFrame<'_>,
        window: &Window,
    ) -> (LaidLine, Vec<(usize, ImageKey)>) {
        let spots = self.style.map_or(&[][..], |style| &style.math);
        let measured: Vec<Measured<'_>> = spots
            .iter()
            .map(|spot| {
                let metrics = math.formulas.metrics(&spot.latex, spot.display);
                (spot, metrics.map(|metrics| math.geometry.formula(metrics)))
            })
            .collect();
        let kind = self.style.map(|style| style.kind);
        let (font_size, weight) = line_font(kind, typeface.size, theme);
        let failed = self.failed(&measured);
        let runs = runs(
            self.text.len(),
            self.style,
            |spot| links.resolves(spot),
            &failed,
        );
        let runs = text_runs(&runs, kind, &typeface.font, weight, theme);
        let split = if self.show_source {
            vec![Part::Text(0..self.text.len())]
        } else {
            parts(self.text.len(), spots, |index| {
                measured
                    .get(index)
                    .is_some_and(|(_, formula)| formula.is_ok())
            })
        };
        let mut builder = Builder {
            line: self,
            math,
            laid: LaidLine {
                line: self.number,
                origin: self.origin,
                height: self.height,
                text_height: self.text_height(&measured, math.geometry),
                font_size,
                pieces: Vec::with_capacity(split.len()),
                formulas: Vec::new(),
                errors: Vec::new(),
                code_block: matches!(kind, Some(LineKind::Fence(_))),
            },
            keys: Vec::new(),
            x: px(0.),
            baseline: scaled(math.geometry, font_size / typeface.size),
        };
        for part in split {
            match part {
                Part::Text(range) => builder.text(range, &runs, window),
                Part::Formula { spot, source } => {
                    if let Some((spot, Ok(formula))) = measured.get(spot) {
                        builder.formula(spot, *formula, source);
                    }
                }
            }
        }
        for (spot, formula) in &measured {
            match formula {
                Ok(formula) if spot.below() => builder.below(spot, *formula),
                Err(error) if !self.show_source => builder.error(spot, error),
                _ => {}
            }
        }
        (builder.laid, builder.keys)
    }

    /// Los bytes de las fórmulas que no componen, que se ven en el color de
    /// error; con la fuente a la vista, se escriben y no se marcan.
    fn failed(&self, measured: &[Measured<'_>]) -> Vec<Range<usize>> {
        if self.show_source {
            return Vec::new();
        }
        measured
            .iter()
            .filter(|(_, formula)| formula.is_err())
            .map(|(spot, _)| spot.range.clone())
            .collect()
    }

    /// El alto de la fila del texto: sin lo que suman los bloques debajo.
    fn text_height(&self, measured: &[Measured<'_>], geometry: Geometry) -> Pixels {
        let below: f32 = measured
            .iter()
            .filter(|(spot, _)| spot.below())
            .filter_map(|(spot, formula)| {
                let formula = formula.as_ref().ok()?;
                Some(units(geometry.rows(*formula, spot.place)))
            })
            .sum();
        (self.height - px(below)).max(px(0.))
    }

    /// Dónde va la imagen de una fórmula con su borde izquierdo en `left` y
    /// el de arriba en `top`, desde el origen de la línea, contando el
    /// margen con que RaTeX la pinta.
    fn place_formula(&self, formula: FormulaBox, left: Pixels, top: Pixels) -> PlacedFormula {
        let padding = px(crate::math::PADDING);
        PlacedFormula {
            bounds: Bounds::new(
                point(
                    self.origin.x + left - padding,
                    self.origin.y + top - padding,
                ),
                size(
                    px(formula.width) + padding * 2.0,
                    px(formula.total()) + padding * 2.0,
                ),
            ),
            image: None,
        }
    }
}

/// Arma una línea de izquierda a derecha.
struct Builder<'l, 'm> {
    line: &'l Line<'l>,
    math: &'l MathFrame<'m>,
    laid: LaidLine,
    /// Las imágenes que esperan sus fórmulas, por índice en `laid.formulas`.
    keys: Vec<(usize, ImageKey)>,
    /// Hasta dónde llega lo armado.
    x: Pixels,
    /// La letra de la línea, para su línea base.
    baseline: Geometry,
}

impl Builder<'_, '_> {
    fn text(&mut self, range: Range<usize>, runs: &[TextRun], window: &Window) {
        let text = SharedString::from(self.line.text[range.clone()].to_owned());
        let runs = slice_runs(runs, range.clone());
        let shaped = window
            .text_system()
            .shape_line(text, self.laid.font_size, &runs, None);
        let width = shaped.width();
        self.laid.pieces.push(Piece {
            source: range,
            x: self.x,
            width,
            text: Some(shaped),
        });
        self.x += width;
    }

    /// Una fórmula en lugar de su fuente: alineada con el texto o, si es lo
    /// único de la línea, centrada.
    fn formula(&mut self, spot: &MathSpot, formula: FormulaBox, source: Range<usize>) {
        let width = px(formula.width);
        let row = f32::from(self.laid.text_height);
        let (left, top) = if spot.place == MathPlace::Alone {
            let top = self.math.geometry.top(formula, spot.place, row);
            (centered(self.line.width, width), top)
        } else {
            (self.x, self.baseline.baseline(row) - formula.height)
        };
        self.laid.pieces.push(Piece {
            source,
            x: left,
            width,
            text: None,
        });
        self.x = left + width;
        self.place(spot, formula, left, px(top));
    }

    /// La fórmula de un bloque, centrada debajo de la fila del texto.
    fn below(&mut self, spot: &MathSpot, formula: FormulaBox) {
        let space = f32::from(self.laid.height - self.laid.text_height);
        let top = self.laid.text_height + px(self.math.geometry.top(formula, spot.place, space));
        let left = centered(self.line.width, px(formula.width));
        self.place(spot, formula, left, top);
    }

    fn place(&mut self, spot: &MathSpot, formula: FormulaBox, left: Pixels, top: Pixels) {
        let key = ImageKey::new(&spot.latex, spot.display, self.math.look);
        self.keys.push((self.laid.formulas.len(), key));
        let placed = self.line.place_formula(formula, left, top);
        self.laid.formulas.push(placed);
    }

    fn error(&mut self, spot: &MathSpot, error: &MathError) {
        let left = self.laid.x_for_index(spot.range.start);
        let right = self.laid.x_for_index(spot.range.end);
        let origin = self.laid.origin;
        self.laid.errors.push(FormulaError {
            bounds: Bounds::new(
                point(origin.x + left, origin.y),
                size(right - left, self.laid.text_height),
            ),
            message: error.to_string().into(),
        });
    }
}

/// Centrada en `area`; si no cabe, desde el borde izquierdo, como el texto
/// que pasa del borde.
fn centered(area: Pixels, width: Pixels) -> Pixels {
    ((area - width) / 2.0).max(px(0.))
}

/// La letra de un heading es más grande: su línea base baja con ella.
fn scaled(geometry: Geometry, factor: f32) -> Geometry {
    Geometry {
        ascent: geometry.ascent * factor,
        descent: geometry.descent * factor,
        ..geometry
    }
}

/// Los `runs` que caen en `range`, recortados a él.
fn slice_runs(runs: &[TextRun], range: Range<usize>) -> Vec<TextRun> {
    let mut out = Vec::new();
    let mut at = 0;
    for run in runs {
        let (start, end) = (at, at + run.len);
        at = end;
        let (from, to) = (start.max(range.start), end.min(range.end));
        if from < to {
            out.push(TextRun {
                len: to - from,
                ..run.clone()
            });
        }
    }
    out
}

/// El fondo de cada línea de un bloque de código, hasta `right`.
pub fn code_block_quads(lines: &[LaidLine], right: Pixels, theme: &Theme) -> Vec<PaintQuad> {
    lines
        .iter()
        .filter(|laid| laid.code_block)
        .map(|laid| {
            fill(
                Bounds::new(laid.origin, size(right - laid.origin.x, laid.height)),
                theme.markdown.code_block_background,
            )
        })
        .collect()
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
