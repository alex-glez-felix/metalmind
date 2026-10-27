//! Las líneas visibles con su estilo: dónde va cada una según lo que mide,
//! con qué letra y en qué colores, sus fórmulas y el fondo de los bloques
//! de código.

use std::ops::Range;

use gpui_kit::{Bounds, Font, PaintQuad, Pixels, Window, fill, point, px, size};
use metalmind_syntax::{LineKind, LineStyle, MathSpot};
use metalmind_vim::LineHeights as _;

mod builder;
mod run_slicer;

use self::builder::{Builder, scaled};
use self::run_slicer::RunSlicer;
use super::layout::{LaidLine, PlacedFormula, Viewport};
use super::links::Links;
use super::markdown::runs;
use super::replaced::{Part, parts};
use super::rows::place;
use super::text::{Faces, text_runs};
use crate::math::{FormulaBox, Formulas, Geometry, ImageKey, Look, MathError};
use crate::theme::Theme;
use crate::workspace::{PaneView, SourceLines};

/// La letra del editor, resuelta para este frame.
pub struct Typeface {
    pub font: Font,
    pub size: Pixels,
}

/// Lo que mide la línea `line` en píxeles, con lo que mide el motor.
pub fn line_height(view: &PaneView<'_>, line: usize) -> f32 {
    units(view.layout().height(line))
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
    let styles = view.document.state().styles();
    let layout = view.layout();
    let height = f32::from(viewport.bounds.size.height);
    let placed = place(viewport.scroll, view.buffer().last_line(), height, |line| {
        units(layout.height(line))
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
        let measured = layout.rows(placed.line);
        let shown = layout.screen_line(placed.line);
        let mut top = 0.0;
        let rows = measured
            .rows
            .iter()
            .enumerate()
            .map(|(row, &height)| {
                let source = shown.as_ref().map_or(0..text.len(), |shown| shown.row(row));
                let row = RowBox {
                    source,
                    top: px(top),
                    height: px(units(height)),
                };
                top += units(height);
                row
            })
            .collect();
        let line = Line {
            number: placed.line,
            text: &text,
            style: styles.and_then(|styles| styles.line(placed.line)),
            show_source: math.source.contains(placed.line),
            origin: point(
                viewport.bounds.left() - viewport.left,
                viewport.bounds.top() + px(placed.top),
            ),
            height: px(placed.height),
            rows,
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

/// Una fila de pantalla antes de darle forma: sus bytes, dónde empieza
/// desde arriba de la línea y lo que mide su texto.
struct RowBox {
    source: Range<usize>,
    top: Pixels,
    height: Pixels,
}

/// Una línea visible antes de darle forma.
struct Line<'a> {
    number: usize,
    text: &'a str,
    style: Option<&'a LineStyle>,
    show_source: bool,
    origin: gpui_kit::Point<Pixels>,
    height: Pixels,
    rows: Vec<RowBox>,
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
                (
                    spot,
                    metrics.and_then(|metrics| math.geometry.formula(metrics)),
                )
            })
            .collect();
        // El alto sale de lo medido, como lo mide el motor; lo que no se
        // pudo pintar se ve después como una fórmula que no compone.
        let measured = with_render_errors(measured, math);
        let kind = self.style.map(|style| style.kind);
        let faces = Faces::new(typeface.font.clone(), typeface.size, theme);
        let (font_size, weight) = faces.line(kind);
        let failed = self.failed(&measured);
        let runs = runs(
            self.text.len(),
            self.style,
            |spot| links.resolves(spot),
            &failed,
        );
        let runs = text_runs(&runs, kind, &faces, weight, theme);
        let split = if self.show_source {
            vec![Part::Text(0..self.text.len())]
        } else {
            parts(self.text.len(), spots, |index| {
                measured
                    .get(index)
                    .is_some_and(|(spot, formula)| formula.is_ok() && !self.splits(spot))
            })
        };
        let mut builder = Builder {
            line: self,
            math,
            laid: LaidLine {
                line: self.number,
                origin: self.origin,
                height: self.height,
                font_size,
                rows: Vec::with_capacity(self.rows.len()),
                formulas: Vec::new(),
                errors: Vec::new(),
                code_block: matches!(kind, Some(LineKind::Fence(_))),
            },
            keys: Vec::new(),
            baseline: scaled(math.geometry, font_size / typeface.size),
        };
        let mut slicer = RunSlicer::new(&runs);
        for row in &self.rows {
            builder.row(row, &split, &measured, &mut slicer, window);
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

    /// Si la fuente de `spot` quedó partida en varias filas: no cabía en
    /// una, así que se ve como texto en vez de una fórmula en la primera
    /// fila y huecos en las demás.
    fn splits(&self, spot: &MathSpot) -> bool {
        let row_of = |byte: usize| self.rows.iter().position(|row| row.source.contains(&byte));
        row_of(spot.range.start) != row_of(spot.range.end.saturating_sub(1))
    }

    /// Lo que miden las filas, sin lo que suma debajo un bloque.
    fn rows_height(&self) -> Pixels {
        self.rows.last().map_or(px(0.), |row| row.top + row.height)
    }

    /// Dónde va la imagen de una fórmula con su borde izquierdo en `left` y
    /// el de arriba en `top`, desde el origen de la línea, con el margen
    /// `padding` con que se pinta alrededor.
    fn place_formula(
        &self,
        formula: FormulaBox,
        left: Pixels,
        top: Pixels,
        padding: Pixels,
    ) -> PlacedFormula {
        let origin = point(self.origin.x + left, self.origin.y + top);
        let area = size(px(formula.width), px(formula.total()));
        PlacedFormula {
            bounds: Bounds::new(origin, area).dilate(padding),
            image: None,
        }
    }
}

/// Las fórmulas que se midieron pero no se pudieron pintar con este
/// aspecto, como las que no componen.
fn with_render_errors<'s>(measured: Vec<Measured<'s>>, math: &MathFrame<'_>) -> Vec<Measured<'s>> {
    measured
        .into_iter()
        .map(|(spot, formula)| {
            let key = || ImageKey::new(&spot.latex, spot.display, math.look);
            match formula {
                Ok(_) => match math.formulas.render_error(&key()) {
                    Some(error) => (spot, Err(error)),
                    None => (spot, formula),
                },
                Err(_) => (spot, formula),
            }
        })
        .collect()
}

/// El fondo de cada línea de un bloque de código, de `left` a `right`.
pub fn code_block_quads(
    lines: &[LaidLine],
    left: Pixels,
    right: Pixels,
    theme: &Theme,
) -> Vec<PaintQuad> {
    lines
        .iter()
        .filter(|laid| laid.code_block)
        .map(|laid| {
            fill(
                Bounds::new(point(left, laid.origin.y), size(right - left, laid.height)),
                theme.syntax.code_block_background,
            )
        })
        .collect()
}
