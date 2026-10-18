use metalmind_buffer::{Bias, Buffer, BufferError, ByteOffset, Edit, Goal, Selection, Selections};
use metalmind_vim::{Key, KeyOutcome, Mode, Vim, VimError};

#[derive(Debug, thiserror::Error)]
pub enum DocumentError {
    #[error(transparent)]
    Vim(#[from] VimError),
    #[error(transparent)]
    Buffer(#[from] BufferError),
}

/// Lo que la vista sabe del texto: `metalmind-buffer` con sus selecciones, y el
/// estado de `metalmind-vim` que las edita.
#[derive(Debug)]
pub struct Document {
    buffer: Buffer,
    selections: Selections,
    vim: Vim,
}

impl Document {
    pub fn new(text: &str) -> Self {
        Self {
            buffer: Buffer::from(text),
            selections: Selections::single(Selection::cursor(ByteOffset::ZERO)),
            vim: Vim::new(),
        }
    }

    pub fn buffer(&self) -> &Buffer {
        &self.buffer
    }

    pub fn mode(&self) -> Mode {
        self.vim.mode()
    }

    pub fn selection(&self) -> Selection {
        *self.selections.primary()
    }

    pub fn cursor(&self) -> ByteOffset {
        self.selection().head
    }

    pub fn line_text(&self, line: usize) -> Result<String, DocumentError> {
        let range = self.buffer.line_start(line)..self.buffer.line_end(line);
        Ok(self.buffer.text_in(range)?)
    }

    pub fn feed(&mut self, key: Key) -> Result<KeyOutcome, DocumentError> {
        Ok(self.vim.feed(key, &mut self.buffer, &mut self.selections)?)
    }

    /// Texto ya compuesto por el teclado o el IME: cada carácter entra al
    /// motor como una tecla. Devuelve lo que pasó con el último.
    pub fn type_text(&mut self, text: &str) -> Result<Option<KeyOutcome>, DocumentError> {
        let mut last = None;
        for c in text.chars() {
            last = Some(self.feed(Key::Char(c))?);
        }
        Ok(last)
    }

    /// Selección hecha con el ratón, con `head` exclusivo. Un clic en Normal
    /// no deja el cursor sobre el salto de línea, como en vim.
    pub fn select(&mut self, anchor: ByteOffset, head: ByteOffset) {
        let selection = if anchor == head {
            Selection::cursor(self.clamp_to_mode(head))
        } else {
            Selection {
                anchor,
                head,
                goal: Goal::None,
            }
        };
        self.selections = Selections::single(selection);
    }

    pub fn selected_text(&self) -> Result<Option<String>, DocumentError> {
        let selection = self.selection();
        if selection.is_cursor() {
            return Ok(None);
        }
        Ok(Some(self.buffer.text_in(selection.range())?))
    }

    /// La línea del cursor con su salto, como la copia `yy`.
    pub fn current_line(&self) -> Result<String, DocumentError> {
        let mut line = self.line_text(self.buffer.line_of(self.cursor()))?;
        line.push('\n');
        Ok(line)
    }

    /// Inserta en el cursor. En Insert se suma a la transacción abierta, así
    /// `u` lo deshace junto con lo escrito; fuera de Insert es un paso propio
    /// y el cursor queda sobre el último carácter pegado.
    pub fn paste(&mut self, text: &str) -> Result<(), DocumentError> {
        if text.is_empty() {
            return Ok(());
        }
        if self.buffer.has_open_transaction() {
            let end = self.insert_at_cursor(text)?;
            self.selections = Selections::single(Selection::cursor(end));
            return Ok(());
        }
        self.buffer.begin_transaction(self.selections.clone())?;
        let end = self.insert_at_cursor(text)?;
        self.selections = Selections::single(Selection::cursor(self.buffer.prev_boundary(end)));
        self.buffer.commit_transaction(self.selections.clone())?;
        Ok(())
    }

    fn insert_at_cursor(&mut self, text: &str) -> Result<ByteOffset, DocumentError> {
        let at = self.cursor();
        let map = self.buffer.apply(vec![Edit::insert(at, text)])?;
        Ok(map.map(at, Bias::After))
    }

    fn clamp_to_mode(&self, offset: ByteOffset) -> ByteOffset {
        let offset = offset.min(self.buffer.len());
        let line = self.buffer.line_of(offset);
        let line_end = self.buffer.line_end(line);
        let on_newline = offset == line_end && line_end > self.buffer.line_start(line);
        if on_newline && self.mode() != Mode::Insert {
            self.buffer.prev_boundary(line_end)
        } else {
            offset
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn keys(document: &mut Document, text: &str) {
        for c in text.chars() {
            let key = match c {
                '⎋' => Key::Escape,
                _ => Key::Char(c),
            };
            document.feed(key).unwrap();
        }
    }

    #[test]
    fn el_texto_compuesto_entra_por_el_motor() {
        let mut document = Document::new("hola\n");
        keys(&mut document, "A");
        assert_eq!(
            document.type_text("ñá").unwrap(),
            Some(KeyOutcome::Inserted)
        );
        keys(&mut document, "⎋");
        assert_eq!(document.buffer().to_string(), "holañá\n");
        assert_eq!(document.mode(), Mode::Normal);
    }

    #[test]
    fn en_normal_el_texto_son_comandos() {
        let mut document = Document::new("uno dos\n");
        assert!(matches!(
            document.type_text("w").unwrap(),
            Some(KeyOutcome::Executed(_))
        ));
        assert_eq!(document.cursor(), ByteOffset::new(4));
    }

    #[test]
    fn pegar_en_normal_es_un_paso_de_undo() {
        let mut document = Document::new("ab\n");
        document.paste("XY").unwrap();
        assert_eq!(document.buffer().to_string(), "XYab\n");
        assert_eq!(document.cursor(), ByteOffset::new(1));
        keys(&mut document, "u");
        assert_eq!(document.buffer().to_string(), "ab\n");
    }

    #[test]
    fn pegar_en_insert_se_deshace_con_lo_escrito() {
        let mut document = Document::new("ab\n");
        keys(&mut document, "iz");
        document.paste("ñ").unwrap();
        keys(&mut document, "q⎋");
        assert_eq!(document.buffer().to_string(), "zñqab\n");
        keys(&mut document, "u");
        assert_eq!(document.buffer().to_string(), "ab\n");
    }

    #[test]
    fn seleccion_con_raton_y_copia() {
        let mut document = Document::new("hola mundo\nadiós\n");
        assert_eq!(document.selected_text().unwrap(), None);
        document.select(ByteOffset::new(5), ByteOffset::new(10));
        assert_eq!(document.selected_text().unwrap().as_deref(), Some("mundo"));
        document.select(ByteOffset::new(11), ByteOffset::new(11));
        assert_eq!(document.current_line().unwrap(), "adiós\n");
    }

    #[test]
    fn en_normal_el_cursor_no_queda_sobre_el_salto() {
        let mut document = Document::new("abc\n\nxyz\n");
        document.select(ByteOffset::new(3), ByteOffset::new(3));
        assert_eq!(document.cursor(), ByteOffset::new(2));
        document.select(ByteOffset::new(4), ByteOffset::new(4));
        assert_eq!(document.cursor(), ByteOffset::new(4));
        keys(&mut document, "A");
        document.select(ByteOffset::new(3), ByteOffset::new(3));
        assert_eq!(document.cursor(), ByteOffset::new(3));
    }
}
