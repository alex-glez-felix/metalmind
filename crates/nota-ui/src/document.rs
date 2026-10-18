//! Una nota abierta, sin nada de GPUI: su texto con el historial y el
//! archivo donde se guarda. Los cursores son de cada panel y el estado de
//! vim es uno solo para todos (ver [`Workspace`](crate::workspace::Workspace)).

use std::fmt;
use std::path::{Path, PathBuf};

use nota_buffer::{Buffer, BufferError, HistorySeq};
use nota_vault::FileError;
use nota_vim::Marks;

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

#[derive(Debug)]
pub struct Document {
    /// La ruta como se escribió, para mostrarla y guardar ahí; `None` en
    /// un buffer sin archivo, como el de abrir un vault sin nota.
    path: Option<PathBuf>,
    buffer: Buffer,
    /// Las marks de vim son de cada buffer; el motor las toma prestadas.
    marks: Marks,
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
        let text = nota_vault::read(path)?;
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

    /// Su archivo pasó a llamarse `path`: se renombró desde nota.
    pub(crate) fn set_path(&mut self, path: PathBuf) {
        self.path = Some(path);
    }

    #[must_use]
    pub fn exists_on_disk(&self) -> bool {
        self.exists_on_disk
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
        self.write(path)?;
        self.saved = (!self.buffer.has_uncommitted_edits()).then(|| self.buffer.history_seq());
        self.exists_on_disk = true;
        self.noeol = false;
        Ok(self.stats())
    }

    /// `:w ruta`, como en vim: si es su archivo, lo guarda; un documento
    /// sin archivo pasa a tener ese, y uno con otro archivo escribe una
    /// copia y sigue en el suyo. Nunca pisa un archivo que ya existe.
    ///
    /// # Errors
    /// Si `path` ya existe o no se pudo escribir.
    pub fn save_to(&mut self, path: &Path) -> Result<FileStats, SaveError> {
        if self.is_file(path) {
            return self.save();
        }
        if path.symlink_metadata().is_ok() {
            return Err(SaveError::Exists);
        }
        if self.path.is_none() {
            self.path = Some(path.to_owned());
            return self.save().inspect_err(|_| self.path = None);
        }
        self.write(path)?;
        Ok(self.stats())
    }

    fn write(&self, path: &Path) -> Result<(), FileError> {
        let eol = self.final_eol();
        nota_vault::write_atomic(path, |out| write!(out, "{}{eol}", self.buffer))
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
