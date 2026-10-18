//! De una acción con nombre a lo que hace en cada modo.

use metalmind_buffer::{Buffer, Selection, Selections};

use super::Vim;
use super::awaiting::Awaiting;
use crate::insert::Typed;
use crate::jumplist::Direction;
use crate::matching;
use crate::motion::Destination;
use crate::object::Object;
use crate::register::Clipboard;
use crate::travel::Travel;
use crate::{Action, KeyOutcome, Mode, Motion, Operator, Prompt, VimError};

impl Vim {
    pub(super) fn run(
        &mut self,
        action: Action,
        buffer: &mut Buffer,
        selections: &mut Selections,
        clipboard: &mut dyn Clipboard,
    ) -> Result<KeyOutcome, VimError> {
        self.awaiting = None;
        match self.mode {
            Mode::Insert => self.run_in_insert(action, buffer, selections)?,
            Mode::CommandLine => {
                return self.run_in_command_line(action, buffer, selections, clipboard);
            }
            Mode::Normal | Mode::OperatorPending(_) => {
                return self.run_in_normal(action, buffer, selections, clipboard);
            }
            Mode::Visual(kind) => {
                return self.run_in_visual(action, kind, buffer, selections, clipboard);
            }
        }
        Ok(KeyOutcome::Executed(action))
    }

    /// Normal y operador pendiente.
    fn run_in_normal(
        &mut self,
        action: Action,
        buffer: &mut Buffer,
        selections: &mut Selections,
        clipboard: &mut dyn Clipboard,
    ) -> Result<KeyOutcome, VimError> {
        match (action, self.mode) {
            (Action::Motion(motion), Mode::OperatorPending(operator)) => {
                if motion.is_jump() && self.lands(motion, buffer, selections) {
                    self.jumped(buffer, selections)?;
                }
                let object = Object::Motion(motion).into();
                self.operate(operator, object, buffer, selections, clipboard)?;
            }
            (Action::Motion(motion), Mode::Normal) => self.move_by(motion, buffer, selections)?,
            (Action::FindChar(search), Mode::Normal | Mode::OperatorPending(_)) => {
                self.awaiting = Some(Awaiting::Find(search));
            }
            (Action::RepeatFind { reversed }, Mode::Normal | Mode::OperatorPending(_)) => {
                self.repeat_find(reversed, buffer, selections, clipboard)?;
            }
            (Action::Search { backward }, Mode::Normal | Mode::OperatorPending(_)) => {
                self.open_search(backward);
            }
            (Action::SearchNext { reversed }, Mode::Normal | Mode::OperatorPending(_)) => {
                return self.search_next(reversed, buffer, selections, clipboard);
            }
            (
                Action::SearchWord {
                    backward,
                    whole_word,
                },
                Mode::Normal | Mode::OperatorPending(_),
            ) => return self.search_word(backward, whole_word, buffer, selections, clipboard),
            (Action::Scroll(scroll), Mode::Normal) => self.scroll(scroll, buffer, selections),
            (
                Action::Reposition {
                    place,
                    first_non_blank,
                },
                Mode::Normal,
            ) => self.reposition(place, first_non_blank, buffer, selections)?,
            (Action::ClearSearchHighlight, Mode::Normal) => {
                self.reset();
                self.search.highlight = false;
            }
            (Action::TextObject(object), Mode::OperatorPending(operator)) => {
                let object = Object::TextObject(object).into();
                self.operate(operator, object, buffer, selections, clipboard)?;
            }
            (Action::CurrentLine, Mode::OperatorPending(operator)) => {
                let object = Object::CurrentLine.into();
                self.operate(operator, object, buffer, selections, clipboard)?;
            }
            (Action::Operator(operator), Mode::Normal) => {
                self.operator_count = self.take_count();
                self.mode = Mode::OperatorPending(operator);
            }
            (Action::SelectRegister, Mode::Normal) => self.awaiting = Some(Awaiting::Register),
            (Action::RecordMacro, Mode::Normal) if self.recording.is_some() => {
                self.stop_recording();
            }
            (Action::RecordMacro, Mode::Normal) => self.awaiting = Some(Awaiting::Record),
            (Action::PlayMacro, Mode::Normal) => self.awaiting = Some(Awaiting::Play),
            (Action::SetMark, Mode::Normal) => self.awaiting = Some(Awaiting::SetMark),
            (Action::GoToMark { exact }, Mode::Normal | Mode::OperatorPending(_)) => {
                self.awaiting = Some(Awaiting::Mark { exact });
            }
            (Action::JumpOlder, Mode::Normal | Mode::OperatorPending(_)) => {
                let jumped = self.jump_in_list(Direction::Older, buffer, selections)?;
                return Ok(jumped.unwrap_or(KeyOutcome::Executed(action)));
            }
            (Action::JumpNewer, Mode::Normal | Mode::OperatorPending(_)) => {
                let jumped = self.jump_in_list(Direction::Newer, buffer, selections)?;
                return Ok(jumped.unwrap_or(KeyOutcome::Executed(action)));
            }
            (Action::DeleteRight, Mode::Normal) => {
                let object = Object::Motion(Motion::Right).into();
                self.operate(Operator::Delete, object, buffer, selections, clipboard)?;
            }
            (Action::Visual(kind), Mode::Normal) => self.enter_visual(kind, buffer, selections),
            (Action::RestoreVisual, Mode::Normal) => self.restore_visual(buffer, selections),
            (Action::Insert(at), Mode::Normal) => self.enter_insert(at, buffer, selections)?,
            (Action::Paste(side), Mode::Normal) => {
                self.paste(side, buffer, selections, clipboard)?;
            }
            (Action::RepeatChange, Mode::Normal) => {
                self.repeat_change(buffer, selections, clipboard)?;
            }
            (Action::Undo, Mode::Normal) => self.travel(Travel::Undo, buffer, selections)?,
            (Action::Redo, Mode::Normal) => self.travel(Travel::Redo, buffer, selections)?,
            (Action::Earlier, Mode::Normal) => self.travel(Travel::Earlier, buffer, selections)?,
            (Action::Later, Mode::Normal) => self.travel(Travel::Later, buffer, selections)?,
            (Action::CommandLine, Mode::Normal) => self.open_command_line(Prompt::Command),
            (Action::Command(command), Mode::Normal) => {
                self.reset();
                return Ok(KeyOutcome::Command(command.into()));
            }
            _ => self.reset(),
        }
        Ok(KeyOutcome::Executed(action))
    }

