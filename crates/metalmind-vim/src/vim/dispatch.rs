//! De una acción con nombre a lo que hace en cada modo.

use metalmind_buffer::{Buffer, Selection, Selections};
use metalmind_syntax::Step;

use super::Vim;
use super::awaiting::Awaiting;
use crate::Target;
use crate::insert::Typed;
use crate::jumplist::Direction;
use crate::markdown::LazyStyles;
use crate::matching;
use crate::motion::Destination;
use crate::object::Object;
use crate::travel::Travel;
use crate::{Action, KeyOutcome, Mode, Motion, Operator, Prompt, VimError};

impl Vim {
    pub(super) fn run(
        &mut self,
        action: Action,
        t: &mut Target<'_>,
    ) -> Result<KeyOutcome, VimError> {
        self.awaiting = None;
        // Lo que lea el estilo de las líneas tiene que ver el texto de ahora.
        t.doc.sync(t.buffer);
        match self.mode {
            Mode::Insert => self.run_in_insert(action, t)?,
            Mode::CommandLine => {
                return self.run_in_command_line(action, t);
            }
            Mode::Normal | Mode::OperatorPending(_) => {
                return self.run_in_normal(action, t);
            }
            Mode::Visual(kind) => {
                return self.run_in_visual(action, kind, t);
            }
        }
        Ok(KeyOutcome::Executed(action))
    }

    /// Normal y operador pendiente.
    fn run_in_normal(
        &mut self,
        action: Action,
        t: &mut Target<'_>,
    ) -> Result<KeyOutcome, VimError> {
        match (action, self.mode) {
            (Action::Motion(motion), Mode::OperatorPending(operator)) => {
                if motion.is_jump() && self.lands(motion, t.buffer, t.selections) {
                    self.jumped(t)?;
                }
                let object = Object::Motion(motion).into();
                self.operate(operator, object, t)?;
            }
            (Action::Motion(motion), Mode::Normal) => self.move_by(motion, t)?,
            (Action::FindChar(search), Mode::Normal | Mode::OperatorPending(_)) => {
                self.awaiting = Some(Awaiting::Find(search));
            }
            (Action::RepeatFind { reversed }, Mode::Normal | Mode::OperatorPending(_)) => {
                self.repeat_find(reversed, t)?;
            }
            (Action::Search { backward }, Mode::Normal | Mode::OperatorPending(_)) => {
                self.open_search(backward);
            }
            (Action::SearchNext { reversed }, Mode::Normal | Mode::OperatorPending(_)) => {
                return self.search_next(reversed, t);
            }
            (
                Action::SearchWord {
                    backward,
                    whole_word,
                },
                Mode::Normal | Mode::OperatorPending(_),
            ) => return self.search_word(backward, whole_word, t),
            (Action::Scroll(scroll), Mode::Normal) => self.scroll(scroll, t),
            (Action::ScrollSideways(sideways), Mode::Normal) => self.scroll_sideways(sideways, t),
            (
                Action::Reposition {
                    place,
                    first_non_blank,
                },
                Mode::Normal,
            ) => self.reposition(place, first_non_blank, t)?,
            (Action::ClearSearchHighlight, Mode::Normal) => {
                self.reset();
                self.search.highlight = false;
            }
            (Action::TextObject(object), Mode::OperatorPending(operator)) => {
                let object = Object::TextObject(object).into();
                self.operate(operator, object, t)?;
            }
            (Action::CurrentLine, Mode::OperatorPending(operator)) => {
                let object = self.doubled_object(operator).into();
                self.operate(operator, object, t)?;
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
                let jumped = self.jump_in_list(Direction::Older, t)?;
                return Ok(jumped.unwrap_or(KeyOutcome::Executed(action)));
            }
            (Action::JumpNewer, Mode::Normal | Mode::OperatorPending(_)) => {
                let jumped = self.jump_in_list(Direction::Newer, t)?;
                return Ok(jumped.unwrap_or(KeyOutcome::Executed(action)));
            }
            (Action::DeleteRight, Mode::Normal) => {
                let object = Object::Motion(Motion::Right).into();
                self.operate(Operator::Delete, object, t)?;
            }
            (Action::Visual(kind), Mode::Normal) => self.enter_visual(kind, t),
            (Action::RestoreVisual, Mode::Normal) => self.restore_visual(t),
            (Action::Insert(at), Mode::Normal) => {
                self.enter_insert(at, true, t)?;
            }
            (Action::Paste(side), Mode::Normal) => {
                self.paste(side, t)?;
            }
            (Action::RepeatChange, Mode::Normal) => {
                self.repeat_change(t)?;
            }
            (Action::Undo, Mode::Normal) => self.travel(Travel::Undo, t)?,
            (Action::Redo, Mode::Normal) => self.travel(Travel::Redo, t)?,
            (Action::Earlier, Mode::Normal) => self.travel(Travel::Earlier, t)?,
            (Action::Later, Mode::Normal) => self.travel(Travel::Later, t)?,
            (Action::CommandLine, Mode::Normal) => self.open_command_line(Prompt::Command),
            (Action::ToggleCheckbox, Mode::Normal) => self.toggle_checkbox(t)?,
            (Action::FormatTable, Mode::Normal) => self.format_table(t)?,
            _ => self.reset(),
        }
        Ok(KeyOutcome::Executed(action))
    }

