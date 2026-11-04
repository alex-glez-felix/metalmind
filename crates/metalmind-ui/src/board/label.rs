//! Los textos de la pizarra: se componen al tamaño en pantalla y se reparten en
//! el área de su elemento. Lo que `shape_text` ya compuso sale de su caché. El
//! texto de una caja abierta se reparte igual: no brinca al abrirla.

use std::ops::Range;
use std::sync::Arc;

use gpui_kit::{
    App, Bounds, ContentMask, Font, Hsla, LineLayout, Pixels, Point as ScreenPoint, SharedString,
    TextAlign, TextRun, UnderlineStyle, Window, WrappedLine, font, point, px, size,
};
use metalmind_board::{Point, Rect};

use super::paint::{Frame, fill_behind};
use super::scene::{Label, LabelKind};
use crate::theme::Theme;

/// Medio píxel: un zoom continuo reutiliza las líneas ya compuestas en vez de
/// componerlas de nuevo en cada tamaño (09 §3.3).
const SIZE_STEPS_PER_PIXEL: f32 = 2.0;
const HALF: f32 = 0.5;

struct Look {
    /// En unidades de la pizarra.
    size: Pixels,
    strong: bool,
    color: Hsla,
    align: TextAlign,
    centered: bool,
    /// El texto no sale del área, aunque no tenga por dónde partirse.
    clipped: bool,
    max_lines: Option<usize>,
}

impl Look {
    fn of(kind: LabelKind, theme: &Theme) -> Self {
        let layout = &theme.layout;
        let body = Self {
            size: layout.board_text,
            strong: false,
            color: theme.ui.board_text,
            align: TextAlign::Center,
            centered: true,
            clipped: true,
            max_lines: None,
        };
        match kind {
            LabelKind::Body => body,
            LabelKind::Free => Self {
                align: TextAlign::Left,
                centered: false,
                clipped: false,
                ..body
            },
            LabelKind::Heading => Self {
                size: layout.board_text_heading,
                strong: true,
                align: TextAlign::Left,
                centered: false,
                clipped: false,
                ..body
            },
            LabelKind::CardTitle => Self {
                strong: true,
                align: TextAlign::Left,
                centered: false,
                max_lines: Some(1),
                ..body
            },
            LabelKind::CardPath => Self {
                size: layout.board_text_small,
                color: theme.ui.board_text_muted,
                align: TextAlign::Left,
                centered: false,
                ..body
            },
            LabelKind::Group => Self {
                color: theme.ui.board_text_muted,
                strong: true,
                align: TextAlign::Left,
                centered: false,
                clipped: false,
                max_lines: Some(1),
                ..body
            },
            LabelKind::Edge => Self {
                size: layout.board_text_small,
                clipped: false,
                ..body
            },
        }
    }

    fn font(&self, theme: &Theme) -> Font {
        Font {
            weight: if self.strong {
                theme.text.strong
            } else {
                theme.text.normal
            },
            ..font(theme.font_family.clone())
        }
    }
}

/// Un texto ya compuesto y repartido en su área.
pub(super) struct Laid {
    lines: Vec<WrappedLine>,
    look: Look,
    line_height: Pixels,
    text_size: Pixels,
    top: Pixels,
    area: Bounds<Pixels>,
    text_box: Bounds<Pixels>,
    kind: LabelKind,
}

/// Compone `text` al tamaño en pantalla; `None` si es demasiado chico para leerse.
/// `underlined` es lo que va subrayado: el texto provisional del IME.
pub(super) fn lay_out(
    text: SharedString,
    underlined: Option<Range<usize>>,
    kind: LabelKind,
    label_area: Rect,
    frame: &Frame<'_>,
    window: &Window,
) -> Option<Laid> {
    let theme = frame.theme;
    let look = Look::of(kind, theme);
    let text_size = (f32::from(look.size) * frame.camera.zoom * SIZE_STEPS_PER_PIXEL).round()
        / SIZE_STEPS_PER_PIXEL;
    if text_size < f32::from(theme.layout.board_min_text) {
        return None;
    }
    let area = frame.bounds_of(label_area);
    let line_height = px(text_size * theme.layout.board_line_height.0);
    let font = look.font(theme);
    let runs = text_runs(text.len(), underlined, &look, &font, theme.stroke.text_line);
    let lines = window
        .text_system()
        .shape_text(
            text,
            px(text_size),
            &runs,
            Some(area.size.width),
            look.max_lines,
        )
        .ok()?
        .into_vec();
    let height: Pixels = lines.iter().map(|line| line.size(line_height).height).sum();
    let top = if look.centered {
        let middle = frame
            .on_screen(Point::new(label_area.x, label_area.center().y))
            .y;
        middle - height.max(line_height) * HALF
    } else {
        area.origin.y
    };
    let text_box = Bounds::new(point(area.origin.x, top), size(area.size.width, height));
    Some(Laid {
        lines,
        look,
        line_height,
        text_size: px(text_size),
        top,
        area,
        text_box,
        kind,
    })
}

fn text_runs(
    len: usize,
    underlined: Option<Range<usize>>,
    look: &Look,
    font: &Font,
    thickness: Pixels,
) -> Vec<TextRun> {
    let run = |len: usize, underline: Option<UnderlineStyle>| TextRun {
        len,
        font: font.clone(),
        color: look.color,
        background_color: None,
        underline,
        strikethrough: None,
    };
    let Some(range) = underlined else {
        return vec![run(len, None)];
    };
    vec![
        run(range.start, None),
        run(
            range.len(),
            Some(UnderlineStyle {
                color: Some(look.color),
                thickness,
                wavy: false,
            }),
        ),
        run(len - range.end, None),
    ]
}

