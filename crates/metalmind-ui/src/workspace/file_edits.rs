//! Ediciones de un archivo que llegan de fuera del editor, como renombrar
//! un tag, sobre su documento abierto con cambios sin guardar: entran en su
//! undo y el disco no se toca (05 §5).

use std::path::{Path, PathBuf};

use metalmind_buffer::{ByteOffset, Edit};
use metalmind_vault::{FileEdits, StaleEdit};
use metalmind_vim::{PaneState, Target};

use super::{DocumentId, Workspace, WorkspaceError};
use crate::document::Document;

/// Por qué no se aplicaron unas ediciones a un documento.
#[derive(Debug, thiserror::Error)]
pub enum FileEditError {
    #[error("{} no está abierto", .0.display())]
    NotOpen(PathBuf),
    /// Un Insert abierto ya tiene su paso de undo: meterlas ahí las
    /// mezclaría con lo tecleado.
    #[error("{} se está editando en Insert", .0.display())]
    Typing(PathBuf),
    #[error(transparent)]
    Stale(#[from] StaleEdit),
    /// El `\n` final no está en el buffer: lo pone el guardado.
    #[error("{} perdería el salto de línea final", .0.display())]
    FinalNewline(PathBuf),
    #[error(transparent)]
    Workspace(#[from] WorkspaceError),
}

impl Workspace {
    /// Si `edits` se pueden aplicar ya al documento abierto de `path`, sin
    /// aplicarlas.
    ///
    /// # Errors
    /// Si no está abierto, tiene un Insert abierto o su texto ya no dice lo
    /// que las ediciones esperan.
    pub fn check_file_edits(&self, path: &Path, edits: &FileEdits) -> Result<(), FileEditError> {
        let id = self.editable_document(path)?;
        buffer_edits(path, self.document(id)?, edits)?;
        Ok(())
    }

    /// El texto del documento abierto de `path` como se guardaría, con su
    /// `\n` final; sobre él se planean y comprueban las ediciones.
    #[must_use]
    pub fn file_text(&self, path: &Path) -> Option<String> {
        let id = self.documents.find(path)?;
        self.documents.get(id).map(Document::file_text)
    }

    /// Aplica `edits`, sobre su texto como se guardaría
    /// ([`Workspace::file_text`]), al documento abierto de `path` como un
    /// solo paso de undo, se vea en uno o más paneles o esté oculto: los cursores, las
    /// marks y las jumplists siguen al texto. Devuelve las ediciones que lo
    /// devuelven a como estaba.
    ///
    /// # Errors
    /// Los de [`Workspace::check_file_edits`]; entonces no cambia nada.
    pub fn edit_file(
        &mut self,
        path: &Path,
        edits: &FileEdits,
    ) -> Result<FileEdits, FileEditError> {
        let id = self.editable_document(path)?;
        let (edits, undo) = buffer_edits(path, self.document(id)?, edits)?;
        let shown = self.panes_showing(id).next();
        match shown {
            Some(pane) => {
                self.edit_pane(
                    pane,
                    None,
                    |vim, target| Ok(vim.apply_edits(edits, target)?),
                )?;
            }
            None => self.edit_hidden(id, edits)?,
        }
        Ok(undo)
    }

    fn editable_document(&self, path: &Path) -> Result<DocumentId, FileEditError> {
        let id = self
            .documents
            .find(path)
            .ok_or_else(|| FileEditError::NotOpen(path.to_owned()))?;
        if self.document(id)?.buffer().has_open_transaction() {
            return Err(FileEditError::Typing(path.to_owned()));
        }
        // Un documento que ni se ve ni está oculto no tiene cursor que mover.
        if self.panes_showing(id).next().is_none() && !self.hidden.contains_key(&id) {
            return Err(WorkspaceError::NoDocument(id).into());
        }
        Ok(id)
    }

    /// Como [`Workspace::edit_pane`] para un documento que ningún panel
    /// muestra: su cursor guardado sigue al texto, y las jumplists de todos
    /// los paneles también.
    fn edit_hidden(&mut self, id: DocumentId, edits: Vec<Edit>) -> Result<(), WorkspaceError> {
        let file = self
            .documents
            .file(id)
            .ok_or(WorkspaceError::NoDocument(id))?;
        let last = self
            .hidden
            .get_mut(&id)
            .ok_or(WorkspaceError::NoDocument(id))?;
        let document = self
            .documents
            .get_mut(id)
            .ok_or(WorkspaceError::NoDocument(id))?;
        let editable = document.editable();
        // La ventana de un panel que no existe: el motor la mueve y se
        // guarda como la del documento oculto.
        let mut pane = PaneState::default();
        pane.viewport = last.viewport;
        self.vim.apply_edits(
            edits,
            &mut Target {
                buffer: &mut *editable.buffer,
                selections: &mut last.selections,
                doc: &mut *editable.state,
                pane: &mut pane,
                file,
                clipboard: None,
            },
        )?;
        last.viewport = pane.viewport;
        let splices = editable.buffer.take_splices();
        document.settle();
        for pane in self.panes.values_mut() {
            pane.state.jumps.adjust(file, &splices);
        }
        self.edited(id);
        Ok(())
    }
}

/// `edits` sobre el buffer de `document`, que no tiene el `\n` final de
/// su archivo (lo pone el guardado), y las que lo devuelven.
fn buffer_edits(
    path: &Path,
    document: &Document,
    edits: &FileEdits,
) -> Result<(Vec<Edit>, FileEdits), FileEditError> {
    let text = document.file_text();
    let (_, undo) = edits.apply(&text)?;
    let len = document.buffer().len().get();
    let has_eol = text.len() > len;
    let last = edits.edits.len().saturating_sub(1);
    let edits = edits
        .edits
        .iter()
        .enumerate()
        .map(|(at, edit)| {
            let mut new = edit.new.clone();
            if edit.range.start > len {
                new.insert(0, '\n');
            }
            // Lo que llega al final del archivo tiene que dejar su `\n`, que
            // en el buffer no se escribe.
            let past = edit.range.end > len;
            if past || (!has_eol && at == last && edit.range.end == len) {
                if new.ends_with('\n') {
                    new.pop();
                } else if past {
                    return Err(FileEditError::FinalNewline(path.to_owned()));
                }
            }
            let range = edit.range.start.min(len)..edit.range.end.min(len);
            Ok(Edit {
                range: ByteOffset::new(range.start)..ByteOffset::new(range.end),
                text: new,
            })
        })
        .collect::<Result<_, _>>()?;
    Ok((edits, undo))
}

#[cfg(test)]
#[path = "file_edits_tests.rs"]
mod tests;
