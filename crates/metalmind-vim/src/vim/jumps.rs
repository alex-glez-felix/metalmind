//! La jumplist (`ctrl-o`, `ctrl-i`) y los saltos entre archivos. La
//! jumplist es de cada panel y el archivo, de cada buffer: quien aloja el
//! motor le presta la del panel que edita, como la ventana.

use metalmind_buffer::{Buffer, Selection, Selections};

use super::Vim;
use crate::jumplist::{Direction, FileId, Jump, JumpList, Landing};
use crate::lines::{clamp_normal, first_non_blank, last_char};
use crate::mark::BufferMark;
use crate::{KeyOutcome, Mode, VimError};

impl Vim {
    /// Cambia la jumplist del motor por `jumps`: antes de editar en otro
    /// panel se le da la suya y después se recupera.
    pub fn swap_jumps(&mut self, jumps: &mut JumpList) {
        std::mem::swap(&mut self.jumps, jumps);
    }

    /// El archivo del buffer que se va a editar, para la jumplist y las
    /// marks `A`–`Z`. Lo editado antes en otro buffer ya debe estar al día
    /// ([`Vim::catch_up`]).
    pub fn set_file(&mut self, file: FileId) {
        self.file = file;
        self.splices_seen = 0;
    }

    /// Lleva la jumplist prestada y las marks `A`–`Z` del archivo actual a
    /// lo que se editó en `buffer` desde la última vez. Quien aloja el motor
    /// lo llama antes de sacar los reemplazos con
    /// [`Buffer::take_splices`]; la jumplist de los otros paneles la mueve
    /// él con [`JumpList::adjust`].
    pub fn catch_up(&mut self, buffer: &Buffer) {
        let splices = buffer.splices().get(self.splices_seen..).unwrap_or(&[]);
        if !splices.is_empty() {
            self.jumps.adjust(self.file, splices);
            self.file_marks.adjust(self.file, splices);
        }
        self.splices_seen = buffer.splices().len();
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
    pub fn push_jump(
        &mut self,
        buffer: &mut Buffer,
        selections: &Selections,
    ) -> Result<(), VimError> {
        self.jumped(buffer, selections)
    }

    /// Pone el cursor tras un salto a otro archivo, ya con su buffer.
    pub fn land(&mut self, landing: Landing, buffer: &Buffer, selections: &mut Selections) {
        self.cursor_moved();
        let offset = buffer.offset_at_point(landing.point);
        let head = if landing.first_non_blank {
            let line = buffer.line_of(offset);
            first_non_blank(buffer, line).min(last_char(buffer, line))
        } else {
            clamp_normal(buffer, offset)
        };
        *selections = Selections::single(Selection::cursor(head));
    }

    /// Como `setpcmark` de vim: antes de un salto, el cursor queda como la
    /// mark de `''` y al final de la jumplist. Dentro de un `:g` solo cuenta
    /// el de antes de empezar.
    pub(super) fn jumped(
        &mut self,
        buffer: &mut Buffer,
        selections: &Selections,
    ) -> Result<(), VimError> {
        if self.global.is_some() {
            return Ok(());
        }
        self.catch_up(buffer);
        let head = selections.primary().head;
        self.marks.set(buffer, BufferMark::Previous, head)?;
        let point = buffer.point_at(head)?;
        self.jumps.push(Jump {
            file: self.file,
            point,
        });
        Ok(())
    }

    /// `ctrl-o` y `ctrl-i`, como `nv_pcmark` de vim. Si la entrada es de
    /// otro archivo, se lo pide a quien aloja el motor.
    pub(super) fn jump_in_list(
        &mut self,
        direction: Direction,
        buffer: &mut Buffer,
        selections: &mut Selections,
    ) -> Result<Option<KeyOutcome>, VimError> {
        let count = self.take_count().unwrap_or(1);
        let in_operator = matches!(self.mode, Mode::OperatorPending(_));
        self.reset();
        if in_operator {
            self.fail();
            return Ok(None);
        }
        self.catch_up(buffer);
        let head = selections.primary().head;
        let here = Jump {
            file: self.file,
            point: buffer.point_at(head)?,
        };
        let missing = &self.missing_files;
        let Some(step) = self
            .jumps
            .step(direction, count, here, |file| !missing.contains(&file))
        else {
            self.fail();
            return Ok(None);
        };
        if step.pushed_here {
            self.marks.set(buffer, BufferMark::Previous, head)?;
        }
        let landing = Landing {
            file: step.target.file,
            point: step.target.point,
            first_non_blank: false,
        };
        if landing.file != self.file {
            return Ok(Some(KeyOutcome::SwitchFile(landing)));
        }
        self.land(landing, buffer, selections);
        Ok(None)
    }
}
