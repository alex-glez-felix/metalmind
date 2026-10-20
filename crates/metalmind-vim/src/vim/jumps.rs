//! La jumplist (`ctrl-o`, `ctrl-i`) y los saltos entre archivos. La
//! jumplist es de cada panel ([`crate::PaneState`]) y el archivo, de cada
//! documento: llegan en el [`Target`].

use metalmind_buffer::{Selection, Selections};

use super::Vim;
use crate::Target;
use crate::jumplist::{Direction, FileId, Jump, Landing};
use crate::lines::{clamp_normal, first_non_blank, last_char};
use crate::mark::BufferMark;
use crate::{KeyOutcome, Mode, VimError};

impl Vim {
    /// Lleva la jumplist del panel y las marks `A`–`Z` del archivo a lo que
    /// se editó en el buffer y todavía no siguieron. La jumplist de los
    /// otros paneles la mueve quien saca los reemplazos con
    /// [`metalmind_buffer::Buffer::take_splices`] ([`crate::JumpList::adjust`]).
    pub(super) fn catch_up(&mut self, t: &mut Target<'_>) {
        let splices = t.doc.unfollowed(t.buffer);
        if !splices.is_empty() {
            t.pane.jumps.adjust(t.file, splices);
            self.file_marks.adjust(t.file, splices);
        }
        t.doc.followed(t.buffer);
    }

    /// El archivo dejó de existir: sus marks `A`–`Z` se pierden y `ctrl-o`
    /// salta sus entradas.
    pub fn file_removed(&mut self, file: FileId) {
        self.file_marks.forget(file);
        self.missing_files.insert(file);
    }

    /// El archivo volvió a existir, como al crear una nota con el nombre de
    /// una que se borró: sus entradas de la jumplist vuelven a valer.
    pub fn file_restored(&mut self, file: FileId) {
        self.missing_files.remove(&file);
    }

    /// Un salto que hace quien aloja el motor, como seguir un link o abrir
    /// otra nota: el cursor de ahora queda en la jumplist y como la mark de
    /// `''`.
    ///
    /// # Errors
    /// Solo si el cursor no cae en el buffer, lo que sería un bug.
    pub fn push_jump(&mut self, t: &mut Target<'_>) -> Result<(), VimError> {
        self.jumped(t)
    }

    /// Pone el cursor tras un salto a otro archivo, ya con su buffer.
    pub fn land(&mut self, landing: Landing, t: &mut Target<'_>) {
        self.cursor_moved();
        let offset = t.buffer.offset_at_point(landing.point);
        let head = if landing.first_non_blank {
            let line = t.buffer.line_of(offset);
            first_non_blank(t.buffer, line).min(last_char(t.buffer, line))
        } else {
            clamp_normal(t.buffer, offset)
        };
        *t.selections = Selections::single(Selection::cursor(head));
    }

    /// Como `setpcmark` de vim: antes de un salto, el cursor queda como la
    /// mark de `''` y al final de la jumplist. Dentro de un `:g` solo cuenta
    /// el de antes de empezar.
    pub(super) fn jumped(&mut self, t: &mut Target<'_>) -> Result<(), VimError> {
        if self.global.is_some() {
            return Ok(());
        }
        self.catch_up(t);
        let head = t.selections.primary().head;
        t.doc.marks.set(t.buffer, BufferMark::Previous, head)?;
        let point = t.buffer.point_at(head)?;
        t.pane.jumps.push(Jump {
            file: t.file,
            point,
        });
        Ok(())
    }

    /// `ctrl-o` y `ctrl-i`, como `nv_pcmark` de vim. Si la entrada es de
    /// otro archivo, se lo pide a quien aloja el motor.
    pub(super) fn jump_in_list(
        &mut self,
        direction: Direction,
        t: &mut Target<'_>,
    ) -> Result<Option<KeyOutcome>, VimError> {
        let count = self.take_count().unwrap_or(1);
        let in_operator = matches!(self.mode, Mode::OperatorPending(_));
        self.reset();
        if in_operator {
            self.fail();
            return Ok(None);
        }
        self.catch_up(t);
        let head = t.selections.primary().head;
        let here = Jump {
            file: t.file,
            point: t.buffer.point_at(head)?,
        };
        let missing = &self.missing_files;
        let Some(step) = t
            .pane
            .jumps
            .step(direction, count, here, |file| !missing.contains(&file))
        else {
            self.fail();
            return Ok(None);
        };
        if step.pushed_here {
            t.doc.marks.set(t.buffer, BufferMark::Previous, head)?;
        }
        let landing = Landing {
            file: step.target.file,
            point: step.target.point,
            first_non_blank: false,
        };
        if landing.file != t.file {
            return Ok(Some(KeyOutcome::SwitchFile(landing)));
        }
        self.land(landing, t);
        Ok(None)
    }
}
