use nota_buffer::{Bias, Buffer, ByteOffset, Edit, Selection, Selections};

use crate::lines::first_non_blank;
use crate::{InsertAt, VimError};

/// Abre la transacción que dura todo el modo Insert, para que `u` la deshaga
/// entera.
pub(crate) fn enter(
    buffer: &mut Buffer,
    selections: &Selections,
    at: InsertAt,
) -> Result<Selections, VimError> {
    let point = |selection: &Selection| {
        let head = selection.head;
        let line = buffer.line_of(head);
        let offset = match at {
            InsertAt::After if head < buffer.line_end(line) => buffer.next_boundary(head),
            InsertAt::Before | InsertAt::After => head,
            InsertAt::FirstNonBlank => first_non_blank(buffer, line),
            InsertAt::LineEnd | InsertAt::LineBelow => buffer.line_end(line),
            InsertAt::LineAbove => buffer.line_start(line),
        };
        Selection::cursor(offset)
    };
    let points = selections.map(point);
    match at {
        InsertAt::LineBelow => open_line(buffer, selections, &points, Bias::After),
        InsertAt::LineAbove => open_line(buffer, selections, &points, Bias::Before),
        _ => {
            buffer.begin_transaction(points.clone())?;
            Ok(points)
        }
    }
}

/// `o`/`O`: el undo vuelve al cursor original, no a la línea nueva.
fn open_line(
    buffer: &mut Buffer,
    original: &Selections,
    points: &Selections,
    bias: Bias,
) -> Result<Selections, VimError> {
    buffer.begin_transaction(original.clone())?;
    let map = buffer.apply(
        points
            .iter()
            .map(|point| Edit::insert(point.head, "\n"))
            .collect(),
    )?;
    Ok(points.map(|point| Selection::cursor(map.map(point.head, bias))))
}

pub(crate) fn type_text(
    buffer: &mut Buffer,
    selections: &Selections,
    text: &str,
) -> Result<Selections, VimError> {
    let map = buffer.apply(
        selections
            .iter()
            .map(|selection| Edit::insert(selection.head, text))
            .collect(),
    )?;
    Ok(selections.map(|selection| Selection::cursor(map.map(selection.head, Bias::After))))
}

/// Con el `backspace=indent,eol,start` de nvim, borra lo que haya antes,
/// incluido el salto de línea.
pub(crate) fn backspace(
    buffer: &mut Buffer,
    selections: &Selections,
) -> Result<Selections, VimError> {
    let map = buffer.apply(
        selections
            .iter()
            .filter(|selection| selection.head > ByteOffset::ZERO)
            .map(|selection| Edit::delete(buffer.prev_boundary(selection.head)..selection.head))
            .collect(),
    )?;
    Ok(selections.map(|selection| Selection::cursor(map.map(selection.head, Bias::Before))))
}

/// Cierra la transacción y deja el cursor sobre el último carácter escrito.
pub(crate) fn leave(buffer: &mut Buffer, selections: &Selections) -> Result<Selections, VimError> {
    let after = selections.map(|selection| {
        let line_start = buffer.line_start(buffer.line_of(selection.head));
        if selection.head > line_start {
            Selection::cursor(buffer.prev_boundary(selection.head))
        } else {
            *selection
        }
    });
    buffer.commit_transaction(after.clone())?;
    Ok(after)
}
