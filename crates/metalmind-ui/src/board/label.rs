//! Los textos de la pizarra: se componen al tamaño en pantalla y se reparten en
//! el área de su elemento. Lo que `shape_text` ya compuso sale de su caché.

use gpui_kit::{
    App, Bounds, ContentMask, Font, Hsla, Pixels, TextAlign, TextRun, Window, font, point, px, size,
};
use metalmind_board::Point;

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

pub fn paint_label(label: &Label, frame: &Frame<'_>, window: &mut Window, cx: &mut App) {
    let theme = frame.theme;
    let look = Look::of(label.kind, theme);
    let text_size = (f32::from(look.size) * frame.camera.zoom * SIZE_STEPS_PER_PIXEL).round()
        / SIZE_STEPS_PER_PIXEL;
    if text_size < f32::from(theme.layout.board_min_text) {
        return;
    }
    let area = frame.bounds_of(label.area);
    let line_height = px(text_size * theme.layout.board_line_height.0);
    let run = TextRun {
        len: label.text.len(),
        font: look.font(theme),
        color: look.color,
        background_color: None,
        underline: None,
        strikethrough: None,
    };
    let shaped = window.text_system().shape_text(
        label.text.clone(),
        px(text_size),
        &[run],
        Some(area.size.width),
        look.max_lines,
    );
    let Ok(lines) = shaped else {
        return;
    };
    let height: Pixels = lines.iter().map(|line| line.size(line_height).height).sum();
    let top = if look.centered {
        let middle = frame
            .on_screen(Point::new(label.area.x, label.area.center().y))
            .y;
        middle - height * HALF
    } else {
        area.origin.y
    };
    let text_box = Bounds::new(point(area.origin.x, top), size(area.size.width, height));
    if label.kind == LabelKind::Edge {
        let width = lines
            .iter()
            .map(|line| line.width())
            .fold(px(0.), Pixels::max);
        fill_behind(text_box, width, frame, window);
    }
    let mask = look.clipped.then_some(ContentMask { bounds: area });
    window.with_content_mask(mask, |window| {
        let mut y = top;
        for line in &lines {
            let painted = line.paint(
                point(area.origin.x, y),
                line_height,
                look.align,
                Some(text_box),
                window,
                cx,
            );
            if let Err(error) = painted {
                log::error!("no se pudo pintar un texto de la pizarra: {error:#}");
                return;
            }
            y += line.size(line_height).height;
        }
    });
}
