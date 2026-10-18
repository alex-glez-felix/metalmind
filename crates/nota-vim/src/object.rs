use nota_buffer::{Buffer, Selection};

use crate::find::Find;
use crate::span::{Region, Target};
use crate::{Motion, Operator, TextObject};

/// Sobre qué actúa un operador.
#[derive(Clone, Copy, Debug)]
pub(crate) enum Object {
    Motion(Motion),
    Find(Find),
    TextObject(TextObject),
    CurrentLine,
}

impl Object {
    /// Lo que abarca `operator` desde `selection`; `None` si falla ahí.
    pub(crate) fn target(
        self,
        buffer: &Buffer,
        selection: &Selection,
        operator: Operator,
        count: Option<usize>,
    ) -> Option<Target> {
        let span = match self {
            Self::Motion(motion) => motion
                .resolve(buffer, selection, count, Some(operator))?
                .span_from(selection.head),
            Self::Find(find) => find
                .resolve(buffer, selection.head, count.unwrap_or(1))?
                .span_from(selection.head),
            Self::TextObject(object) => {
                object.resolve(buffer, selection.head, count.unwrap_or(1))?
            }
            Self::CurrentLine => return current_lines(buffer, selection, count.unwrap_or(1)),
        };
        Some(span.into_target(buffer))
    }
}

/// `3dd` falla en la última línea, pero si hay menos de 3 toma las que haya.
fn current_lines(buffer: &Buffer, selection: &Selection, count: usize) -> Option<Target> {
    let first = buffer.line_of(selection.head);
    let last_line = buffer.last_line();
    if count > 1 && first >= last_line {
        return None;
    }
    Some(Target {
        region: Region::Lines {
            first,
            last: first.saturating_add(count - 1).min(last_line),
        },
        start: selection.head,
        adjusted: false,
    })
}
