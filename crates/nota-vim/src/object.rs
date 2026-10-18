use nota_buffer::{Buffer, ByteOffset, Selection};

use crate::find::Find;
use crate::mark;
use crate::search::{Query, WordSearch};
use crate::span::{Missed, Region, Span, SpanKind, Target};
use crate::{Motion, Operator, TextObject};

/// Sobre qué actúa un operador.
#[derive(Clone, Debug)]
pub(crate) enum Object {
    Motion(Motion),
    Find(Find),
    /// `d/foo<CR>`, `dn`: hasta donde empieza la coincidencia.
    Search(Query),
    /// `d*`, `d#`: como `Search`, con la palabra bajo el cursor.
    SearchWord(WordSearch),
    TextObject(TextObject),
    CurrentLine,
    /// `'x` o, si `exact`, `` `x ``, con la mark ya resuelta.
    Mark {
        offset: ByteOffset,
        exact: bool,
    },
}

impl Object {
    /// Lo que abarca `operator` desde `selection`, o dónde queda el cursor si
    /// falla ahí.
    pub(crate) fn target(
        &self,
        buffer: &Buffer,
        selection: &Selection,
        operator: Operator,
        count: Option<usize>,
    ) -> Result<Target, Missed> {
        let span = match *self {
            Self::Motion(motion) => motion
                .resolve(buffer, selection, count, Some(operator))
                .ok_or(Missed::Stay)?
                .span_from(selection.head),
            Self::Find(find) => find
                .resolve(buffer, selection.head, count.unwrap_or(1))
                .ok_or(Missed::Stay)?
                .span_from(selection.head),
            Self::Search(ref query) => query
                .find(buffer, selection.head, count.unwrap_or(1))
                .ok_or(Missed::Stay)?
                .destination()
                .span_from(selection.head),
            Self::SearchWord(search) => search
                .find(buffer, selection.head, count.unwrap_or(1))
                .ok_or(Missed::Stay)?
                .destination()
                .span_from(selection.head),
            Self::TextObject(object) => {
                object.resolve(buffer, selection.head, count.unwrap_or(1))?
            }
            Self::Mark { offset, exact } => {
                let kind = if exact {
                    SpanKind::Exclusive
                } else {
                    SpanKind::Linewise
                };
                let destination = mark::destination(buffer, offset, exact);
                Span::between(selection.head, destination, kind)
            }
            Self::CurrentLine => {
                return current_lines(buffer, selection, count.unwrap_or(1)).ok_or(Missed::Stay);
            }
        };
        Ok(span.into_target(buffer))
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
