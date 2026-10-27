//! La línea que ve el IME: el texto donde se escribe con el preedit
//! insertado, en unidades UTF-16. La usan el editor, el input del picker y
//! los campos de la lista de notas.

use std::ops::Range;

use crate::picker::LineEdit;
use crate::utf16;

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

    pub fn utf16_at(&self, byte: usize) -> usize {
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

/// La edición de un commit del IME. Con preedit, el rango es el del preedit,
/// que no está en la línea: va en el cursor. Sin él, el rango (en UTF-16)
/// es de lo ya escrito y se reemplaza, como cuando el IME corrige una
/// palabra.
pub fn commit<'a>(
    line: &str,
    had_preedit: bool,
    range: Option<Range<usize>>,
    text: &'a str,
) -> LineEdit<'a> {
    match range {
        Some(range) if !had_preedit => LineEdit::Replace {
            start: utf16::to_byte(line, range.start),
            end: utf16::to_byte(line, range.end),
            text,
        },
        _ => LineEdit::Insert(text),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn un_commit_reemplaza_lo_escrito_salvo_si_venia_de_un_preedit() {
        assert_eq!(
            commit("añob", false, Some(2..4), "x"),
            LineEdit::Replace {
                start: 3,
                end: 5,
                text: "x"
            }
        );
        assert_eq!(commit("añob", true, Some(2..3), "é"), LineEdit::Insert("é"));
        assert_eq!(commit("añob", false, None, "x"), LineEdit::Insert("x"));
    }

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