    fn run_in_insert(&mut self, action: Action, t: &mut Target<'_>) -> Result<(), VimError> {
        match action {
            Action::NormalMode => self.finish_insert(t)?,
            Action::Interrupt => self.leave_insert(t)?,
            Action::Backspace => self.type_in_insert(&Typed::Backspace, t)?,
            Action::Newline => {
                self.type_in_insert(&Typed::Newline { list: true }, t)?;
            }
            Action::NextTableCell => self.type_in_insert(&self.cell_or_tab(Step::Next), t)?,
            Action::PreviousTableCell => {
                self.type_in_insert(&self.cell_or_tab(Step::Previous), t)?;
            }
            _ => self.reset(),
        }
        Ok(())
    }

    /// Un Insert en bloque copia a las demás líneas lo escrito en la
    /// primera: ahí `tab` es un tab, como en vim, y no reescribe la tabla.
    fn cell_or_tab(&self, step: Step) -> Typed {
        if self.block_insert.is_some() {
            Typed::Text("\t".to_owned())
        } else {
            Typed::Cell(step)
        }
    }

    fn run_in_command_line(
        &mut self,
        action: Action,
        t: &mut Target<'_>,
    ) -> Result<KeyOutcome, VimError> {
        match action {
            Action::NormalMode | Action::Interrupt => self.close_command_line(),
            Action::Backspace => self.command_line_backspace(),
            Action::HistoryPrevious => self.recall(true),
            Action::HistoryNext => self.recall(false),
            Action::SubmitCommand => {
                return self.submit_command_line(t);
            }
            _ => self.reset(),
        }
        Ok(KeyOutcome::Executed(action))
    }

    /// Sobre qué actúa un operador doblado (`dd`, `>>`): la línea o, en
    /// `>>` y `<<` con `indent_list_children`, el ítem con sus hijos.
    fn doubled_object(&self, operator: Operator) -> Object {
        if operator.shift().is_some() && self.markdown.indent_list_children {
            Object::ListItems
        } else {
            Object::CurrentLine
        }
    }

    /// Una motion en Normal o, extendiendo la selección, en Visual. Como en
    /// vim, falla si falla en todos los cursores, y `h` y `l` también si no
    /// se mueven.
    pub(super) fn move_by(&mut self, motion: Motion, t: &mut Target<'_>) -> Result<(), VimError> {
        if let Motion::Window(place) = motion {
            return self.move_to_window_line(place, t);
        }
        let count = self.take_count();
        self.reset();
        let purpose = self.motion_purpose();
        let visual = self.in_visual();
        let styles = LazyStyles::new(t.doc.styles_mut());
        let moved = move_cursors(t.selections, visual, |s| {
            let destination = motion.resolve(t.buffer, s, count, purpose, t.pane, &styles)?;
            let stuck = motion.fails_without_moving() && destination.offset == s.head;
            (!stuck).then_some(destination)
        });
        match moved {
            Some(moved) => {
                if motion.is_jump() {
                    self.jumped(t)?;
                }
                *t.selections = moved;
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
