//! Una nota abierta en el motor vim, para medir teclas como las recibe el editor.

use metalmind_buffer::{Buffer, Selection, Selections};
use metalmind_vim::{Key, KeyOutcome, Vim, VimError};

/// En [`Session::feed`], `\x1b` es Esc, `\n` es Enter y `\x08` es Backspace.
pub struct Session {
    pub vim: Vim,
    pub buffer: Buffer,
    pub selections: Selections,
}

impl Session {
    /// `text` en Normal, con el cursor al inicio de la línea `line`.
    #[must_use]
    pub fn new(text: &str, line: usize) -> Self {
        let buffer = Buffer::from(text);
        let cursor = buffer.line_start(line);
        Self {
            vim: Vim::new(),
            buffer,
            selections: Selections::single(Selection::cursor(cursor)),
        }
    }

    /// Lo que pasó con la última tecla.
    ///
    /// # Errors
    /// Si el motor falla con alguna tecla.
    pub fn feed(&mut self, keys: &str) -> Result<Option<KeyOutcome>, VimError> {
        let mut last = None;
        for c in keys.chars() {
            let key = match c {
                '\x1b' => Key::Escape,
                '\n' => Key::Enter,
                '\x08' => Key::Backspace,
                c => Key::Char(c),
            };
            last = Some(self.vim.feed(key, &mut self.buffer, &mut self.selections)?);
        }
        Ok(last)
    }
}

#[cfg(test)]
mod tests {
    use metalmind_vim::Mode;

    use super::*;

    #[test]
    fn feeds_escape_and_enter() {
        let mut session = Session::new("uno\ndos\n", 1);
        session.feed("ciwtres\x1b").unwrap();
        assert_eq!(session.buffer.to_string(), "uno\ntres\n");
        assert_eq!(session.vim.mode(), Mode::Normal);
        session.feed(":%s/uno/cero/\n").unwrap();
        assert_eq!(session.buffer.to_string(), "cero\ntres\n");
    }
}
