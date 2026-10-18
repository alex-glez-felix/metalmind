//! La línea donde se escribe el query: texto y cursor, con las teclas de
//! edición de una línea de readline. Offsets en bytes.

use unicode_segmentation::UnicodeSegmentation as _;

/// Una edición de la línea, como la piden las acciones `picker::`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LineEdit<'a> {
    Insert(&'a str),
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

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct InputLine {
    text: String,
    cursor: usize,
}

impl InputLine {
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
                let text: String = text.chars().filter(|c| !c.is_control()).collect();
                self.text.insert_str(self.cursor, &text);
                self.cursor += text.len();
                return !text.is_empty();
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
}
