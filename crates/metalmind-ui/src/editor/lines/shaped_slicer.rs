use std::ops::Range;

use gpui_kit::{ShapedLine, ShapedLineCursor};

/// Una línea con su forma, cortada en trozos de izquierda a derecha: se le
/// da forma entera una vez, la misma con que el motor la parte en filas, y
/// no a cada trozo de cada fila.
pub(super) struct ShapedSlicer<'s> {
    text: &'s str,
    cursor: ShapedLineCursor<'s>,
    /// Hasta dónde se cortó.
    at: usize,
}

impl<'s> ShapedSlicer<'s> {
    pub(super) fn new(shaped: &'s ShapedLine) -> Self {
        Self {
            text: &shaped.text,
            cursor: shaped.cursor(),
            at: 0,
        }
    }

    /// Los bytes `range` con su forma, desde 0. `None` si `range` empieza
    /// antes de donde terminó el anterior o parte un carácter, lo que sería
    /// un bug de quien arma las filas.
    pub(super) fn take(&mut self, range: Range<usize>) -> Option<ShapedLine> {
        let fits = self.at <= range.start
            && range.start <= range.end
            && range.end <= self.text.len()
            && self.text.is_char_boundary(range.start)
            && self.text.is_char_boundary(range.end);
        if !fits {
            log::error!(
                "trozo {range:?} fuera de orden tras el byte {} de {:?}",
                self.at,
                self.text
            );
            return None;
        }
        if self.at < range.start {
            self.cursor.take_until(range.start);
        }
        self.at = range.end;
        Some(self.cursor.take_until(range.end))
    }
}

#[cfg(test)]
mod tests {
    use gpui_kit::{Font, Hsla, SharedString, TestAppContext, TextRun, font, px};
    use pretty_assertions::assert_eq;

    use super::ShapedSlicer;

    #[test]
    fn los_trozos_son_los_de_la_linea_entera_y_empiezan_en_cero() {
        let mut cx = TestAppContext::single();
        let window = cx.add_empty_window();
        window.update(|window, _| {
            let text = "una línea, $x^2$ y más texto";
            let run = |len, font: Font| TextRun {
                len,
                font,
                color: Hsla::default(),
                background_color: None,
                underline: None,
                strikethrough: None,
            };
            let (formula, rest) = (text.find('$').unwrap(), text.rfind('$').unwrap() + 1);
            let runs = [
                run(formula, font("Mono")),
                run(text.len() - formula, font("Mono")),
            ];
            let whole =
                window
                    .text_system()
                    .shape_line(SharedString::from(text), px(15.), &runs, None);
            let mut slicer = ShapedSlicer::new(&whole);
            let before = slicer.take(0..formula).unwrap();
            let after = slicer.take(rest..text.len()).unwrap();
            assert_eq!(before.text.as_ref(), "una línea, ");
            assert_eq!(after.text.as_ref(), " y más texto");
            assert_eq!(before.width(), whole.x_for_index(formula));
            assert_eq!(after.x_for_index(0), px(0.));
            assert_eq!(
                after.width(),
                whole.width() - whole.x_for_index(rest),
                "el trozo mide lo que ocupa en la línea"
            );
            assert!(slicer.take(3..5).is_none(), "un trozo de atrás es un bug");
            assert_eq!(slicer.take(text.len()..text.len()).unwrap().len(), 0);
        });
    }

    #[test]
    fn un_trozo_que_parte_un_caracter_no_se_corta() {
        let mut cx = TestAppContext::single();
        let window = cx.add_empty_window();
        window.update(|window, _| {
            let text = "línea";
            let runs = [TextRun {
                len: text.len(),
                font: font("Mono"),
                color: Hsla::default(),
                background_color: None,
                underline: None,
                strikethrough: None,
            }];
            let whole =
                window
                    .text_system()
                    .shape_line(SharedString::from(text), px(15.), &runs, None);
            let mut slicer = ShapedSlicer::new(&whole);
            assert!(slicer.take(0..2).is_none(), "la í ocupa los bytes 1 y 2");
            assert_eq!(slicer.take(0..3).unwrap().text.as_ref(), "lí");
        });
    }
}
