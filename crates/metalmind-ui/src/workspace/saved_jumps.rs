//! La jumplist del panel enfocado y las marks `A`–`Z` entre sesiones, como
//! el shada de nvim: al cerrar pasan de [`FileId`] a rutas del vault y al
//! abrir, de vuelta.

use std::path::Path;

use metalmind_buffer::Point;
use metalmind_vault::{SavedJump, SavedJumps};
use metalmind_vim::{FileId, Jump, JumpList};

use super::{Workspace, WorkspaceError};
use crate::finder::relative_to;

impl Workspace {
    /// La jumplist del panel enfocado y las marks `A`–`Z`, con rutas del
    /// vault de raíz `root`. Lo que cae fuera del vault no se guarda.
    ///
    /// # Errors
    /// Solo si el estado está inconsistente, lo que sería un bug.
    pub fn saved_jumps(&self, root: &Path) -> Result<SavedJumps, WorkspaceError> {
        let saved = |jump: Jump| -> Option<SavedJump> {
            let path = relative_to(root, self.files.path(jump.file)?)?;
            Some(SavedJump {
                path,
                line: jump.point.line,
                column: jump.point.column,
            })
        };
        let jumps = self
            .focused_jumps()?
            .into_iter()
            .filter_map(saved)
            .collect();
        let open = |file: FileId| {
            let document = self.documents.get(self.documents.of_file(file)?)?;
            Some((document.state(), document.buffer()))
        };
        let marks = self
            .vim
            .file_marks(open)
            .filter_map(|(letter, jump)| Some((letter, saved(jump)?)))
            .collect();
        Ok(SavedJumps { jumps, marks })
    }

    /// Repone lo guardado por [`Workspace::saved_jumps`] en otra sesión
    /// del vault de `root`: la jumplist en el panel enfocado y las marks.
    ///
    /// # Errors
    /// Solo si no existe el panel enfocado, lo que sería un bug.
    pub fn restore_jumps(&mut self, root: &Path, saved: &SavedJumps) -> Result<(), WorkspaceError> {
        let mut jump = |saved: &SavedJump| Jump {
            file: self.files.of_path(&root.join(saved.path.as_str())),
            point: Point::new(saved.line, saved.column),
        };
        let jumps: Vec<Jump> = saved.jumps.iter().map(&mut jump).collect();
        let marks: Vec<(char, Jump)> = saved
            .marks
            .iter()
            .map(|(letter, saved)| (*letter, jump(saved)))
            .collect();
        let focused = self.focused();
        self.panes
            .get_mut(&focused)
            .ok_or(WorkspaceError::NoPane(focused))?
            .state
            .jumps = JumpList::restored(jumps);
        for (letter, jump) in marks {
            self.vim.restore_file_mark(letter, jump);
        }
        Ok(())
    }

    /// La jumplist del panel enfocado, con lo editado en su documento que
    /// todavía no siguió.
    fn focused_jumps(&self) -> Result<Vec<Jump>, WorkspaceError> {
        let pane = self.pane(self.focused())?;
        let id = pane.document;
        let document = self.document(id)?;
        let file = self
            .documents
            .file(id)
            .ok_or(WorkspaceError::NoDocument(id))?;
        Ok(pane
            .state
            .jumps
            .followed(file, document.state(), document.buffer()))
    }
}

#[cfg(test)]
#[path = "saved_jumps_tests.rs"]
mod tests;
