//! Una fila del picker en una sola línea: mide el texto y el detalle con el
//! shaping de GPUI y recorta los dos al ancho que tiene la fila, con
//! [`fit`](super::super::fit), para que nunca se envuelvan ni se encimen.

use std::ops::Range;

use gpui_kit::{
    App, Bounds, ContentMask, ElementId, Font, FontWeight, GlobalElementId, Hsla,
    InspectorElementId, IntoElement, LayoutId, Pixels, ShapedLine, SharedString, Style, TextAlign,
    TextRun, Window, point, px, relative, size,
};
use unicode_segmentation::UnicodeSegmentation as _;

use super::{Overflow, PickerRow};
use crate::picker::fit;
use crate::theme::Theme;

/// Un texto que la fuente del tema siempre tiene: de él sale la línea base.
const REFERENCE_PROBE: &str = "x";

pub struct RowLine {
    row: PickerRow,
    matched: Hsla,
    muted: Hsla,
    strong: FontWeight,
    text_size: Pixels,
    detail_size: Pixels,
    gap: Pixels,
    detail_share: f32,
}

impl RowLine {
    pub fn new(row: PickerRow, theme: &Theme) -> Self {
        Self {
            row,
            matched: theme.picker_match,
            muted: theme.picker_muted,
            strong: theme.weight_strong,
            text_size: theme.ui_font_size,
            detail_size: theme.ui_small_font_size,
            gap: theme.picker_gap,
            detail_share: theme.picker_detail_share,
        }
    }
}

pub struct Shaped {
    text: ShapedLine,
    detail: Option<ShapedLine>,
    /// El ascent y el descent de la fuente del tema: la línea base común.
    reference: (f32, f32),
}

impl IntoElement for RowLine {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

impl gpui_kit::Element for RowLine {
    type RequestLayoutState = ();
    type PrepaintState = Shaped;

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        let mut style = Style::default();
        style.size.width = relative(1.).into();
        style.size.height = relative(1.).into();
        (window.request_layout(style, [], cx), ())
    }

    fn prepaint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        window: &mut Window,
        _cx: &mut App,
    ) -> Self::PrepaintState {
        let text_style = window.text_style();
        let typeface = Typeface {
            font: text_style.font(),
            size: self.text_size,
            color: text_style.color,
            strong: self.strong,
        };
        let detail_typeface = Typeface {
            size: self.detail_size,
            color: self.muted,
            ..typeface.clone()
        };
        let text = Measured::new(
            fit::one_line(&self.row.text),
            &self.row.highlights,
            &typeface,
            self.matched,
            window,
        );
        let detail = self.row.detail.as_deref().map(|detail| {
            Measured::new(
                fit::one_line(detail),
                &[],
                &detail_typeface,
                self.muted,
                window,
            )
        });
        let ellipsis = f32::from(typeface.shape(fit::ELLIPSIS.into(), window).width());
        let detail_ellipsis =
            f32::from(detail_typeface.shape(fit::ELLIPSIS.into(), window).width());
        let (text_budget, detail_budget) = fit::split(
            f32::from(bounds.size.width),
            text.total(),
            detail.as_ref().map(Measured::total),
            f32::from(self.gap),
            self.detail_share,
            detail_ellipsis,
        );
        let focus = self
            .row
            .highlights
            .first()
            .map(|range| text.graphemes_of(range));
        let kept = match self.row.overflow {
            Overflow::Around => {
                fit::around(&text.widths, focus.unwrap_or(0..0), text_budget, ellipsis)
            }
            Overflow::Path => {
                fit::path(&text.widths, &text.slashes(), focus, text_budget, ellipsis)
            }
        };
        // Una línea con la fuente del tema y sin respaldo: mide como medirían
        // las filas sin CJK, que así quedan donde las pondría GPUI.
        let probe = typeface.shape(REFERENCE_PROBE.into(), window);
        let reference = (f32::from(probe.ascent), f32::from(probe.descent));
        Shaped {
            reference,
            text: text.render(&kept, &typeface, self.matched, window),
            detail: detail.zip(detail_budget).map(|(detail, detail_budget)| {
                let kept = fit::path(
                    &detail.widths,
                    &detail.slashes(),
                    None,
                    detail_budget,
                    detail_ellipsis,
                );
                detail.render(&kept, &detail_typeface, self.muted, window)
            }),
        }
    }

    fn paint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        shaped: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        // GPUI centra cada línea en el alto de la fila según su propio ascent
        // y descent, que crecen si entra una fuente de respaldo (CJK): se
        // corren para que las dos se apoyen en la línea base de la fuente del tema.
        let line_height = bounds.size.height;
        let top = |line: &ShapedLine| {
            let shift = fit::baseline_shift(
                shaped.reference,
                f32::from(line.ascent),
                f32::from(line.descent),
            );
            bounds.top() + px(shift)
        };
        let detail_width = shaped.detail.as_ref().map_or(px(0.), ShapedLine::width);
        let text_right = if shaped.detail.is_some() {
            bounds.right() - detail_width - self.gap
        } else {
            bounds.right()
        };
        // El recorte va por grafemas medidos sueltos; el kerning de la línea
        // entera puede sumar una fracción de píxel, que no debe pisar al detalle.
        let text_bounds = Bounds::new(
            point(bounds.left(), bounds.top()),
            size((text_right - bounds.left()).max(px(0.)), bounds.size.height),
        );
        window.with_content_mask(
            Some(ContentMask {
                bounds: text_bounds,
            }),
            |window| {
                let origin = point(bounds.left(), top(&shaped.text));
                paint_line(&shaped.text, origin, line_height, window, cx);
            },
        );
        if let Some(detail) = &shaped.detail {
            let origin = point(bounds.right() - detail_width, top(detail));
            window.with_content_mask(Some(ContentMask { bounds }), |window| {
                paint_line(detail, origin, line_height, window, cx);
            });
        }
    }
}

