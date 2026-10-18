//! `:normal {teclas}`: las teclas como si se teclearan, en cada línea del
//! rango o donde está el cursor.

use std::ops::RangeInclusive;

use metalmind_buffer::{Buffer, Selection, Selections};

use super::{Failure, done};
use crate::lines::Want;
use crate::register::Clipboard;
use crate::vim::Vim;
use crate::{CommandError, Key, KeyOutcome, Mode, VimError};

impl Vim {
    /// Todo es un solo paso de undo. Devuelve lo último que la UI tiene que
    /// avisar, como una búsqueda que dio la vuelta.
    pub(super) fn normal(
        &mut self,
        keys: &str,
        lines: Option<RangeInclusive<usize>>,
        buffer: &mut Buffer,
        selections: &mut Selections,
        clipboard: &mut dyn Clipboard,
    ) -> Result<KeyOutcome, Failure> {
        let keys: Vec<Key> = keys.chars().map(Key::Char).collect();
        self.ex_depth += 1;
        buffer.begin_undo_group();
        let notice = self.normal_on_lines(&keys, lines, buffer, selections, clipboard);
        buffer.end_undo_group();
        self.ex_depth -= 1;
        Ok(notice?.unwrap_or_else(done))
    }

    /// Como vim, en cada línea empieza en la columna 0 sin cambiar la
    /// columna buscada, y una línea que ya no existe es la última.
    fn normal_on_lines(
        &mut self,
        keys: &[Key],
        lines: Option<RangeInclusive<usize>>,
        buffer: &mut Buffer,
        selections: &mut Selections,
        clipboard: &mut dyn Clipboard,
    ) -> Result<Option<KeyOutcome>, VimError> {
        let Some(lines) = lines else {
            return self.normal_once(keys, buffer, selections, clipboard);
        };
        let mut notice = None;
        for line in lines {
            let line = line.min(buffer.last_line());
            let goal = Want::of(buffer, selections.primary(), true, self.viewport.columns()).goal();
            *selections = Selections::single(Selection {
                goal,
                ..Selection::cursor(buffer.line_start(line))
            });
            notice = self
                .normal_once(keys, buffer, selections, clipboard)?
                .or(notice);
        }
        Ok(notice)
    }

    /// Una pasada de las teclas. Lo que una tecla que falla descarta son
    /// solo las de esta pasada; lo que queda a medias se termina como con
    /// `<Esc>`, también Insert, pero Visual sigue.
    fn normal_once(
        &mut self,
        keys: &[Key],
        buffer: &mut Buffer,
        selections: &mut Selections,
        clipboard: &mut dyn Clipboard,
    ) -> Result<Option<KeyOutcome>, VimError> {
        let notice = self.play_isolated(keys, buffer, selections, clipboard)?;
        while self.is_incomplete() {
            self.process(Key::Escape, buffer, selections, clipboard)?;
        }
        self.failed = false;
        Ok(notice.map(|notice| match notice {
            KeyOutcome::Command(invocation) => KeyOutcome::InvalidCommand(
                CommandError::HostCommandInside(invocation.command.name().to_owned()),
            ),
            KeyOutcome::SwitchFile(_) => KeyOutcome::InvalidCommand(CommandError::FileSwitchInside),
            notice => notice,
        }))
    }

    /// Si a lo tecleado le falta algo para ser un comando completo.
    fn is_incomplete(&self) -> bool {
        matches!(
            self.mode,
            Mode::Insert | Mode::OperatorPending(_) | Mode::CommandLine
        ) || !self.pending.is_empty()
            || self.count.is_some()
            || self.operator_count.is_some()
            || self.selected.is_some()
            || self.awaiting.is_some()
    }
}
