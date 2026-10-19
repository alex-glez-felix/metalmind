//! Una nota abierta, sin nada de GPUI: su texto con el historial y el
//! archivo donde se guarda. Los cursores son de cada panel y el estado de
//! vim es uno solo para todos (ver [`Workspace`](crate::workspace::Workspace)).

use std::fmt;
use std::path::{Path, PathBuf};

use metalmind_buffer::{Buffer, BufferError, HistorySeq};
use metalmind_vault::FileError;
use metalmind_vim::{Layout, Marks, Rows};

/// Como el `fixeol` de vim: en disco cada línea termina en `\n`, y en el buffer
/// la última no lo lleva.
const EOL: &str = "\n";

/// Lo que vim muestra en lugar de la ruta de un buffer sin archivo.
const NO_NAME: &str = "[No Name]";

#[derive(Debug, thiserror::Error)]
pub enum SaveError {
    #[error("E32: No file name")]
    NoFileName,
    #[error("E13: File exists (add ! to override)")]
    Exists,
    #[error(transparent)]
    File(#[from] FileError),
}

/// Tamaño del archivo, para avisarlo como vim (`"a.md" 3L, 20B`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FileStats {
    pub lines: usize,
    pub bytes: usize,
}

impl fmt::Display for FileStats {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}L, {}B", self.lines, self.bytes)
    }
}

/// Lo que el motor toma prestado de un documento mientras edita.
pub(crate) struct Editable<'a> {
    pub buffer: &'a mut Buffer,
    pub marks: &'a mut Marks,
    pub layout: &'a mut Option<Layout>,
}

#[derive(Debug)]
pub struct Document {
    /// La ruta como se escribió, para mostrarla y guardar ahí; `None` en
    /// un buffer sin archivo, como el de abrir un vault sin nota.
    path: Option<PathBuf>,
    buffer: Buffer,
    /// Las marks de vim son de cada buffer; el motor las toma prestadas.
    marks: Marks,
    /// El estilo de cada línea y lo que mide; también lo toma prestado el
    /// motor, que lo sigue al editar. Siempre está, salvo mientras lo tiene.
    layout: Option<Layout>,
    /// Estado del historial que coincide con el disco; `None` si se guardó
    /// a mitad de una transacción y ningún estado coincide.
    saved: Option<HistorySeq>,
    exists_on_disk: bool,
    /// El archivo era una sola línea vacía. Vim distingue "sin líneas" de
    /// "una línea vacía", y el buffer vacío no: sin esto, `:w` lo vaciaría.
    blank_line: bool,
    /// Al archivo le faltaba el `\n` final y todavía no se guardó.
    noeol: bool,
}

impl Document {
    /// Abre `path`; si no existe, empieza vacío y se crea al guardar. El `\n`
    /// final del archivo no entra al buffer: lo vuelve a poner `save`.
    ///
    /// # Errors
    /// Si existe pero no se puede leer como texto.
    pub fn open(path: &Path) -> Result<Self, FileError> {
        let text = metalmind_vault::read(path)?;
        let exists_on_disk = text.is_some();
        let text = text.unwrap_or_default();
        let lines = text.strip_suffix(EOL);
        let mut document = Self::new(path, lines.unwrap_or(&text));
        document.exists_on_disk = exists_on_disk;
        document.blank_line = text == EOL;
        document.noeol = lines.is_none() && !text.is_empty();
        Ok(document)
    }

    #[must_use]
    pub fn new(path: &Path, text: &str) -> Self {
        Self::with_path(Some(path.to_owned()), text)
    }

    /// Un buffer vacío sin archivo: `:w` pide una ruta, como en vim.
    #[must_use]
    pub fn unnamed() -> Self {
        Self::with_path(None, "")
    }

    fn with_path(path: Option<PathBuf>, text: &str) -> Self {
        let buffer = Buffer::from(text);
        Self {
            path,
            saved: Some(buffer.history_seq()),
            marks: Marks::default(),
            layout: Some(Layout::new(&buffer)),
            buffer,
            exists_on_disk: false,
            blank_line: false,
            noeol: false,
        }
    }

    #[must_use]
    pub fn path(&self) -> Option<&Path> {
        self.path.as_deref()
    }

    /// La ruta para mostrar, o `[No Name]` como vim.
    #[must_use]
    pub fn display_name(&self) -> String {
        self.path
            .as_deref()
            .map_or_else(|| NO_NAME.to_owned(), |path| path.display().to_string())
    }

    /// `path` nombra el archivo de este documento, aunque se escriba de
    /// otra forma (relativa, con `..` o por un enlace). Se resuelven las dos
    /// rutas ahora, porque guardar sigue los enlaces como estén al guardar.
    #[must_use]
    pub fn is_file(&self, path: &Path) -> bool {
        self.resolved_path().is_some_and(|own| own == resolve(path))
    }

    /// La ruta absoluta y sin enlaces de su archivo, si tiene.
    #[must_use]
    pub fn resolved_path(&self) -> Option<PathBuf> {
        self.path.as_deref().map(resolve)
    }

    /// La ruta absoluta de la entrada en su carpeta, con las carpetas sin
    /// enlaces pero sin seguir el último: si el archivo es un enlace, la del
    /// enlace. Es lo que se mueve al renombrar o mover una carpeta.
    #[must_use]
    pub fn entry_path(&self) -> Option<PathBuf> {
        self.path.as_deref().map(entry_path)
    }

