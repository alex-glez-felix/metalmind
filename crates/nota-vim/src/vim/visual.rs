//! Visual por carácter (`v`) y por línea (`V`), y lo que comparten con el
//! de bloque (`ctrl-v`, en `block/`).

use nota_buffer::{Buffer, Goal, PerSelection, Selection, Selections};

use super::Vim;
use super::awaiting::Awaiting;
use super::dispatch::move_cursors;
use super::operate::{Operation, run};
use crate::columns::Columns;
use crate::dot::{Change, Dot, Subject};
use crate::lines::clamp_normal;
use crate::motion::Purpose;
use crate::object::Object;
use crate::operator::{Operand, Outcome};
use crate::register::{Clipboard, Register, RegisterName, Shape, Write};
use crate::transform::Transform;
use crate::visual::{self, Area, LastVisual, Reselect, VisualSize};
use crate::{
    Action, KeyOutcome, Mode, Motion, Operator, PasteSide, Prompt, TextObject, VimError,
    VisualKind, paste,
};

impl Vim {
    /// Lo que hace cada acción en Visual; las que no aplican solo cancelan
    /// lo pendiente.
    pub(super) fn run_in_visual(
        &mut self,
        action: Action,
        kind: VisualKind,
        buffer: &mut Buffer,
        selections: &mut Selections,
        clipboard: &mut dyn Clipboard,
    ) -> Result<KeyOutcome, VimError> {
        let moves = matches!(
            action,
            Action::Motion(_) | Action::Scroll(_) | Action::Reposition { .. }
        );
        if std::mem::take(&mut self.stale_line_end) && moves {
            *selections = selections.map(|s| Selection {
                goal: Goal::None,
                ..*s
            });
        }
        match action {
            Action::Motion(motion) => self.move_by(motion, buffer, selections)?,
            Action::Scroll(scroll) => self.scroll(scroll, buffer, selections),
            Action::Reposition {
                place,
                first_non_blank,
            } => self.reposition(place, first_non_blank, buffer, selections)?,
            Action::FindChar(search) => self.awaiting = Some(Awaiting::Find(search)),
            Action::RepeatFind { reversed } => {
                self.repeat_find(reversed, buffer, selections, clipboard)?;
            }
            Action::Search { backward } => self.open_search(backward),
            Action::SearchNext { reversed } => {
                return self.search_next(reversed, buffer, selections, clipboard);
            }
            Action::GoToMark { exact } => self.awaiting = Some(Awaiting::Mark { exact }),
            Action::TextObject(object) => self.select_object(object, kind, buffer, selections),
            Action::SelectRegister => self.awaiting = Some(Awaiting::Register),
            Action::Operator(operator) if kind == VisualKind::Block => {
                self.operate_block(operator, buffer, selections, clipboard)?;
            }
            Action::Operator(operator) => {
                self.operate_visual(operator, kind, buffer, selections, clipboard)?;
            }
            Action::Paste(side) if kind == VisualKind::Block => {
                self.paste_over_block(side, buffer, selections, clipboard)?;
            }
            Action::Paste(side) => self.paste_visual(side, kind, buffer, selections, clipboard)?,
            Action::BlockInsert { append } if kind == VisualKind::Block => {
                self.insert_in_block(append, buffer, selections)?;
            }
            Action::Replace => self.awaiting = Some(Awaiting::Replace),
            Action::Visual(new) => self.toggle_visual(new, kind, buffer, selections),
            Action::OtherEnd => self.other_end(kind, selections),
            Action::OtherCorner => self.other_corner(kind, buffer, selections),
            Action::RestoreVisual => self.restore_visual(buffer, selections),
            Action::NormalMode | Action::Interrupt => self.leave_visual(kind, buffer, selections),
            Action::CommandLine => self.command_line_from_visual(kind, buffer, selections),
            _ => self.reset(),
        }
        Ok(KeyOutcome::Executed(action))
    }

    pub(super) fn in_visual(&self) -> bool {
        matches!(self.mode, Mode::Visual(_))
    }

    pub(super) fn motion_purpose(&self) -> Purpose {
        if self.in_visual() {
            Purpose::Select
        } else {
            Purpose::Move
        }
    }

