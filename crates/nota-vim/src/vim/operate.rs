//! Operadores, pegados y undo: lo que lee y escribe registros o recorre el
//! historial.

use nota_buffer::{Buffer, PerSelection, Selections};

use super::Vim;
use crate::dot::{Change, Dot, MarkJump, Subject};
use crate::insert;
use crate::object::Object;
use crate::operator::{Operand, Outcome};
use crate::register::{Clipboard, Register, RegisterName, Shape, Write};
use crate::search::Query;
use crate::travel::Travel;
use crate::{InsertAt, Operator, PasteSide, Viewport, VimError, paste};

/// Un operador ya tecleado, con lo que `.` necesita para repetirlo.
pub(super) struct Operation {
    pub operator: Operator,
    pub subject: Subject,
    pub count: Option<usize>,
    pub register: Option<RegisterName>,
}

impl Vim {
    pub(super) fn operate(
        &mut self,
        operator: Operator,
        subject: Subject,
        buffer: &mut Buffer,
        selections: &mut Selections,
        clipboard: &mut dyn Clipboard,
    ) -> Result<(), VimError> {
        let object = self.object_of(&subject, buffer);
        self.operate_on(operator, subject, object, buffer, selections, clipboard)
    }

    /// Sobre qué actúa un operador; `None` si no hay `;` o `n` que repetir o
    /// la mark no existe.
    pub(super) fn object_of(&self, subject: &Subject, buffer: &Buffer) -> Option<Object> {
        match *subject {
            Subject::Object(ref object) => Some(object.clone()),
            Subject::RepeatFind { reversed } => self
                .last_find
                .map(|find| Object::Find(find.repeat(reversed))),
            Subject::SearchNext { reversed } => self.search.query().map(|query| {
                Object::Search(Query {
                    backward: query.backward != reversed,
                    ..query
                })
            }),
            Subject::Mark(MarkJump { name, exact }) => {
                let offset = self.marks.get(buffer, name)?;
                Some(Object::Mark { offset, exact })
            }
        }
    }

    /// `object` es `subject` ya resuelto; `None` si no se pudo.
    pub(super) fn operate_on(
        &mut self,
        operator: Operator,
        subject: Subject,
        object: Option<Object>,
        buffer: &mut Buffer,
        selections: &mut Selections,
        clipboard: &mut dyn Clipboard,
    ) -> Result<(), VimError> {
        let count = self.take_count();
        let register = self.selected.take();
        self.reset();
        let Some(object) = object else {
            self.fail();
            return Ok(());
        };
        let operands = selections.map_to(|selection| Operand {
            selection: *selection,
            target: object.target(buffer, selection, operator, count, &self.viewport),
        });
        let operation = Operation {
            operator,
            subject,
            count,
            register,
        };
        self.finish_operator(operation, &operands, buffer, selections, clipboard)
    }

    /// Aplica el operador sobre lo ya resuelto, guarda lo copiado o borrado
    /// en los registros y graba el cambio para `.`.
    pub(super) fn finish_operator(
        &mut self,
        operation: Operation,
        operands: &PerSelection<Operand>,
        buffer: &mut Buffer,
        selections: &mut Selections,
        clipboard: &mut dyn Clipboard,
    ) -> Result<(), VimError> {
        let Operation {
            operator,
            subject,
            count,
            register,
        } = operation;
        let always_numbered = subject.always_numbered();
        // Como en vim, `".dw` no borra nada: no hay dónde guardar lo borrado.
        if register.is_some_and(|name| !name.is_writable()) {
            self.fail();
            return Ok(());
        }
        let outcome = operator.apply(buffer, operands, self.viewport.columns())?;
        let (applied, written, spans_lines) = match outcome {
            Outcome::Missed(moved) => {
                *selections = moved;
                self.fail();
                return Ok(());
            }
            Outcome::Applied {
                selections,
                register,
                spans_lines,
            } => (selections, register, spans_lines),
        };
        *selections = applied;
        if let Some(written) = written {
            // Solo `y`, `d` y `c` copian lo que abarcan.
            let write = match operator {
                Operator::Yank => Write::Yank,
                _ => Write::Delete {
                    spans_lines,
                    always_numbered,
                },
            };
            self.registers.write(register, written, write, clipboard);
        }
        let dot = Dot::new(Change::Operate(operator, subject), count, register);
        match operator {
            Operator::Change => self.start_insert(dot),
            Operator::Yank => {}
            _ => self.last_change = Some(dot),
        }
        Ok(())
    }

