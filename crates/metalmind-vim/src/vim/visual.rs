//! Visual por carácter (`v`) y por línea (`V`), y lo que comparten con el
//! de bloque (`ctrl-v`, en `block/`).

use metalmind_buffer::{Buffer, Goal, PerSelection, Selection, Selections};

use super::Vim;
use super::awaiting::Awaiting;
use super::block::put_block_over_chars;
use super::dispatch::move_cursors;
use super::operate::{Operation, run};
use crate::Target;
use crate::columns::Columns;
use crate::dot::{Change, Dot, Subject};
use crate::lines::clamp_normal;
use crate::markdown::LazyOutline;
use crate::motion::Purpose;
use crate::object::Object;
use crate::operator::{Operand, Outcome};
use crate::register::{Register, RegisterName, Shape, Write};
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
        t: &mut Target<'_>,
    ) -> Result<KeyOutcome, VimError> {
        let moves = matches!(
            action,
            Action::Motion(_) | Action::Scroll(_) | Action::Reposition { .. }
        );
        if std::mem::take(&mut self.stale_line_end) && moves {
            *t.selections = t.selections.map(|s| Selection {
                goal: Goal::None,
                ..*s
            });
        }
        match action {
            Action::Motion(motion) => self.move_by(motion, t)?,
            Action::Scroll(scroll) => self.scroll(scroll, t),
            Action::Reposition {
                place,
                first_non_blank,
            } => self.reposition(place, first_non_blank, t)?,
            Action::FindChar(search) => self.awaiting = Some(Awaiting::Find(search)),
            Action::RepeatFind { reversed } => {
                self.repeat_find(reversed, t)?;
            }
            Action::Search { backward } => self.open_search(backward),
            Action::SearchNext { reversed } => {
                return self.search_next(reversed, t);
            }
            Action::GoToMark { exact } => self.awaiting = Some(Awaiting::Mark { exact }),
            Action::TextObject(object) => self.select_object(object, kind, t),
            Action::SelectRegister => self.awaiting = Some(Awaiting::Register),
            Action::Operator(operator) if kind == VisualKind::Block => {
                self.operate_block(operator, t)?;
            }
            Action::Operator(operator) => {
                self.operate_visual(operator, kind, t)?;
            }
            Action::Paste(side) if kind == VisualKind::Block => {
                self.paste_over_block(side, t)?;
            }
            Action::Paste(side) => self.paste_visual(side, kind, t)?,
            Action::BlockInsert { append } if kind == VisualKind::Block => {
                self.insert_in_block(append, t)?;
            }
            Action::Replace => self.awaiting = Some(Awaiting::Replace),
            Action::ToggleCheckbox => self.toggle_checkbox_visual(kind, t)?,
            Action::Visual(new) => self.toggle_visual(new, kind, t),
            Action::OtherEnd => self.other_end(kind, t),
            Action::OtherCorner => self.other_corner(kind, t),
            Action::RestoreVisual => self.restore_visual(t),
            Action::NormalMode | Action::Interrupt => self.leave_visual(kind, t),
            Action::CommandLine => self.command_line_from_visual(kind, t),
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
    pub(super) fn enter_visual(&mut self, kind: VisualKind, t: &mut Target<'_>) {
        let count = self.count.take();
        self.reset();
        self.stale_line_end = false;
        let cursors = t
            .selections
            .map(|s| Selection::cursor(s.head).with_goal(s.goal));
        self.mode = Mode::Visual(kind);
        *t.selections = match (count, self.last_visual_size) {
            (Some(times), Some(Reselect::Range(size))) => {
                self.mode = Mode::Visual(size.kind());
                let columns = t.pane.viewport.columns();
                cursors.map(|s| size.select_from(t.buffer, s.head, times, columns))
            }
            (Some(times), Some(Reselect::Block(size))) => {
                self.mode = Mode::Visual(VisualKind::Block);
                let columns = t.pane.viewport.columns();
                cursors
                    .map(|s| super::block::reselect_block(size, times, t.buffer, s.head, columns))
            }
            (Some(times), None) if times > 1 => {
                let motion = match kind {
                    VisualKind::Char | VisualKind::Block => Motion::Right,
                    VisualKind::Line => Motion::Down,
                };
                let outline = LazyOutline::new(t.doc.styles_mut());
                move_cursors(&cursors, true, |s| {
                    let times = Some(times - 1);
                    motion.resolve(
                        t.buffer,
                        s,
                        times,
                        Purpose::Select,
                        &t.pane.viewport,
                        &outline,
                    )
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
        t: &mut Target<'_>,
    ) {
        if kind == current {
            self.leave_visual(current, t);
        } else {
            self.reset();
            self.mode = Mode::Visual(kind);
        }
    }

    /// Sale a Normal con el cursor en el extremo que se movía, fuera del fin
    /// de línea; `gv` recuerda la selección tal como estaba.
    pub(super) fn leave_visual(&mut self, kind: VisualKind, t: &mut Target<'_>) {
        self.reset();
        self.last_visual = Some(LastVisual::save(t.buffer, t.selections, kind));
        self.mode = Mode::Normal;
        *t.selections = t
            .selections
            .map(|s| Selection::cursor(clamp_normal(t.buffer, s.head)));
    }

    /// `:` en Visual sale de la selección y empieza la línea con `'<,'>`.
    fn command_line_from_visual(&mut self, kind: VisualKind, t: &mut Target<'_>) {
        self.leave_visual(kind, t);
        self.open_command_line(Prompt::Command);
        self.type_in_command_line("'<,'>");
    }

    /// En un bloque, un `$` sigue valiendo para operar, pero la próxima
    /// motion ya no lo usa.
    pub(super) fn other_end(&mut self, kind: VisualKind, t: &mut Target<'_>) {
        self.reset();
        let keeps_line_end = |s: &Selection| kind == VisualKind::Block && s.goal == Goal::LineEnd;
        self.stale_line_end = t.selections.iter().any(keeps_line_end);
        *t.selections = t.selections.map(|s| Selection {
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
    pub(super) fn restore_visual(&mut self, t: &mut Target<'_>) {
        self.reset();
        let Some(marks) = self.last_visual.clone() else {
            return;
        };
        let Some(restored) = marks.restore(t.buffer) else {
            return;
        };
        if let Mode::Visual(kind) = self.mode {
            self.last_visual = Some(LastVisual::save(t.buffer, t.selections, kind));
        }
        self.mode = Mode::Visual(marks.kind());
        *t.selections = restored;
    }

    /// Con varias selecciones, la primera decide el modo.
    pub(super) fn select_object(
        &mut self,
        object: TextObject,
        kind: VisualKind,
        t: &mut Target<'_>,
    ) {
        let count = self.count.take().unwrap_or(1);
        self.reset();
        let outline = LazyOutline::new(t.doc.styles_mut());
        let selected = t
            .selections
            .map_to(|s| object.select(t.buffer, s, kind, count, &outline));
        let kind = selected.iter().next().map_or(kind, |&(_, kind)| kind);
        *t.selections = selected.to_selections(|&(selection, _)| selection);
        self.mode = Mode::Visual(kind);
    }

    /// Como en vim, el count solo cuenta en `>` y `<`, y `.` repite el
    /// operador sobre la misma cantidad de texto desde el cursor, con el
    /// mismo registro y el mismo count.
    pub(super) fn operate_visual(
        &mut self,
        operator: Operator,
        kind: VisualKind,
        t: &mut Target<'_>,
    ) -> Result<(), VimError> {
        let count = self.take_count();
        let register = self.selected.take();
        let size = self.finish_visual(kind, t);
        let operands = visual_operands(
            Area::Selection(kind),
            t.buffer,
            t.selections,
            t.pane.viewport.columns(),
        );
        let operation = Operation {
            operator,
            subject: Subject::Object(Object::Visual(Area::Sized(size))),
            count,
            register,
        };
        self.finish_operator(operation, &operands, t)
    }

    /// `.` de un operador en Visual: como vim, vuelve a seleccionar lo mismo
    /// desde el cursor, con la columna buscada que eso deja, y opera ahí.
    pub(super) fn redo_visual(
        &mut self,
        operator: Operator,
        size: VisualSize,
        t: &mut Target<'_>,
    ) -> Result<(), VimError> {
        let count = self.take_count();
        let register = self.selected.take();
        self.reset();
        let columns = t.pane.viewport.columns();
        // Un operando por cursor aunque las selecciones rehechas se solapen:
        // cada cursor guarda su trozo en el registro.
        let operands = t.selections.map_to(|cursor| {
            let redone = size.redo_from(t.buffer, cursor, columns);
            let area = Area::Selection(size.kind());
            Operand {
                selection: redone,
                target: Ok(area.target(t.buffer, &redone, columns)),
            }
        });
        let operation = Operation {
            operator,
            subject: Subject::Object(Object::Visual(Area::Sized(size))),
            count,
            register,
        };
        self.finish_operator(operation, &operands, t)
    }

    /// `p` y `P`: lo pegado reemplaza la selección. Como en vim, con `p` lo
    /// borrado va a los registros como un `d`, y con `P` a ninguno; `.`
    /// repite ese borrado (`"_d` tras `P`), que es lo que vim guarda.
    pub(super) fn paste_visual(
        &mut self,
        side: PasteSide,
        kind: VisualKind,
        t: &mut Target<'_>,
    ) -> Result<(), VimError> {
        let times = self.take_count().unwrap_or(1);
        let register = self.selected.take();
        let extent = self.finish_visual(kind, t);
        let content = self.read_register(register, t.clipboard.as_deref_mut());
        let targets = t.selections.map_to(|s| visual::target(t.buffer, s, kind));
        let deleted =
            Register::from_regions(t.buffer, targets.iter().map(|extent| &extent.region))?;
        let spans_lines = targets
            .iter()
            .any(|extent| extent.region.spans_lines(t.buffer));
        *t.selections = match &content {
            Some(
                content @ Register {
                    shape: Shape::Block { width },
                    ..
                },
            ) if kind == VisualKind::Char => {
                put_block_over_chars(&targets, content, *width, times, t)?
            }
            Some(content) => paste::replace(t.buffer, &targets, content, times)?,
            None => run(
                Operator::Delete,
                &Object::Visual(Area::Selection(kind)),
                t.buffer,
                t.selections,
                &t.pane.viewport,
                self.indent,
                &LazyOutline::new(t.doc.styles_mut()),
            )?,
        };
        let repeated_register = match side {
            PasteSide::After => {
                let write = Write::Delete {
                    spans_lines,
                    always_numbered: false,
                };
                self.registers
                    .write(None, deleted, write, t.clipboard.as_deref_mut());
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
    pub(super) fn replace_char(&mut self, c: char, t: &mut Target<'_>) -> Result<(), VimError> {
        let Mode::Visual(kind) = self.mode else {
            self.reset();
            return Ok(());
        };
        if kind == VisualKind::Block {
            return self.replace_in_block(c, t);
        }
        let size = self.finish_visual(kind, t);
        let area = Area::Selection(kind);
        transform(
            Transform::Replace(c),
            area,
            t.buffer,
            t.selections,
            t.pane.viewport.columns(),
        )?;
        self.last_change = Some(Dot::new(Change::Replace(c, size), None, None));
        Ok(())
    }

    pub(super) fn replace_sized(
        &mut self,
        c: char,
        size: VisualSize,
        t: &mut Target<'_>,
    ) -> Result<(), VimError> {
        self.reset();
        let area = Area::Sized(size);
        transform(
            Transform::Replace(c),
            area,
            t.buffer,
            t.selections,
            t.pane.viewport.columns(),
        )?;
        self.last_change = Some(Dot::new(Change::Replace(c, size), None, None));
        Ok(())
    }

    /// Antes de operar: sale a Normal, `gv` recuerda la selección, y `.` y
    /// `[count]v` su tamaño.
    pub(super) fn finish_visual(&mut self, kind: VisualKind, t: &Target<'_>) -> VisualSize {
        self.reset();
        self.mode = Mode::Normal;
        self.last_visual = Some(LastVisual::save(t.buffer, t.selections, kind));
        let columns = t.pane.viewport.columns();
        let size = VisualSize::of(t.buffer, t.selections.primary(), kind, columns);
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
