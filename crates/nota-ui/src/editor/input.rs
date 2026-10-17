//! Input handler de GPUI: por aquí llegan el texto compuesto, las commits del
//! IME y el preedit. Al IME se le muestra solo la línea del cursor (o la línea
//! de comandos), con el preedit insertado, en unidades UTF-16.

use std::ops::Range;

use gpui_kit::{
    Bounds, Context, EntityInputHandler, Pixels, Point, UTF16Selection, Window, point, size,
};

use super::Editor;
use super::element::cursor_position;
use super::utf16;

/// La línea donde se escribe, tal como la ve el IME.
#[derive(Debug, PartialEq, Eq)]
pub struct ImeLine {
    pub text: String,
    /// Byte del cursor, donde empieza el preedit.
    pub cursor: usize,
    pub marked_len: usize,
}

impl ImeLine {
    pub fn new(mut line: String, cursor: usize, marked: &str) -> Self {
        line.insert_str(cursor, marked);
        Self {
            text: line,
            cursor,
            marked_len: marked.len(),
        }
    }

    fn utf16_at(&self, byte: usize) -> usize {
        utf16::from_byte(&self.text, byte)
    }

    pub fn marked_range(&self) -> Range<usize> {
        self.utf16_at(self.cursor)..self.utf16_at(self.cursor + self.marked_len)
    }

    pub fn caret(&self) -> usize {
        self.utf16_at(self.cursor + self.marked_len)
    }

    /// El texto en `range` y el rango que de verdad cubre, ajustado a
    /// fronteras de carácter.
    pub fn slice(&self, range: Range<usize>) -> (String, Range<usize>) {
        let start = utf16::to_byte(&self.text, range.start);
        let end = utf16::to_byte(&self.text, range.end).max(start);
        let text = self.text[start..end].to_owned();
        (text, self.utf16_at(start)..self.utf16_at(end))
    }
}

impl EntityInputHandler for Editor {
    fn text_for_range(
        &mut self,
        range: Range<usize>,
        adjusted_range: &mut Option<Range<usize>>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<String> {
        let (text, adjusted) = self.ime_line()?.slice(range);
        *adjusted_range = Some(adjusted);
        Some(text)
    }

    fn selected_text_range(
        &mut self,
        _ignore_disabled_input: bool,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<UTF16Selection> {
        let caret = self.ime_line()?.caret();
        Some(UTF16Selection {
            range: caret..caret,
            reversed: false,
        })
    }

    fn marked_text_range(
        &self,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<Range<usize>> {
        self.marked.as_ref()?;
        Some(self.ime_line()?.marked_range())
    }

    fn unmark_text(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        self.set_preedit(None, cx);
    }

    fn replace_text_in_range(
        &mut self,
        range: Option<Range<usize>>,
        text: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.commit_text(range, text, window, cx);
    }

    fn replace_and_mark_text_in_range(
        &mut self,
        _range: Option<Range<usize>>,
        new_text: &str,
        _new_selected_range: Option<Range<usize>>,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let preedit = (!new_text.is_empty()).then(|| new_text.to_owned());
        self.set_preedit(preedit, cx);
    }

    /// El IME coloca su ventana de candidatos junto al cursor.
    fn bounds_for_range(
        &mut self,
        _range_utf16: Range<usize>,
        _element_bounds: Bounds<Pixels>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<Bounds<Pixels>> {
        if self.document.command_line().is_some() {
            return None;
        }
        let layout = self.layout.as_ref()?;
        let (laid, column) = cursor_position(&self.document, layout)?;
        let line_height = layout.viewport.line_height;
        Some(Bounds::new(
            point(
                laid.origin.x + laid.shaped.x_for_index(column),
                laid.origin.y,
            ),
            size(line_height, line_height),
        ))
    }

    fn character_index_for_point(
        &mut self,
        position: Point<Pixels>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<usize> {
        if self.document.command_line().is_some() {
            return None;
        }
        let (line, index) = self.layout.as_ref()?.hit(position)?;
        let buffer = self.document.buffer();
        (line == buffer.line_of(self.document.cursor()))
            .then(|| self.ime_line().map(|ime| ime.utf16_at(index)))
            .flatten()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn el_preedit_se_inserta_en_el_cursor() {
        let ime = ImeLine::new("añob".to_owned(), 3, "´");
        assert_eq!(ime.text, "añ´ob");
        assert_eq!(ime.marked_range(), 2..3);
        assert_eq!(ime.caret(), 3);
    }

    #[test]
    fn sin_preedit_el_caret_es_el_cursor() {
        let ime = ImeLine::new("hola".to_owned(), 2, "");
        assert_eq!(ime.marked_range(), 2..2);
        assert_eq!(ime.caret(), 2);
    }

    #[test]
    fn slice_ajusta_a_fronteras_de_caracter() {
        let ime = ImeLine::new("a😀b".to_owned(), 0, "");
        assert_eq!(ime.slice(0..2), ("a".to_owned(), 0..1));
        assert_eq!(ime.slice(1..3), ("😀".to_owned(), 1..3));
        assert_eq!(ime.slice(3..99), ("b".to_owned(), 3..4));
    }
}
