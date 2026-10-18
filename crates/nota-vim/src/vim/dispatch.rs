//! De una acción con nombre a lo que hace en cada modo.

use nota_buffer::{Buffer, Selection, Selections};

use super::Vim;
use super::awaiting::Awaiting;
use crate::insert::Typed;
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
        match (action, self.mode) {
            (Action::Motion(motion), Mode::OperatorPending(operator)) => {
                let object = Object::Motion(motion).into();
                self.operate(operator, object, buffer, selections, clipboard)?;
            }
            (Action::Motion(motion), Mode::Normal) => {
                let count = self.take_count();
                self.reset();
                let moved = move_cursors(selections, |s| {
                    let destination = motion.resolve(buffer, s, count, None)?;
                    let stuck = motion.fails_without_moving() && destination.offset == s.head;
                    (!stuck).then_some(destination)
                });
                match moved {
                    Some(moved) => *selections = moved,
                    None => self.fail(),
                }
            }
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
            (Action::DeleteRight, Mode::Normal) => {
                let object = Object::Motion(Motion::Right).into();
                self.operate(Operator::Delete, object, buffer, selections, clipboard)?;
            }
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
            (Action::NormalMode, Mode::Insert) => {
                self.repeat_insert(buffer, selections)?;
                self.leave_insert(buffer, selections)?;
            }
            (Action::Interrupt, Mode::Insert) => self.leave_insert(buffer, selections)?,
            (Action::Backspace, Mode::Insert) => {
                self.type_in_insert(&Typed::Backspace, buffer, selections)?;
            }
            (Action::Newline, Mode::Insert) => {
                self.type_in_insert(&Typed::Text("\n".to_owned()), buffer, selections)?;
            }
            (Action::CommandLine, Mode::Normal) => self.open_command_line(Prompt::Command),
            (Action::NormalMode | Action::Interrupt, Mode::CommandLine) => {
                self.close_command_line();
            }
            (Action::Backspace, Mode::CommandLine) => self.command_line_backspace(),
            (Action::HistoryPrevious, Mode::CommandLine) => self.recall(true),
            (Action::HistoryNext, Mode::CommandLine) => self.recall(false),
            (Action::SubmitCommand, Mode::CommandLine) => {
                return self.submit_command_line(buffer, selections, clipboard);
            }
            _ => self.reset(),
        }
        Ok(KeyOutcome::Executed(action))
    }
}

/// Cada cursor va a donde lo lleve `resolve`; si falla, se queda. `None` si
/// falla en todos.
pub(super) fn move_cursors(
    selections: &Selections,
    resolve: impl Fn(&Selection) -> Option<Destination>,
) -> Option<Selections> {
    let moves = selections.map_to(|selection| (*selection, resolve(selection)));
    if moves.iter().all(|(_, destination)| destination.is_none()) {
        return None;
    }
    Some(moves.to_selections(|(selection, destination)| {
        destination.map_or(*selection, |destination| {
            Selection::cursor(destination.offset).with_goal(destination.goal)
        })
    }))
}
