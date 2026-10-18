//! Una nota abierta, sin nada de GPUI: su texto con el historial y el
//! archivo donde se guarda. Los cursores son de cada panel y el estado de
//! vim es uno solo para todos (ver [`Workspace`](crate::workspace::Workspace)).

use std::fmt;
use std::path::{Path, PathBuf};

use nota_buffer::{Buffer, BufferError, HistorySeq};
use nota_vault::FileError;
use nota_vim::Marks;

/// Como el `fixeol` de vim: en disco cada línea termina en `\n`, y en el buffer
/// la última no lo lleva. Un buffer vacío se guarda como un archivo vacío.
const EOL: &str = "\n";

/// Tamaño del archivo tal como se guarda, para avisarlo como vim (`"a.md" 3L, 20B`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FileStats {
    pub lines: usize,
    pub bytes: usize,
}

impl FileStats {
    fn of(buffer: &Buffer) -> Self {
        if buffer.is_empty() {
            return Self { lines: 0, bytes: 0 };
        }
        Self {
            lines: buffer.line_count(),
            bytes: buffer.len().get() + EOL.len(),
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
    /// La ruta como se escribió, para mostrarla y guardar ahí.
    path: PathBuf,
    buffer: Buffer,
    /// Las marks de vim son de cada buffer; el motor las toma prestadas.
    marks: Marks,
    /// Estado del historial que coincide con el disco; `None` si se guardó
    /// a mitad de una transacción y ningún estado coincide.
    saved: Option<HistorySeq>,
    exists_on_disk: bool,
}

impl Document {
    /// Abre `path`; si no existe, empieza vacío y se crea al guardar. El `\n`
    /// final del archivo no entra al buffer: lo vuelve a poner `save`.
    ///
    /// # Errors
    /// Si existe pero no se puede leer como texto.
    pub fn open(path: &Path) -> Result<Self, FileError> {
        let text = nota_vault::read(path)?;
        let exists_on_disk = text.is_some();
        let text = text.unwrap_or_default();
        let mut document = Self::new(path, text.strip_suffix(EOL).unwrap_or(&text));
        document.exists_on_disk = exists_on_disk;
        Ok(document)
    }

    #[must_use]
    pub fn new(path: &Path, text: &str) -> Self {
        let buffer = Buffer::from(text);
        Self {
            path: path.to_owned(),
            saved: Some(buffer.history_seq()),
            marks: Marks::default(),
            buffer,
            exists_on_disk: false,
        }
    }

    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// `path` nombra el archivo de este documento, aunque se escriba de
    /// otra forma (relativa, con `..` o por un enlace). Se resuelven las dos
    /// rutas ahora, porque guardar sigue los enlaces como estén al guardar.
    #[must_use]
    pub fn is_file(&self, path: &Path) -> bool {
        resolve(path) == resolve(&self.path)
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

    /// Escribe el buffer con un `\n` al final, aunque el archivo no lo tuviera.
    ///
    /// # Errors
    /// Si no se pudo escribir; el archivo en disco queda como estaba.
    pub fn save(&mut self) -> Result<FileStats, FileError> {
        let eol = if self.buffer.is_empty() { "" } else { EOL };
        nota_vault::write_atomic(&self.path, |out| write!(out, "{}{eol}", self.buffer))?;
        self.saved = (!self.buffer.has_uncommitted_edits()).then(|| self.buffer.history_seq());
        self.exists_on_disk = true;
        Ok(self.stats())
    }

    #[must_use]
    pub fn buffer(&self) -> &Buffer {
        &self.buffer
    }

    /// El buffer para editarlo y sus marks para prestárselas al motor.
    pub(crate) fn editable(&mut self) -> (&mut Buffer, &mut Marks) {
        (&mut self.buffer, &mut self.marks)
    }

    /// # Errors
    /// Solo si el buffer está inconsistente, lo que sería un bug.
    pub fn line_text(&self, line: usize) -> Result<String, BufferError> {
        let range = self.buffer.line_start(line)..self.buffer.line_end(line);
        self.buffer.text_in(range)
    }
}

/// Absoluta y sin enlaces; de un archivo que todavía no existe se resuelve
/// la carpeta. Si ni eso existe, se queda con la ruta absoluta tal cual.
fn resolve(path: &Path) -> PathBuf {
    if let Ok(file) = path.canonicalize() {
        return file;
    }
    let absolute = std::path::absolute(path).unwrap_or_else(|_| path.to_owned());
    match (absolute.parent(), absolute.file_name()) {
        (Some(parent), Some(name)) => parent
            .canonicalize()
            .map_or_else(|_| absolute.clone(), |parent| parent.join(name)),
        _ => absolute,
    }
}

#[cfg(test)]
#[path = "document_tests.rs"]
mod tests;