    /// Su archivo pasó a llamarse `path`: se renombró desde nota.
    pub(crate) fn set_path(&mut self, path: PathBuf) {
        self.path = Some(path);
    }

    #[must_use]
    pub fn exists_on_disk(&self) -> bool {
        self.exists_on_disk
    }

    /// Representa el mismo archivo en disco que `other`: si existe, su texto
    /// y sus saltos de línea finales, que el texto del buffer no distingue.
    #[must_use]
    pub fn same_on_disk(&self, other: &Self) -> bool {
        self.exists_on_disk == other.exists_on_disk
            && self.blank_line == other.blank_line
            && self.noeol == other.noeol
            && self.buffer.to_string() == other.buffer.to_string()
    }

    /// Lo que ocuparía en disco al guardarlo ahora.
    #[must_use]
    pub fn stats(&self) -> FileStats {
        if self.final_eol().is_empty() {
            return FileStats { lines: 0, bytes: 0 };
        }
        FileStats {
            lines: self.buffer.line_count(),
            bytes: self.buffer.len().get() + EOL.len(),
        }
    }

    /// Como lo avisa vim al abrir: sin el `\n` que le falta al archivo, si
    /// le falta (`noeol`).
    #[must_use]
    pub fn opened_stats(&self) -> FileStats {
        let stats = self.stats();
        if self.noeol() {
            // Sin cambios, el buffer es el texto del archivo, que no está vacío.
            FileStats {
                bytes: stats.bytes - EOL.len(),
                ..stats
            }
        } else {
            stats
        }
    }

    /// Al archivo le falta el `\n` final y el buffer sigue como en disco.
    #[must_use]
    pub fn noeol(&self) -> bool {
        self.noeol && !self.is_modified()
    }

    /// Un buffer vacío no lleva `\n`, salvo que se abriera de una línea vacía.
    fn final_eol(&self) -> &'static str {
        if self.buffer.is_empty() && !self.blank_line {
            ""
        } else {
            EOL
        }
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
    /// Si no tiene archivo o no se pudo escribir; el archivo en disco queda
    /// como estaba.
    pub fn save(&mut self) -> Result<FileStats, SaveError> {
        let path = self.path.as_deref().ok_or(SaveError::NoFileName)?;
        let eol = self.final_eol();
        metalmind_vault::write_atomic(path, |out| write!(out, "{}{eol}", self.buffer))?;
        Ok(self.mark_saved())
    }

    /// El disco ya tiene lo que tiene el buffer.
    fn mark_saved(&mut self) -> FileStats {
        self.saved = (!self.buffer.has_uncommitted_edits()).then(|| self.buffer.history_seq());
        // Ahora el archivo es lo que se escribió: una sola línea vacía solo
        // si el buffer sigue vacío.
        self.blank_line &= self.buffer.is_empty();
        self.exists_on_disk = true;
        self.noeol = false;
        self.stats()
    }

    /// `:w ruta`, como en vim: si es su archivo, lo guarda; un documento
    /// sin archivo pasa a tener ese, y uno con otro archivo escribe una
    /// copia y sigue en el suyo. Nunca pisa un archivo que ya existe.
    ///
    /// # Errors
    /// Si `path` ya existe, aunque aparezca mientras se escribe, o no se
    /// pudo escribir.
    pub fn save_to(&mut self, path: &Path) -> Result<FileStats, SaveError> {
        if self.is_file(path) {
            return self.save();
        }
        let eol = self.final_eol();
        metalmind_vault::create_atomic(path, |out| write!(out, "{}{eol}", self.buffer)).map_err(
            |error| match error {
                FileError::Exists { .. } => SaveError::Exists,
                error => SaveError::File(error),
            },
        )?;
        if self.path.is_none() {
            self.path = Some(path.to_owned());
            return Ok(self.mark_saved());
        }
        Ok(self.stats())
    }

    #[must_use]
    pub fn marks(&self) -> &Marks {
        &self.marks
    }

    #[must_use]
    pub fn buffer(&self) -> &Buffer {
        &self.buffer
    }

    /// Este documento, releído de disco, se queda con las marks de `old`
    /// en su línea y columna, como vim al releer un archivo.
    ///
    /// # Errors
    /// Solo si una mark no cae en el buffer de `old`, lo que sería un bug.
    pub(crate) fn keep_marks_of(&mut self, old: &Self) -> Result<(), BufferError> {
        self.marks = old.marks.carried(&old.buffer, &mut self.buffer)?;
        Ok(())
    }

    /// Lo que mide una línea de texto y cada heading.
    pub(crate) fn set_rows(&mut self, rows: Rows) {
        if let Some(layout) = self.layout.as_mut() {
            layout.set_rows(rows);
        }
    }

    /// El estilo de las líneas, para pintarlas.
    #[must_use]
    pub fn layout(&self) -> Option<&Layout> {
        self.layout.as_ref()
    }

    /// El buffer para editarlo, y sus marks y su layout para prestárselos
    /// al motor.
    pub(crate) fn editable(&mut self) -> Editable<'_> {
        Editable {
            buffer: &mut self.buffer,
            marks: &mut self.marks,
            layout: &mut self.layout,
        }
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
    entry_path(path)
}

/// Absoluta, con su carpeta sin enlaces y el nombre tal cual: la de la
/// entrada, aunque sea un enlace. Si la carpeta no existe, la ruta
/// absoluta tal cual.
pub(crate) fn entry_path(path: &Path) -> PathBuf {
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
