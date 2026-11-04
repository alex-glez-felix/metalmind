//! El texto de una caja mientras se escribe: lo que hay en el buffer del motor,
//! pintado donde iría su etiqueta y partido a su ancho, con el cursor de cada
//! modo. Lo edita el motor; aquí solo se pinta y se traduce un clic.

use std::ops::Range;

use gpui_kit::{Bounds, Context, Pixels, Point as ScreenPoint, Window, fill, point, size};
use metalmind_buffer::{ByteOffset, Selection};
use metalmind_vim::Mode;

use super::BoardView;
use super::label::{Laid, TextRows, lay_out};
use super::paint::Frame;
use super::scene::OpenLabel;
use crate::theme::Theme;

/// Lo que hace falta del motor para pintar el texto abierto.
pub(super) struct OpenTextSnapshot {
    pub label: OpenLabel,
    pub text: String,
    pub selection: Selection,
    pub mode: Mode,
    /// El texto provisional del IME, en el cursor.
    pub preedit: Option<String>,
}

impl OpenTextSnapshot {
    /// El texto como se ve, con el preedit metido en el cursor, y el rango del
    /// preedit.
    fn displayed(&self) -> (String, Option<Range<usize>>) {
        let Some(preedit) = self.preedit.as_deref().filter(|p| !p.is_empty()) else {
            return (self.text.clone(), None);
        };
        let at = self.selection.head.get().min(self.text.len());
        let mut shown = self.text.clone();
        shown.insert_str(at, preedit);
        (shown, Some(at..at + preedit.len()))
    }

    /// Pinta el texto y el cursor. Da sus filas y dónde quedó el cursor, para
    /// el ratón y el IME.
    pub(super) fn paint(
        &self,
        frame: &Frame<'_>,
        window: &mut Window,
        cx: &mut gpui_kit::App,
    ) -> Option<(TextRows, Bounds<Pixels>)> {
        let (shown, preedit) = self.displayed();
        let laid = lay_out(
            shown.clone().into(),
            preedit,
            self.label.kind,
            self.label.area,
            frame,
            window,
        )?;
        let rows = laid.rows();
        self.paint_selection(&rows, &laid, frame, window);
        laid.paint(None, window, cx);
        let caret = self.caret(&rows, &laid, &shown, frame)?;
        window.paint_quad(fill(caret, self.caret_color(frame.theme)));
        Some((rows, caret))
    }

    fn caret_color(&self, theme: &Theme) -> gpui_kit::Hsla {
        match self.mode {
            Mode::Insert => theme.cursor.insert,
            Mode::Visual(_) => theme.cursor.visual,
            _ => theme.cursor.normal,
        }
    }

    fn caret(
        &self,
        rows: &TextRows,
        laid: &Laid,
        shown: &str,
        frame: &Frame<'_>,
    ) -> Option<Bounds<Pixels>> {
        let head = self.selection.head.get().min(shown.len());
        let (x, y) = rows.place(head)?;
        let width = if self.mode == Mode::Insert {
            frame.theme.stroke.caret
        } else {
            let next = shown[head..]
                .chars()
                .next()
                .map_or(head, |c| head + c.len_utf8());
            let after = rows.place(next).filter(|(_, next_y)| *next_y == y);
            after.map_or(laid.blank_width(), |(next_x, _)| {
                if next_x > x {
                    next_x - x
                } else {
                    laid.blank_width()
                }
            })
        };
        Some(Bounds::new(point(x, y), size(width, laid.line_height())))
    }

    /// El fondo de lo elegido en Visual, fila por fila.
    fn paint_selection(
        &self,
        rows: &TextRows,
        laid: &Laid,
        frame: &Frame<'_>,
        window: &mut Window,
    ) {
        if !matches!(self.mode, Mode::Visual(_)) {
            return;
        }
        let (anchor, head) = (self.selection.anchor.get(), self.selection.head.get());
        let chosen = anchor.min(head)..anchor.max(head) + 1;
        for (left, right, top) in rows.spans(chosen) {
            let bounds = Bounds::new(point(left, top), size(right - left, laid.line_height()));
            window.paint_quad(fill(bounds, frame.theme.ui.selection));
        }
    }
}

impl BoardView {
    /// Si apretar en `at` cae en el texto abierto: lleva ahí el cursor.
    pub(super) fn press_in_open_text(
        &mut self,
        at: ScreenPoint<Pixels>,
        cx: &mut Context<Self>,
    ) -> bool {
        let Some(hit) = self.text_area.as_ref() else {
            return false;
        };
        if !hit.bounds.contains(&at) {
            return false;
        }
        let offset = ByteOffset::new(hit.rows.offset_at(at).unwrap_or_default());
        let pane = self.pane;
        let pressed = self.workspace.update(cx, |workspace, cx| {
            cx.notify();
            workspace.press(pane, offset)
        });
        if let Err(error) = pressed {
            log::error!("{error}");
        }
        true
    }
}

/// El área del texto abierto en el último frame, para los clics.
pub(super) struct TextArea {
    pub bounds: Bounds<Pixels>,
    pub rows: TextRows,
}

#[cfg(test)]
mod tests {
    use metalmind_board::Rect;
    use pretty_assertions::assert_eq;

    use super::*;
    use crate::board::scene::LabelKind;

    fn snapshot(text: &str, head: usize, preedit: Option<&str>) -> OpenTextSnapshot {
        OpenTextSnapshot {
            label: OpenLabel {
                kind: LabelKind::Body,
                area: Rect {
                    x: 0.0,
                    y: 0.0,
                    w: 100.0,
                    h: 50.0,
                },
            },
            text: text.to_owned(),
            selection: Selection::cursor(ByteOffset::new(head)),
            mode: Mode::Insert,
            preedit: preedit.map(ToOwned::to_owned),
        }
    }

    #[test]
    fn sin_preedit_se_ve_el_texto_tal_cual() {
        let shown = snapshot("niño", 3, None).displayed();
        assert_eq!(shown, ("niño".to_owned(), None));
        let empty = snapshot("niño", 3, Some("")).displayed();
        assert_eq!(empty, ("niño".to_owned(), None));
    }

    #[test]
    fn el_preedit_entra_en_el_cursor_y_se_sabe_donde_quedo() {
        let shown = snapshot("niño", 4, Some("にほ")).displayed();
        assert_eq!(shown, ("niñにほo".to_owned(), Some(4..10)));
    }

    #[test]
    fn un_cursor_mas_alla_del_texto_no_rompe_el_preedit() {
        let shown = snapshot("a", 9, Some("x")).displayed();
        assert_eq!(shown, ("ax".to_owned(), Some(1..2)));
    }
}
