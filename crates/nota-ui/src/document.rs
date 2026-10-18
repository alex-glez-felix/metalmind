//! Lo que el editor sabe de una nota, sin nada de GPUI: el texto con sus
//! selecciones, el estado de vim que lo edita y el archivo donde se guarda.

use std::fmt;
use std::path::{Path, PathBuf};

use nota_buffer::{
    Bias, Buffer, BufferError, ByteOffset, Edit, Goal, HistorySeq, Selection, Selections,
};
use nota_vault::FileError;
use nota_vim::{Clipboard, Key, KeyOutcome, Mode, Vim, VimError};

#[derive(Debug, thiserror::Error)]
pub enum DocumentError {
    #[error(transparent)]
    Vim(#[from] VimError),
    #[error(transparent)]
    Buffer(#[from] BufferError),
}

/// Tamaño de lo leído o escrito, para avisarlo como vim (`"a.md" 3L, 20B`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FileStats {
    pub lines: usize,
    pub bytes: usize,
}

impl FileStats {
    fn of(buffer: &Buffer) -> Self {
        Self {
            lines: buffer.line_count(),
            bytes: buffer.len().get(),
        }
    }
}

impl fmt::Display for FileStats {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}L, {}B", self.lines, self.bytes)
    }
}

#[derive(Debug)]
pub struct Document {
    path: PathBuf,
    buffer: Buffer,
    selections: Selections,
    vim: Vim,
    /// Estado del historial que coincide con el disco; `None` si se guardó
    /// a mitad de una transacción y ningún estado coincide.
    saved: Option<HistorySeq>,
    exists_on_disk: bool,
}

impl Document {
    /// Abre `path`; si no existe, empieza vacío y se crea al guardar.
    ///
    /// # Errors
    /// Si existe pero no se puede leer como texto.
    pub fn open(path: &Path) -> Result<Self, FileError> {
        let text = nota_vault::read(path)?;
        let exists_on_disk = text.is_some();
        let mut document = Self::new(path, &text.unwrap_or_default());
        document.exists_on_disk = exists_on_disk;
        Ok(document)
    }

    #[must_use]
    pub fn new(path: &Path, text: &str) -> Self {
        let buffer = Buffer::from(text);
        Self {
            path: path.to_owned(),
            saved: Some(buffer.history_seq()),
            buffer,
            selections: Selections::single(Selection::cursor(ByteOffset::ZERO)),
            vim: Vim::new(),
            exists_on_disk: false,
        }
    }

    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    #[must_use]
    pub fn exists_on_disk(&self) -> bool {
        self.exists_on_disk
    }

    #[must_use]
    pub fn stats(&self) -> FileStats {
        FileStats::of(&self.buffer)
    }

    /// Como vim, depende del estado del historial y no del texto: volver a
    /// lo guardado con `u` o `g-` lo limpia, pero llegar al mismo texto por
    /// otra edición no.
    #[must_use]
    pub fn is_modified(&self) -> bool {
        self.buffer.has_uncommitted_edits() || self.saved != Some(self.buffer.history_seq())
    }

    /// # Errors
    /// Si no se pudo escribir; el archivo en disco queda como estaba.
    pub fn save(&mut self) -> Result<FileStats, FileError> {
        nota_vault::write_atomic(&self.path, |out| write!(out, "{}", self.buffer))?;
        self.saved = (!self.buffer.has_uncommitted_edits()).then(|| self.buffer.history_seq());
        self.exists_on_disk = true;
        Ok(self.stats())
    }

    #[must_use]
    pub fn buffer(&self) -> &Buffer {
        &self.buffer
    }

    #[must_use]
    pub fn mode(&self) -> Mode {
        self.vim.mode()
    }

    #[must_use]
    pub fn command_line(&self) -> Option<&str> {
        self.vim.command_line()
    }

    #[must_use]
    pub fn selection(&self) -> Selection {
        *self.selections.primary()
    }

    #[must_use]
    pub fn cursor(&self) -> ByteOffset {
        self.selection().head
    }

    /// # Errors
    /// Solo si el buffer está inconsistente, lo que sería un bug.
    pub fn line_text(&self, line: usize) -> Result<String, DocumentError> {
        let range = self.buffer.line_start(line)..self.buffer.line_end(line);
        Ok(self.buffer.text_in(range)?)
    }

    /// `clipboard` es lo que leen y escriben los registros `"+` y `"*`.
    ///
    /// # Errors
    /// Solo si una edición resulta inválida, lo que sería un bug.
    pub fn feed(
        &mut self,
        key: Key,
        clipboard: &mut dyn Clipboard,
    ) -> Result<KeyOutcome, DocumentError> {
        let outcome =
            self.vim
                .feed_with_clipboard(key, &mut self.buffer, &mut self.selections, clipboard)?;
        Ok(outcome)
    }

    /// Texto ya compuesto por el teclado o el IME: cada carácter entra al
    /// motor como una tecla. Devuelve lo que pasó con el último.
    ///
    /// # Errors
    /// Solo si una edición resulta inválida, lo que sería un bug.
    pub fn type_text(
        &mut self,
        text: &str,
        clipboard: &mut dyn Clipboard,
    ) -> Result<Option<KeyOutcome>, DocumentError> {
        let mut last = None;
        for c in text.chars() {
            last = Some(self.feed(Key::Char(c), clipboard)?);
        }
        Ok(last)
    }

    /// Selección hecha con el ratón, con `head` exclusivo. Un clic en Normal
    /// no deja el cursor sobre el salto de línea, y en Insert descarta la
    /// repetición del count, como en vim.
    pub fn select(&mut self, anchor: ByteOffset, head: ByteOffset) {
        self.vim.cursor_moved();
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

    /// # Errors
    /// Solo si la selección no cae en el buffer, lo que sería un bug.
    pub fn selected_text(&self) -> Result<Option<String>, DocumentError> {
        let selection = self.selection();
        if selection.is_cursor() {
            return Ok(None);
        }
        Ok(Some(self.buffer.text_in(selection.range())?))
    }

    /// La línea del cursor con su salto, como la copia `yy`.
    ///
    /// # Errors
    /// Solo si el buffer está inconsistente, lo que sería un bug.
    pub fn current_line(&self) -> Result<String, DocumentError> {
        let mut line = self.line_text(self.buffer.line_of(self.cursor()))?;
        line.push('\n');
        Ok(line)
    }

    /// Pega en el cursor, o en la línea de comandos si está abierta. En
    /// Insert entra como lo tecleado: `u` lo deshace junto con lo escrito y
    /// el count lo repite. En Normal es un paso propio y el cursor queda
    /// sobre el último carácter pegado.
    ///
    /// # Errors
    /// Solo si la edición resulta inválida, lo que sería un bug.
    pub fn paste(&mut self, text: &str) -> Result<(), DocumentError> {
        if text.is_empty() {
            return Ok(());
        }
        if self.mode().takes_text() {
            self.vim
                .insert_text(text, &mut self.buffer, &mut self.selections)?;
            return Ok(());
        }
        self.vim.cursor_moved();
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
#[path = "document_tests.rs"]
mod tests;
