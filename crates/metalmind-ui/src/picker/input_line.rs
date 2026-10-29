//! Un campo de una línea: el query de los pickers y las preguntas del
//! panel. Texto y cursor, con las teclas de edición de readline. Offsets en
//! bytes.

use gpui_kit::Keystroke;
use unicode_segmentation::UnicodeSegmentation as _;

/// Una edición de la línea, como la piden las acciones `picker::`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LineEdit<'a> {
    Insert(&'a str),
    /// Cambia los bytes `start..end` por `text`, como pide un IME que
    /// corrige lo ya escrito; el cursor queda al final de lo nuevo.
    Replace {
        start: usize,
        end: usize,
        text: &'a str,
    },
    /// `backspace`: el grafema antes del cursor.
    DeleteBackward,
    /// `delete`: el grafema después del cursor.
    DeleteForward,
    /// `ctrl-w`: la palabra antes del cursor y los espacios que la siguen.
    DeleteWordBackward,
    /// `ctrl-u`: todo lo que hay antes del cursor.
    DeleteToStart,
    Left,
    Right,
    Home,
    End,
}

impl LineEdit<'static> {
    /// La tecla de edición que pide `keystroke`, si es una; el texto no,
    /// que llega por el IME o como `key_char`.
    #[must_use]
    pub fn for_key(keystroke: &Keystroke) -> Option<Self> {
        let modifiers = keystroke.modifiers;
        if modifiers.alt || modifiers.platform || modifiers.function {
            return None;
        }
        if modifiers.control {
            return match keystroke.key.as_str() {
                "u" => Some(Self::DeleteToStart),
                "w" | "backspace" => Some(Self::DeleteWordBackward),
                "a" => Some(Self::Home),
                "e" => Some(Self::End),
                _ => None,
            };
        }
        match keystroke.key.as_str() {
            "backspace" => Some(Self::DeleteBackward),
            "delete" => Some(Self::DeleteForward),
            "left" => Some(Self::Left),
            "right" => Some(Self::Right),
            "home" => Some(Self::Home),
            "end" => Some(Self::End),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct InputLine {
    text: String,
    cursor: usize,
}

impl InputLine {
    /// Con el cursor al final.
    #[must_use]
    pub fn new(text: &str) -> Self {
        Self::with_cursor(text, text.len())
    }

    /// Con el cursor en `cursor`, o al final si no cae entre dos grafemas.
    #[must_use]
    pub fn with_cursor(text: &str, cursor: usize) -> Self {
        let on_boundary = cursor == text.len()
            || text
                .grapheme_indices(true)
                .any(|(offset, _)| offset == cursor);
        Self {
            text: text.to_owned(),
            cursor: if on_boundary { cursor } else { text.len() },
        }
    }

    #[must_use]
    pub fn text(&self) -> &str {
        &self.text
    }

    /// Byte del cursor, siempre en frontera de grafema.
    #[must_use]
    pub fn cursor(&self) -> usize {
        self.cursor
    }

    /// Aplica `edit`; devuelve si cambió el texto (no solo el cursor).
    pub fn apply(&mut self, edit: LineEdit<'_>) -> bool {
        let before = self.text.len();
        match edit {
            LineEdit::Insert(text) => {
                let text = printable(text);
                self.text.insert_str(self.cursor, &text);
                self.cursor += text.len();
                return !text.is_empty();
            }
            LineEdit::Replace { start, end, text } => {
                let start = self.floor_boundary(start);
                let end = self.floor_boundary(end).max(start);
                let text = printable(text);
                let changed = self.text[start..end] != text;
                self.text.replace_range(start..end, &text);
                self.cursor = start + text.len();
                return changed;
            }
            LineEdit::DeleteBackward => self.delete_to(self.previous_boundary()),
            LineEdit::DeleteForward => {
                let end = self.next_boundary();
                self.text.replace_range(self.cursor..end, "");
            }
            LineEdit::DeleteWordBackward => self.delete_to(self.word_start()),
            LineEdit::DeleteToStart => self.delete_to(0),
            LineEdit::Left => self.cursor = self.previous_boundary(),
            LineEdit::Right => self.cursor = self.next_boundary(),
            LineEdit::Home => self.cursor = 0,
            LineEdit::End => self.cursor = self.text.len(),
        }
        self.text.len() != before
    }

    fn floor_boundary(&self, offset: usize) -> usize {
        let mut offset = offset.min(self.text.len());
        while !self.text.is_char_boundary(offset) {
            offset -= 1;
        }
        offset
    }

    fn delete_to(&mut self, start: usize) {
        self.text.replace_range(start..self.cursor, "");
        self.cursor = start;
    }

    fn previous_boundary(&self) -> usize {
        self.text[..self.cursor]
            .grapheme_indices(true)
            .next_back()
            .map_or(0, |(offset, _)| offset)
    }

    fn next_boundary(&self) -> usize {
        self.text[self.cursor..]
            .graphemes(true)
            .next()
            .map_or(self.cursor, |grapheme| self.cursor + grapheme.len())
    }

    /// Como el `ctrl-w` de readline: salta los espacios y luego la palabra.
    fn word_start(&self) -> usize {
        let before = &self.text[..self.cursor];
        let trimmed = before.trim_end();
        trimmed
            .char_indices()
            .rev()
            .find(|(_, c)| c.is_whitespace())
            .map_or(0, |(offset, c)| offset + c.len_utf8())
    }
}

fn printable(text: &str) -> String {
    text.chars().filter(|c| !c.is_control()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// El texto con `|` en el cursor.
    fn line(marked: &str) -> InputLine {
        let cursor = marked.find('|').unwrap();
        InputLine {
            text: marked.replace('|', ""),
            cursor,
        }
    }

    fn show(line: &InputLine) -> String {
        let mut text = line.text.clone();
        text.insert(line.cursor, '|');
        text
    }

    #[test]
    fn ediciones() {
        let cases = [
            ("ab|", LineEdit::Insert("c"), "abc|", true),
            ("a|c", LineEdit::Insert("ñ"), "añ|c", true),
            (
                "teh|",
                LineEdit::Replace {
                    start: 0,
                    end: 3,
                    text: "the",
                },
                "the|",
                true,
            ),
            (
                "|añb",
                LineEdit::Replace {
                    start: 2,
                    end: 9,
                    text: "x",
                },
                "ax|",
                true,
            ),
            ("a|", LineEdit::Insert("\n\t"), "a|", false),
            ("añ|", LineEdit::DeleteBackward, "a|", true),
            ("|a", LineEdit::DeleteBackward, "|a", false),
            ("cafe\u{301}|", LineEdit::DeleteBackward, "caf|", true),
            ("|ñb", LineEdit::DeleteForward, "|b", true),
            ("a|", LineEdit::DeleteForward, "a|", false),
            ("uno dos  |", LineEdit::DeleteWordBackward, "uno |", true),
            ("uno do|s", LineEdit::DeleteWordBackward, "uno |s", true),
            ("uno|", LineEdit::DeleteWordBackward, "|", true),
            ("ab|cd", LineEdit::DeleteToStart, "|cd", true),
            ("añ|", LineEdit::Left, "a|ñ", false),
            ("|a", LineEdit::Left, "|a", false),
            ("a|ñb", LineEdit::Right, "añ|b", false),
            ("ab|", LineEdit::Right, "ab|", false),
            ("ab|c", LineEdit::Home, "|abc", false),
            ("a|bc", LineEdit::End, "abc|", false),
        ];
        for (before, edit, after, changed) in cases {
            let mut input = line(before);
            assert_eq!(input.apply(edit), changed, "{before:?} {edit:?}");
            assert_eq!(show(&input), after, "{before:?} {edit:?}");
        }
    }

    #[test]
    fn el_cursor_inicial_cae_entre_grafemas() {
        assert_eq!(show(&InputLine::new("año")), "año|");
        assert_eq!(show(&InputLine::with_cursor("nota.md", 4)), "nota|.md");
        assert_eq!(show(&InputLine::with_cursor("ñ", 1)), "ñ|");
        assert_eq!(
            show(&InputLine::with_cursor("cafe\u{301}s", 4)),
            "cafe\u{301}s|"
        );
        assert_eq!(show(&InputLine::with_cursor("ab", 9)), "ab|");
    }

    #[test]
    fn teclas_de_edicion() {
        use gpui_kit::Modifiers;

        let key = |key: &str, modifiers| Keystroke {
            modifiers,
            key: key.to_owned(),
            key_char: None,
        };
        let none = Modifiers::none();
        let ctrl = Modifiers::control();
        let cases = [
            (key("backspace", none), Some(LineEdit::DeleteBackward)),
            (key("delete", none), Some(LineEdit::DeleteForward)),
            (key("left", none), Some(LineEdit::Left)),
            (key("right", none), Some(LineEdit::Right)),
            (key("home", none), Some(LineEdit::Home)),
            (key("end", none), Some(LineEdit::End)),
            (key("u", ctrl), Some(LineEdit::DeleteToStart)),
            (key("w", ctrl), Some(LineEdit::DeleteWordBackward)),
            (key("backspace", ctrl), Some(LineEdit::DeleteWordBackward)),
            (key("a", ctrl), Some(LineEdit::Home)),
            (key("e", ctrl), Some(LineEdit::End)),
            (key("c", ctrl), None),
            (key("a", none), None),
            (key("backspace", Modifiers::alt()), None),
        ];
        for (keystroke, edit) in cases {
            assert_eq!(LineEdit::for_key(&keystroke), edit, "{keystroke:?}");
        }
    }
}
