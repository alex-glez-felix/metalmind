use nota_buffer::{Bias, Buffer, ByteOffset, Edit, EditMap, Selection, Selections};

use crate::lines::first_non_blank;
use crate::register::Register;
use crate::{PasteSide, VimError};

/// `p`/`P`. El cursor queda donde lo deja vim: en el primer no blanco de la
/// primera línea pegada, o en el último carácter de un pegado de una línea.
pub(crate) fn paste(
    buffer: &mut Buffer,
    selections: &Selections,
    register: &Register,
    side: PasteSide,
    count: usize,
) -> Result<Selections, VimError> {
    let text = repeated(register, count);
    if text.is_empty() {
        return Ok(selections.clone());
    }
    let insertions: Vec<ByteOffset> = selections
        .iter()
        .map(|selection| insertion_point(buffer, selection.head, register.linewise, side))
        .collect();
    let piece = match (register.linewise, side) {
        (true, PasteSide::After) => format!("\n{text}"),
        (true, PasteSide::Before) => format!("{text}\n"),
        (false, _) => text,
    };
    buffer.begin_transaction(selections.clone())?;
    let map = buffer.apply(
        insertions
            .iter()
            .map(|&at| Edit::insert(at, piece.clone()))
            .collect(),
    )?;
    // Una inserción por selección: nunca queda vacío.
    let after = Selections::new(
        insertions
            .iter()
            .map(|&at| Selection::cursor(cursor_after(buffer, &map, at, register, side))),
    )
    .unwrap_or_else(|| selections.clone());
    buffer.commit_transaction(after.clone())?;
    Ok(after)
}

fn repeated(register: &Register, count: usize) -> String {
    let separator = if register.linewise { "\n" } else { "" };
    vec![register.text.as_str(); count].join(separator)
}

fn insertion_point(
    buffer: &Buffer,
    head: ByteOffset,
    linewise: bool,
    side: PasteSide,
) -> ByteOffset {
    let line = buffer.line_of(head);
    match (linewise, side) {
        (true, PasteSide::After) => buffer.line_end(line),
        (true, PasteSide::Before) => buffer.line_start(line),
        (false, PasteSide::After) if head < buffer.line_end(line) => buffer.next_boundary(head),
        (false, _) => head,
    }
}

fn cursor_after(
    buffer: &Buffer,
    map: &EditMap,
    at: ByteOffset,
    register: &Register,
    side: PasteSide,
) -> ByteOffset {
    let start = map.map(at, Bias::Before);
    if register.linewise {
        let first_pasted = match side {
            PasteSide::After => buffer.line_of(start) + 1,
            PasteSide::Before => buffer.line_of(start),
        };
        return first_non_blank(buffer, first_pasted);
    }
    if register.text.contains('\n') {
        start
    } else {
        buffer.prev_boundary(map.map(at, Bias::After))
    }
}