    /// `v` o `V` desde Normal. Con count toma la cantidad de texto de la
    /// última vez que se operó en Visual, multiplicada, y su modo; si nunca
    /// se operó, `count` caracteres o líneas.
    pub(super) fn enter_visual(
        &mut self,
        kind: VisualKind,
        buffer: &Buffer,
        selections: &mut Selections,
    ) {
        let count = self.count.take();
        self.reset();
        let cursors = selections.map(|s| Selection::cursor(s.head).with_goal(s.goal));
        self.mode = Mode::Visual(kind);
        *selections = match (count, self.last_visual_size) {
            (Some(times), Some(Reselect::Range(size))) => {
                self.mode = Mode::Visual(size.kind());
                let columns = self.viewport.columns();
                cursors.map(|s| size.select_from(buffer, s.head, times, columns))
            }
            (Some(times), Some(Reselect::Block(size))) => {
                self.mode = Mode::Visual(VisualKind::Block);
                cursors.map(|s| self.reselect_block(size, times, buffer, s.head))
            }
            (Some(times), None) if times > 1 => {
                let motion = match kind {
                    VisualKind::Char | VisualKind::Block => Motion::Right,
                    VisualKind::Line => Motion::Down,
                };
                move_cursors(&cursors, true, |s| {
                    motion.resolve(buffer, s, Some(times - 1), Purpose::Select, &self.viewport)
                })
                .unwrap_or(cursors)
            }
            _ => cursors,
        };
    }

    /// `v` o `V` en Visual: cambia de tipo o, si ya es de ese tipo, sale.
    pub(super) fn toggle_visual(
        &mut self,
        kind: VisualKind,
        current: VisualKind,
        buffer: &Buffer,
        selections: &mut Selections,
    ) {
        if kind == current {
            self.leave_visual(current, buffer, selections);
        } else {
            self.reset();
            self.mode = Mode::Visual(kind);
        }
    }

    /// Sale a Normal con el cursor en el extremo que se movía, fuera del fin
    /// de línea; `gv` recuerda la selección tal como estaba.
    pub(super) fn leave_visual(
        &mut self,
        kind: VisualKind,
        buffer: &Buffer,
        selections: &mut Selections,
    ) {
        self.reset();
        self.last_visual = Some(LastVisual::save(buffer, selections, kind));
        self.mode = Mode::Normal;
        *selections = selections.map(|s| Selection::cursor(clamp_normal(buffer, s.head)));
    }

    /// `:` en Visual sale de la selección y empieza la línea con `'<,'>`.
    fn command_line_from_visual(
        &mut self,
        kind: VisualKind,
        buffer: &Buffer,
        selections: &mut Selections,
    ) {
        self.leave_visual(kind, buffer, selections);
        self.open_command_line(Prompt::Command);
        self.type_in_command_line("'<,'>");
    }

    /// En un bloque, un `$` sigue valiendo para operar, pero la próxima
    /// motion ya no lo usa.
    pub(super) fn other_end(&mut self, kind: VisualKind, selections: &mut Selections) {
        self.reset();
        let keeps_line_end = |s: &Selection| kind == VisualKind::Block && s.goal == Goal::LineEnd;
        self.stale_line_end = selections.iter().any(keeps_line_end);
        *selections = selections.map(|s| Selection {
            anchor: s.head,
            head: s.anchor,
            goal: if keeps_line_end(s) {
                Goal::LineEnd
            } else {
                Goal::None
            },
        });
    }

    /// `gv`. Sin una selección anterior no hace nada.
    pub(super) fn restore_visual(&mut self, buffer: &Buffer, selections: &mut Selections) {
        self.reset();
        let Some(marks) = self.last_visual.clone() else {
            return;
        };
        let Some(restored) = marks.restore(buffer) else {
            return;
        };
        if let Mode::Visual(kind) = self.mode {
            self.last_visual = Some(LastVisual::save(buffer, selections, kind));
        }
        self.mode = Mode::Visual(marks.kind());
        *selections = restored;
    }

    /// Con varias selecciones, la primera decide el modo.
    pub(super) fn select_object(
        &mut self,
        object: TextObject,
        kind: VisualKind,
        buffer: &Buffer,
        selections: &mut Selections,
    ) {
        let count = self.count.take().unwrap_or(1);
        self.reset();
        let selected = selections.map_to(|s| object.select(buffer, s, kind, count));
        let kind = selected.iter().next().map_or(kind, |&(_, kind)| kind);
        *selections = selected.to_selections(|&(selection, _)| selection);
        self.mode = Mode::Visual(kind);
    }

    /// Como en vim, el count no cuenta y `.` repite el operador sobre la
    /// misma cantidad de texto desde el cursor, con el mismo registro.
    pub(super) fn operate_visual(
        &mut self,
        operator: Operator,
        kind: VisualKind,
        buffer: &mut Buffer,
        selections: &mut Selections,
        clipboard: &mut dyn Clipboard,
    ) -> Result<(), VimError> {
        let register = self.selected.take();
        let size = self.finish_visual(kind, buffer, selections);
        let operands = visual_operands(
            Area::Selection(kind),
            buffer,
            selections,
            self.viewport.columns(),
        );
        let operation = Operation {
            operator,
            subject: Subject::Object(Object::Visual(Area::Sized(size))),
            count: None,
            register,
        };
        self.finish_operator(operation, &operands, buffer, selections, clipboard)
    }