fn paint_line(
    line: &ShapedLine,
    origin: gpui_kit::Point<Pixels>,
    line_height: Pixels,
    window: &mut Window,
    cx: &mut App,
) {
    if let Err(error) = line.paint(origin, line_height, TextAlign::Left, None, window, cx) {
        log::error!("no se pudo pintar una fila del picker: {error}");
    }
}

#[derive(Clone)]
struct Typeface {
    font: Font,
    size: Pixels,
    color: Hsla,
    /// El peso de lo que coincide.
    strong: FontWeight,
}

impl Typeface {
    fn run(&self, len: usize) -> TextRun {
        TextRun {
            len,
            font: self.font.clone(),
            color: self.color,
            background_color: None,
            underline: None,
            strikethrough: None,
        }
    }

    fn matched_run(&self, len: usize, color: Hsla) -> TextRun {
        let mut run = self.run(len);
        run.font.weight = self.strong;
        run.color = color;
        run
    }

    /// Los tramos de `len` bytes con estilo, con `highlights` resaltados.
    fn runs(&self, len: usize, highlights: &[Range<usize>], matched: Hsla) -> Vec<TextRun> {
        let mut runs = Vec::new();
        let mut at = 0;
        for range in highlights {
            if range.start > at {
                runs.push(self.run(range.start - at));
            }
            runs.push(self.matched_run(range.len(), matched));
            at = range.end;
        }
        if len > at {
            runs.push(self.run(len - at));
        }
        runs
    }

    fn shape(&self, text: SharedString, window: &Window) -> ShapedLine {
        let runs = [self.run(text.len())];
        self.shape_runs(text, &runs, window)
    }

    fn shape_runs(&self, text: SharedString, runs: &[TextRun], window: &Window) -> ShapedLine {
        window.text_system().shape_line(text, self.size, runs, None)
    }
}

/// Un texto con su forma y el ancho de cada grafema.
struct Measured {
    text: String,
    highlights: Vec<Range<usize>>,
    /// Dónde empieza cada grafema, más el final del texto.
    starts: Vec<usize>,
    widths: Vec<f32>,
}

impl Measured {
    fn new(
        text: String,
        highlights: &[Range<usize>],
        typeface: &Typeface,
        matched: Hsla,
        window: &Window,
    ) -> Self {
        let runs = typeface.runs(text.len(), highlights, matched);
        let shaped = typeface.shape_runs(SharedString::from(text.clone()), &runs, window);
        let starts: Vec<usize> = text
            .grapheme_indices(true)
            .map(|(start, _)| start)
            .chain([text.len()])
            .collect();
        let glyphs: Vec<(usize, f32)> = shaped
            .runs
            .iter()
            .flat_map(|run| &run.glyphs)
            .map(|glyph| (glyph.index, f32::from(glyph.position.x)))
            .collect();
        let widths = fit::grapheme_widths(&starts, &glyphs, f32::from(shaped.width()));
        Self {
            text,
            highlights: highlights.to_vec(),
            starts,
            widths,
        }
    }

    fn slashes(&self) -> Vec<usize> {
        fit::slashes(&self.text)
    }

    fn total(&self) -> f32 {
        self.widths.iter().sum()
    }

    /// Los grafemas que tocan los bytes de `range`.
    fn graphemes_of(&self, range: &Range<usize>) -> Range<usize> {
        let start = self
            .starts
            .partition_point(|&at| at <= range.start)
            .saturating_sub(1);
        let end = self.starts.partition_point(|&at| at < range.end);
        start..end.max(start)
    }

    fn render(
        &self,
        kept: &fit::Kept,
        typeface: &Typeface,
        matched: Hsla,
        window: &Window,
    ) -> ShapedLine {
        let (text, highlights) = fit::render(&self.text, &self.starts, kept, &self.highlights);
        let runs = typeface.runs(text.len(), &highlights, matched);
        typeface.shape_runs(text.into(), &runs, window)
    }
}
