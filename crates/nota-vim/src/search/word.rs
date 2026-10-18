//! Lo que buscan `*`, `#`, `g*` y `g#`, con la lógica de
//! `find_ident_under_cursor()` de vim.

use nota_buffer::{Buffer, ByteOffset};

use crate::cursor::Class;

/// La palabra bajo el cursor o la siguiente en la línea; si no queda
/// ninguna, la puntuación.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Word {
    pub start: ByteOffset,
    text: String,
    keyword: bool,
}

impl Word {
    pub fn under_cursor(buffer: &Buffer, head: ByteOffset) -> Option<Self> {
        let line = buffer.line_of(head);
        let line_start = buffer.line_start(line);
        let text = buffer.line_text(line);
        let chars: Vec<(usize, char)> = text.char_indices().collect();
        let column = head.get() - line_start.get();
        let cursor = chars.iter().position(|&(at, _)| at >= column)?;
        let class = |index: usize| Class::of(Some(chars[index].1), false);
        let found = (cursor..chars.len())
            .find(|&index| class(index) == Class::Word)
            .or_else(|| (cursor..chars.len()).find(|&index| class(index) != Class::Blank))?;
        let kind = class(found);
        let first = (0..found)
            .rev()
            .take_while(|&index| class(index) == kind)
            .last()
            .unwrap_or(found);
        let end = (found..chars.len())
            .find(|&index| class(index) != kind)
            .map_or(text.len(), |index| chars[index].0);
        Some(Self {
            start: line_start + chars[first].0,
            text: text[chars[first].0..end].to_owned(),
            keyword: kind == Class::Word,
        })
    }

    /// El patrón que arma vim: el texto escapado y, para `*` y `#` sobre una
    /// palabra, entre `\<` y `\>`.
    pub fn pattern(&self, whole_word: bool) -> String {
        let mut pattern = String::new();
        let bounded = whole_word && self.keyword;
        if bounded {
            pattern.push_str("\\<");
        }
        for c in self.text.chars() {
            if matches!(c, '\\' | '/' | '.' | '*' | '~' | '[' | '^' | '$') {
                pattern.push('\\');
            }
            pattern.push(c);
        }
        if bounded {
            pattern.push_str("\\>");
        }
        pattern
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn word(text: &str, column: usize) -> Option<(usize, String)> {
        let buffer = Buffer::from(text);
        Word::under_cursor(&buffer, ByteOffset::new(column))
            .map(|word| (word.start.get(), word.pattern(true)))
    }

    #[test]
    fn prefers_a_word_then_punctuation() {
        assert_eq!(word("foo bar", 1), Some((0, "\\<foo\\>".to_owned())));
        assert_eq!(word("foo bar", 3), Some((4, "\\<bar\\>".to_owned())));
        assert_eq!(word("a ... b", 2), Some((6, "\\<b\\>".to_owned())));
        assert_eq!(word("a b ...", 4), Some((4, "\\.\\.\\.".to_owned())));
        assert_eq!(word("a b x.*", 6), Some((5, "\\.\\*".to_owned())));
        assert_eq!(word("x añob", 3), Some((2, "\\<añob\\>".to_owned())));
        assert_eq!(word("x   ", 1), None);
        assert_eq!(word("", 0), None);
    }
}