    /// `p` y `P`: lo pegado reemplaza la selección. Como en vim, con `p` lo
    /// borrado va a los registros como un `d`, y con `P` a ninguno; `.`
    /// repite ese borrado (`"_d` tras `P`), que es lo que vim guarda.
    pub(super) fn paste_visual(
        &mut self,
        side: PasteSide,
        kind: VisualKind,
        buffer: &mut Buffer,
        selections: &mut Selections,
        clipboard: &mut dyn Clipboard,
    ) -> Result<(), VimError> {
        let times = self.take_count().unwrap_or(1);
        let register = self.selected.take();
        let extent = self.finish_visual(kind, buffer, selections);
        let content = self.read_register(register, clipboard);
        let targets = selections.map_to(|s| visual::target(buffer, s, kind));
        let deleted = Register::from_regions(buffer, targets.iter().map(|t| &t.region))?;
        let spans_lines = targets.iter().any(|t| t.region.spans_lines(buffer));
        *selections = match &content {
            Some(
                content @ Register {
                    shape: Shape::Block { width },
                    ..
                },
            ) if kind == VisualKind::Char => {
                self.put_block_over_chars(&targets, content, *width, times, buffer)?
            }
            Some(content) => paste::replace(buffer, &targets, content, times)?,
            None => run(
                Operator::Delete,
                &Object::Visual(Area::Selection(kind)),
                buffer,
                selections,
                &self.viewport,
            )?,
        };
        let repeated_register = match side {
            PasteSide::After => {
                let write = Write::Delete {
                    spans_lines,
                    always_numbered: false,
                };
                self.registers.write(None, deleted, write, clipboard);
                None
            }
            PasteSide::Before => Some(RegisterName::BlackHole),
        };
        let repeated = Subject::Object(Object::Visual(Area::Sized(extent)));
        let change = Change::Operate(Operator::Delete, repeated);
        self.last_change = Some(Dot::new(change, None, repeated_register));
        Ok(())
    }

    /// Completa `r` en Visual.
    pub(super) fn replace_char(
        &mut self,
        c: char,
        buffer: &mut Buffer,
        selections: &mut Selections,
    ) -> Result<(), VimError> {
        let Mode::Visual(kind) = self.mode else {
            self.reset();
            return Ok(());
        };
        if kind == VisualKind::Block {
            return self.replace_in_block(c, buffer, selections);
        }
        let size = self.finish_visual(kind, buffer, selections);
        let area = Area::Selection(kind);
        transform(
            Transform::Replace(c),
            area,
            buffer,
            selections,
            self.viewport.columns(),
        )?;
        self.last_change = Some(Dot::new(Change::Replace(c, size), None, None));
        Ok(())
    }

    pub(super) fn replace_sized(
        &mut self,
        c: char,
        size: VisualSize,
        buffer: &mut Buffer,
        selections: &mut Selections,
    ) -> Result<(), VimError> {
        self.reset();
        let area = Area::Sized(size);
        transform(
            Transform::Replace(c),
            area,
            buffer,
            selections,
            self.viewport.columns(),
        )?;
        self.last_change = Some(Dot::new(Change::Replace(c, size), None, None));
        Ok(())
    }

    /// Antes de operar: sale a Normal, `gv` recuerda la selección, y `.` y
    /// `[count]v` su tamaño.
    fn finish_visual(
        &mut self,
        kind: VisualKind,
        buffer: &Buffer,
        selections: &Selections,
    ) -> VisualSize {
        self.reset();
        self.mode = Mode::Normal;
        self.last_visual = Some(LastVisual::save(buffer, selections, kind));
        let size = VisualSize::of(buffer, selections.primary(), kind, self.viewport.columns());
        self.last_visual_size = Some(Reselect::Range(size));
        size
    }
}

fn transform(
    transform: Transform,
    area: Area,
    buffer: &mut Buffer,
    selections: &mut Selections,
    columns: Columns,
) -> Result<(), VimError> {
    let operands = visual_operands(area, buffer, selections, columns);
    let (Outcome::Missed(after)
    | Outcome::Applied {
        selections: after, ..
    }) = transform.apply(buffer, &operands)?;
    *selections = after;
    Ok(())
}

fn visual_operands(
    area: Area,
    buffer: &Buffer,
    selections: &Selections,
    columns: Columns,
) -> PerSelection<Operand> {
    selections.map_to(|selection| Operand {
        selection: *selection,
        target: Ok(area.target(buffer, selection, columns)),
    })
}