    fn run_in_insert(
        &mut self,
        action: Action,
        buffer: &mut Buffer,
        selections: &mut Selections,
    ) -> Result<(), VimError> {
        match action {
            Action::NormalMode => self.finish_insert(buffer, selections)?,
            Action::Interrupt => self.leave_insert(buffer, selections)?,
            Action::Backspace => self.type_in_insert(&Typed::Backspace, buffer, selections)?,
            Action::Newline => {
                self.type_in_insert(&Typed::Text("\n".to_owned()), buffer, selections)?;
            }
            _ => self.reset(),
        }
        Ok(())
    }

    fn run_in_command_line(
        &mut self,
        action: Action,
        buffer: &mut Buffer,
        selections: &mut Selections,
        clipboard: &mut dyn Clipboard,
    ) -> Result<KeyOutcome, VimError> {
        match action {
            Action::NormalMode | Action::Interrupt => self.close_command_line(),
            Action::Backspace => self.command_line_backspace(),
            Action::HistoryPrevious => self.recall(true),
            Action::HistoryNext => self.recall(false),
            Action::SubmitCommand => {
                return self.submit_command_line(buffer, selections, clipboard);
            }
            _ => self.reset(),
        }
        Ok(KeyOutcome::Executed(action))
    }

    /// Una motion en Normal o, extendiendo la selección, en Visual. Como en
    /// vim, falla si falla en todos los cursores, y `h` y `l` también si no
    /// se mueven.
    pub(super) fn move_by(
        &mut self,
        motion: Motion,
        buffer: &mut Buffer,
        selections: &mut Selections,
    ) -> Result<(), VimError> {
        if let Motion::Window(place) = motion {
            return self.move_to_window_line(place, buffer, selections);
        }
        let count = self.take_count();
        self.reset();
        let purpose = self.motion_purpose();
        let moved = move_cursors(selections, self.in_visual(), |s| {
            let destination = motion.resolve(buffer, s, count, purpose, &self.viewport)?;
            let stuck = motion.fails_without_moving() && destination.offset == s.head;
            (!stuck).then_some(destination)
        });
        match moved {
            Some(moved) => {
                if motion.is_jump() {
                    self.jumped(buffer, selections)?;
                }
                *selections = moved;
            }
            None => self.fail(),
        }
        Ok(())
    }
}

/// Cada cursor va a donde lo lleve `resolve`; si falla, se queda. En Visual
/// el otro extremo de la selección no se mueve. `None` si falla en todos.
pub(super) fn move_cursors(
    selections: &Selections,
    visual: bool,
    resolve: impl Fn(&Selection) -> Option<Destination>,
) -> Option<Selections> {
    let moves = selections.map_to(|selection| (*selection, resolve(selection)));
    if moves.iter().all(|(_, destination)| destination.is_none()) {
        return None;
    }
    Some(moves.to_selections(|(selection, destination)| {
        destination.map_or(*selection, |destination| Selection {
            anchor: if visual {
                selection.anchor
            } else {
                destination.offset
            },
            head: destination.offset,
            goal: destination.goal,
        })
    }))
}

impl Vim {
    /// Si un salto como objeto de operador llega a algún lado: un `d%` sin
    /// pareja falla sin tocar la jumplist, como en vim.
    fn lands(&self, motion: Motion, buffer: &Buffer, selections: &Selections) -> bool {
        match motion {
            Motion::Matching => match super::combined(self.operator_count, self.count) {
                Some(percent) => percent <= 100,
                None => matching::find(buffer, selections.primary().head).is_some(),
            },
            _ => true,
        }
    }
}
