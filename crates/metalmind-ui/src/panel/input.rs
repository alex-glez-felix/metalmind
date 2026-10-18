//! Un campo de una línea: el nombre al crear o renombrar y el filtro.

/// Texto con un cursor, en bytes y siempre en el borde de un carácter.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct LineInput {
    text: String,
    cursor: usize,
}

impl LineInput {
    /// Con el cursor al final.
    #[must_use]
    pub fn new(text: &str) -> Self {
        Self {
            text: text.to_owned(),
            cursor: text.len(),
        }
    }

    /// Con el cursor en `cursor`, o al final si no cae en un carácter.
    #[must_use]
    pub fn with_cursor(text: &str, cursor: usize) -> Self {
        let mut input = Self::new(text);
        if text.is_char_boundary(cursor) {
            input.cursor = cursor;
        }
        input
    }

    #[must_use]
    pub fn text(&self) -> &str {
        &self.text
    }

    #[must_use]
    pub fn cursor(&self) -> usize {
        self.cursor
    }

    pub fn insert(&mut self, text: &str) {
        let text: String = text.chars().filter(|c| !c.is_control()).collect();
        self.text.insert_str(self.cursor, &text);
        self.cursor += text.len();
    }

    pub fn backspace(&mut self) {
        if let Some(previous) = self.previous() {
            self.text.replace_range(previous..self.cursor, "");
            self.cursor = previous;
        }
    }

    pub fn delete(&mut self) {
        if let Some(next) = self.next() {
            self.text.replace_range(self.cursor..next, "");
        }
    }

    pub fn left(&mut self) {
        if let Some(previous) = self.previous() {
            self.cursor = previous;
        }
    }

    pub fn right(&mut self) {
        if let Some(next) = self.next() {
            self.cursor = next;
        }
    }

    pub fn home(&mut self) {
        self.cursor = 0;
    }

    pub fn end(&mut self) {
        self.cursor = self.text.len();
    }

    /// `ctrl-u`: borra hasta el principio, como en la línea de comandos de vim.
    pub fn clear_before(&mut self) {
        self.text.replace_range(..self.cursor, "");
        self.cursor = 0;
    }

    fn previous(&self) -> Option<usize> {
        self.text[..self.cursor]
            .char_indices()
            .next_back()
            .map(|(at, _)| at)
    }

    fn next(&self) -> Option<usize> {
        self.text[self.cursor..]
            .chars()
            .next()
            .map(|c| self.cursor + c.len_utf8())
    }
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn edita_por_caracteres() {
        let mut input = LineInput::new("año");
        input.left();
        input.left();
        assert_eq!(input.cursor(), 1);
        input.insert("x");
        assert_eq!(input.text(), "axño");
        input.delete();
        assert_eq!(input.text(), "axo");
        input.backspace();
        input.backspace();
        input.backspace();
        assert_eq!((input.text(), input.cursor()), ("o", 0));
        input.end();
        input.right();
        input.insert("\n\tz");
        assert_eq!(input.text(), "oz", "sin controles");
        input.home();
        input.delete();
        assert_eq!(input.text(), "z");
    }

    #[test]
    fn ctrl_u_borra_hasta_el_cursor() {
        let mut input = LineInput::with_cursor("nota.md", 4);
        input.clear_before();
        assert_eq!((input.text(), input.cursor()), (".md", 0));
        assert_eq!(LineInput::with_cursor("ñ", 1).cursor(), 2);
    }
}
