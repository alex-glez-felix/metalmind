//! `:normal {teclas}`: las teclas como si se teclearan, en cada línea del
//! rango o donde está el cursor.

use std::ops::RangeInclusive;

use metalmind_buffer::{Selection, Selections};

use super::{Failure, done};
use crate::Target;
use crate::lines::Want;
use crate::vim::Vim;
use crate::{CommandError, Key, KeyOutcome, Mode, VimError};

impl Vim {
    /// Todo es un solo paso de undo. Devuelve lo último que la UI tiene que
    /// avisar, como una búsqueda que dio la vuelta.
    pub(super) fn normal(
        &mut self,
        keys: &str,
        lines: Option<RangeInclusive<usize>>,
        t: &mut Target<'_>,
    ) -> Result<KeyOutcome, Failure> {
        let keys: Vec<Key> = keys.chars().map(Key::Char).collect();
        self.ex_depth += 1;
        t.buffer.begin_undo_group();
        let notice = self.normal_on_lines(&keys, lines, t);
        t.buffer.end_undo_group();
        self.ex_depth -= 1;
        Ok(notice?.unwrap_or_else(done))
    }

    /// Como vim, en cada línea empieza en la columna 0 sin cambiar la
    /// columna buscada, y una línea que ya no existe es la última.
    fn normal_on_lines(
        &mut self,
        keys: &[Key],
        lines: Option<RangeInclusive<usize>>,
        t: &mut Target<'_>,
    ) -> Result<Option<KeyOutcome>, VimError> {
        let Some(lines) = lines else {
            return self.normal_once(keys, t);
        };
        let mut notice = None;
        for line in lines {
            let line = line.min(t.buffer.last_line());
            let goal = Want::of(
                t.buffer,
                t.selections.primary(),
                true,
                t.pane.viewport.columns(),
            )
            .goal();
            *t.selections = Selections::single(Selection {
                goal,
                ..Selection::cursor(t.buffer.line_start(line))
            });
            notice = self.normal_once(keys, t)?.or(notice);
        }
        Ok(notice)
    }

    /// Una pasada de las teclas. Lo que una tecla que falla descarta son
    /// solo las de esta pasada; lo que queda a medias se termina como con
    /// `<Esc>`, también Insert, pero Visual sigue.
    fn normal_once(
        &mut self,
        keys: &[Key],
        t: &mut Target<'_>,
    ) -> Result<Option<KeyOutcome>, VimError> {
        let notice = self.play_isolated(keys, t)?;
        while self.is_incomplete() {
            self.process(Key::Escape, t)?;
        }
        self.failed = false;
        Ok(notice.map(|notice| match notice {
            KeyOutcome::Dispatch(invocation) => KeyOutcome::InvalidCommand(
                CommandError::HostCommandInside(invocation.action.to_string()),
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
