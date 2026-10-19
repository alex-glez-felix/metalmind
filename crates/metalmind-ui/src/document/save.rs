//! Guardar un documento sin pisar lo que cambió en disco desde que se leyó
//! o se escribió por última vez.

use std::path::Path;

use metalmind_vault::{ContentHash, FileError, SaveKind};

use super::{Document, FileStats, SaveError};
use crate::disk::Disk;

impl Document {
    /// Escribe el buffer con un `\n` al final, aunque el archivo no lo
    /// tuviera. Si el archivo cambió en disco desde que se leyó o se
    /// escribió, no lo pisa.
    ///
    /// # Errors
    /// Si no tiene archivo, cambió en disco o no se pudo escribir; el
    /// archivo en disco queda como estaba.
    pub fn save(&mut self, disk: &dyn Disk, kind: SaveKind) -> Result<FileStats, SaveError> {
        self.path.as_deref().ok_or(SaveError::NoFileName)?;
        if self.check_disk()? {
            return Err(SaveError::ChangedOnDisk);
        }
        self.overwrite(disk, kind)
    }

    /// Como [`Document::save`], aunque el archivo haya cambiado en disco: `:w!`.
    ///
    /// # Errors
    /// Si no tiene archivo o no se pudo escribir.
    pub fn overwrite(&mut self, disk: &dyn Disk, kind: SaveKind) -> Result<FileStats, SaveError> {
        let path = self.path.as_deref().ok_or(SaveError::NoFileName)?;
        let text = self.file_text();
        disk.write(path, &text, kind)?;
        Ok(self.mark_saved(&text))
    }

    /// Si el archivo ya no es lo que se leyó o se escribió por última vez;
    /// queda anotado hasta que vuelva a serlo, se guarde o se relea.
    ///
    /// # Errors
    /// Si existe pero no se puede leer.
    pub fn check_disk(&mut self) -> Result<bool, FileError> {
        let Some(path) = self.path.as_deref() else {
            return Ok(false);
        };
        let now = metalmind_vault::read(path)?;
        self.changed_on_disk = now.as_deref().map(ContentHash::of) != self.on_disk;
        Ok(self.changed_on_disk)
    }

    /// El archivo cambió fuera de Metalmind desde que se leyó o se escribió.
    #[must_use]
    pub fn changed_on_disk(&self) -> bool {
        self.changed_on_disk
    }

    /// Lo que se escribe en el archivo: el buffer y, salvo que esté vacío, un
    /// `\n` al final.
    fn file_text(&self) -> String {
        format!("{}{}", self.buffer, self.final_eol())
    }

    /// El disco ya tiene `text`, lo que tiene el buffer.
    fn mark_saved(&mut self, text: &str) -> FileStats {
        self.saved = (!self.buffer.has_uncommitted_edits()).then(|| self.buffer.history_seq());
        self.written = self.buffer.version();
        self.on_disk = Some(ContentHash::of(text));
        self.changed_on_disk = false;
        // Ahora el archivo es lo que se escribió: una sola línea vacía solo
        // si el buffer sigue vacío.
        self.blank_line &= self.buffer.is_empty();
        self.noeol = false;
        self.stats()
    }

    /// Lo guardado a mitad de un Insert coincide con el estado que dejó al
    /// cerrarse, si no se editó nada después.
    pub(crate) fn settle(&mut self) {
        let unchanged =
            !self.buffer.has_uncommitted_edits() && self.buffer.version() == self.written;
        if self.saved.is_none() && unchanged {
            self.saved = Some(self.buffer.history_seq());
        }
    }

    /// `:w ruta`, como en vim: si es su archivo, lo guarda; un documento
    /// sin archivo pasa a tener ese, y uno con otro archivo escribe una
    /// copia y sigue en el suyo. Sin `force` nunca pisa un archivo que ya
    /// existe; con él (`:w! ruta`), sí.
    ///
    /// # Errors
    /// Si `path` ya existe sin `force`, aunque aparezca mientras se escribe,
    /// o no se pudo escribir.
    pub fn save_to(
        &mut self,
        path: &Path,
        disk: &dyn Disk,
        force: bool,
    ) -> Result<FileStats, SaveError> {
        let kind = SaveKind::Explicit;
        if self.is_file(path) {
            return if force {
                self.overwrite(disk, kind)
            } else {
                self.save(disk, kind)
            };
        }
        let text = self.file_text();
        if force {
            disk.write(path, &text, kind)?;
        } else {
            metalmind_vault::create_atomic(path, |out| out.write_all(text.as_bytes())).map_err(
                |error| match error {
                    FileError::Exists { .. } => SaveError::Exists,
                    error => SaveError::File(error),
                },
            )?;
        }
        if self.path.is_none() {
            self.path = Some(path.to_owned());
            return Ok(self.mark_saved(&text));
        }
        Ok(self.stats())
    }
}
