//! Las ediciones de varios bloques, aplicadas juntas en un paso de undo.

use metalmind_buffer::{Bias, Buffer, BufferError, ByteOffset, Edit, EditMap};

/// Las ediciones de un bloque y dónde queda el cursor, en el texto de antes:
/// `shift` bytes tras donde quede `at` al aplicarlas.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Plan {
    pub edits: Vec<Edit>,
    pub at: ByteOffset,
    pub shift: usize,
}

/// Aplica las ediciones de todos los planes, sin las que pisen a otro, y
/// devuelve dónde queda el cursor de cada uno.
pub(crate) fn apply<'a>(
    buffer: &mut Buffer,
    plans: impl IntoIterator<Item = &'a Plan> + Clone,
) -> Result<(Vec<ByteOffset>, EditMap), BufferError> {
    let edits = plans
        .clone()
        .into_iter()
        .flat_map(|plan| plan.edits.iter().cloned());
    let map = buffer.apply(without_overlaps(edits))?;
    let cursors = plans
        .into_iter()
        .map(|plan| map.map(plan.at, Bias::Before) + plan.shift)
        .collect();
    Ok((cursors, map))
}

/// Dos bloques que se tocan editan lo mismo; vale el primero.
pub(crate) fn without_overlaps(edits: impl IntoIterator<Item = Edit>) -> Vec<Edit> {
    let mut edits: Vec<Edit> = edits.into_iter().collect();
    edits.sort_by_key(|edit| edit.range.start);
    let mut kept: Vec<Edit> = Vec::with_capacity(edits.len());
    for edit in edits {
        if kept
            .last()
            .is_none_or(|last| edit.range.start >= last.range.end)
        {
            kept.push(edit);
        }
    }
    kept
}