    pub(super) fn paste(
        &mut self,
        side: PasteSide,
        buffer: &mut Buffer,
        selections: &mut Selections,
        clipboard: &mut dyn Clipboard,
    ) -> Result<(), VimError> {
        let count = self.take_count();
        let register = self.selected.take();
        self.reset();
        if register == Some(RegisterName::LastInsert) {
            return self.reinsert(side, count, buffer, selections);
        }
        // Como en vim, `.` repite un `p` aunque el registro estuviera vacío.
        self.last_change = Some(Dot::new(Change::Paste(side), count, register));
        let times = count.unwrap_or(1);
        let Some(content) = self.read_register(register, clipboard) else {
            self.fail();
            return Ok(());
        };
        match content.shape {
            Shape::Block { width } => {
                self.put_block(&content, width, side, times, buffer, selections)?;
            }
            Shape::Chars | Shape::Lines => {
                *selections = paste::paste(buffer, selections, &content, side, times)?;
            }
        }
        Ok(())
    }

    /// `"/` es el último patrón, de una búsqueda o de `:s`.
    pub(super) fn read_register(
        &self,
        name: Option<RegisterName>,
        clipboard: &mut dyn Clipboard,
    ) -> Option<Register> {
        if name != Some(RegisterName::LastSearch) {
            return self.registers.read(name, clipboard);
        }
        let pattern = self.search.pattern.as_ref()?;
        Some(Register::chars(pattern.source()))
    }

    /// `".p` y `".P` vuelven a teclear lo último insertado, como `a` o `i`
    /// seguido del texto y `<Esc>` (`stuff_inserted` de vim): `.` repite
    /// esa pasada por Insert.
    fn reinsert(
        &mut self,
        side: PasteSide,
        count: Option<usize>,
        buffer: &mut Buffer,
        selections: &mut Selections,
    ) -> Result<(), VimError> {
        let Some(typed) = self.last_insert.clone() else {
            self.fail();
            return Ok(());
        };
        let at = match side {
            PasteSide::After => InsertAt::After,
            PasteSide::Before => InsertAt::Before,
        };
        // A diferencia de un `a` tecleado, `u` vuelve a donde se tecleó `".p`.
        buffer.begin_transaction(selections.clone())?;
        *selections = insert::insertion_points(buffer, selections, at);
        self.start_insert(Dot::new(Change::Insert(at), None, None));
        for _ in 0..count.unwrap_or(1) {
            for piece in &typed {
                self.type_in_insert(piece, buffer, selections)?;
            }
        }
        self.leave_insert(buffer, selections)
    }

    pub(super) fn travel(
        &mut self,
        travel: Travel,
        buffer: &mut Buffer,
        selections: &mut Selections,
    ) -> Result<(), VimError> {
        let times = self.count.take().unwrap_or(1);
        self.reset();
        match travel.run(buffer, times)? {
            Some(restored) => *selections = restored,
            None => self.fail(),
        }
        Ok(())
    }
}

/// Aplica el operador sin grabar nada para `.` ni tocar los registros, y
/// devuelve dónde quedan las selecciones.
pub(super) fn run(
    operator: Operator,
    object: &Object,
    buffer: &mut Buffer,
    selections: &Selections,
    viewport: &Viewport,
) -> Result<Selections, VimError> {
    let operands = selections.map_to(|selection| Operand {
        selection: *selection,
        target: object.target(buffer, selection, operator, None, viewport),
    });
    let outcome = operator.apply(buffer, &operands, viewport.columns())?;
    Ok(match outcome {
        Outcome::Missed(selections) | Outcome::Applied { selections, .. } => selections,
    })
}