impl Laid {
    pub(super) fn line_height(&self) -> Pixels {
        self.line_height
    }

    /// Lo que mide una `M`: el cursor de bloque sobre el fin de línea.
    pub(super) fn em(&self) -> Pixels {
        self.text_size * HALF
    }

    /// `mask`: el texto no sale de ahí.
    pub(super) fn paint(&self, mask: Option<Bounds<Pixels>>, window: &mut Window, cx: &mut App) {
        let mask = mask.map(|bounds| ContentMask { bounds });
        window.with_content_mask(mask, |window| {
            let mut y = self.top;
            for line in &self.lines {
                let painted = line.paint(
                    point(self.area.origin.x, y),
                    self.line_height,
                    self.look.align,
                    Some(self.text_box),
                    window,
                    cx,
                );
                if let Err(error) = painted {
                    log::error!("no se pudo pintar un texto de la pizarra: {error:#}");
                    return;
                }
                y += line.size(self.line_height).height;
            }
        });
    }

    /// Las filas de pantalla, para ubicar el cursor y traducir un clic.
    pub(super) fn rows(&self) -> TextRows {
        let mut rows = Vec::new();
        let mut y = self.top;
        let mut line_start = 0;
        for line in &self.lines {
            let layout = line.unwrapped_layout.clone();
            let mut row_start = 0;
            let boundaries = line
                .wrap_boundaries
                .iter()
                .map(|boundary| layout.runs[boundary.run_ix].glyphs[boundary.glyph_ix].index);
            for row_end in boundaries.chain([line.len()]) {
                let (start_x, end_x) = (layout.x_for_index(row_start), layout.x_for_index(row_end));
                let left = match self.look.align {
                    TextAlign::Center => {
                        self.area.origin.x + (self.area.size.width - (end_x - start_x)) * HALF
                    }
                    TextAlign::Right => {
                        self.area.origin.x + self.area.size.width - (end_x - start_x)
                    }
                    TextAlign::Left => self.area.origin.x,
                };
                rows.push(TextRow {
                    layout: layout.clone(),
                    start: line_start + row_start,
                    end: line_start + row_end,
                    line_start,
                    start_x,
                    left,
                    y,
                });
                row_start = row_end;
                y += self.line_height;
            }
            line_start += line.len() + 1;
        }
        if rows.is_empty() {
            rows.push(TextRow {
                layout: Arc::default(),
                start: 0,
                end: 0,
                line_start: 0,
                start_x: px(0.),
                left: match self.look.align {
                    TextAlign::Center => self.area.origin.x + self.area.size.width * HALF,
                    TextAlign::Right | TextAlign::Left => self.area.origin.x,
                },
                y,
            });
        }
        TextRows {
            rows,
            line_height: self.line_height,
        }
    }
}

struct TextRow {
    layout: Arc<LineLayout>,
    start: usize,
    end: usize,
    line_start: usize,
    start_x: Pixels,
    left: Pixels,
    y: Pixels,
}

/// Dónde cayó cada fila de un texto en pantalla.
pub(super) struct TextRows {
    rows: Vec<TextRow>,
    line_height: Pixels,
}

impl TextRows {
    /// Arriba a la izquierda del byte `index`. Al final de una fila partida el
    /// byte es el inicio de la siguiente.
    pub(super) fn place(&self, index: usize) -> Option<(Pixels, Pixels)> {
        let row = self
            .rows
            .iter()
            .find(|row| index >= row.start && index < row.end)
            .or_else(|| {
                self.rows
                    .iter()
                    .rev()
                    .find(|row| index >= row.start && index <= row.end)
            })?;
        let x = row.layout.x_for_index(index - row.line_start) - row.start_x;
        Some((row.left + x, row.y))
    }

    /// Los tramos `(desde x, hasta x, y)` de los bytes `range`, fila por fila.
    pub(super) fn spans(&self, range: Range<usize>) -> Vec<(Pixels, Pixels, Pixels)> {
        self.rows
            .iter()
            .filter(|row| row.start < range.end && range.start <= row.end)
            .map(|row| {
                let from = range.start.max(row.start) - row.line_start;
                let to = range.end.min(row.end) - row.line_start;
                let x = |index| row.left + row.layout.x_for_index(index) - row.start_x;
                (x(from), x(to), row.y)
            })
            .collect()
    }

    /// El byte más cercano a `at`.
    pub(super) fn offset_at(&self, at: ScreenPoint<Pixels>) -> Option<usize> {
        let row = self
            .rows
            .iter()
            .find(|row| at.y < row.y + self.line_height)
            .or(self.rows.last())?;
        let x = at.x - row.left + row.start_x;
        let index = row.layout.closest_index_for_x(x) + row.line_start;
        Some(index.clamp(row.start, row.end))
    }
}

pub fn paint_label(label: &Label, frame: &Frame<'_>, window: &mut Window, cx: &mut App) {
    if label.text.is_empty() {
        return;
    }
    let Some(laid) = lay_out(
        label.text.clone(),
        None,
        label.kind,
        label.area,
        frame,
        window,
    ) else {
        return;
    };
    if laid.kind == LabelKind::Edge {
        let width = laid
            .lines
            .iter()
            .map(|line| line.width())
            .fold(px(0.), Pixels::max);
        fill_behind(laid.text_box, width, frame, window);
    }
    let mask = laid.look.clipped.then_some(laid.area);
    laid.paint(mask, window, cx);
}
