//! `I`, `A` y `c` sobre un bloque: se escribe en la primera línea de cada
//! uno y, al salir de Insert con `<Esc>`, lo escrito se copia a las demás.

use metalmind_buffer::{Bias, ByteOffset, PerSelection, Selection, Selections};

use super::{Vim, at_corners, corners, primary_size};
use crate::Target;
use crate::VimError;
use crate::block::{self, Block, BlockEdit, Insertion, Pending};
use crate::dot::{BlockChange, Change, Dot};
use crate::insert::left_of_cursors;
use crate::lines::clamp_normal;

impl Vim {
    /// `I` o `A`; con count, lo escrito se repite antes de copiarse a las
    /// demás líneas.
    pub(in crate::vim) fn insert_in_block(
        &mut self,
        append: bool,
        t: &mut Target<'_>,
    ) -> Result<(), VimError> {
        let count = self.take_count();
        let blocks = self.finish_block(t);
        self.start_block_insert(append, count, &blocks, t)
    }

    /// Al salir de Insert con `<Esc>`, lo escrito en la primera línea de cada
    /// bloque se repite en las demás. Devuelve dónde quedan los cursores si
    /// `I` o `A` los llevan a la esquina del bloque. Como vim, no repite nada
    /// si el cursor dejó la primera línea tras `I` o `A`.
    pub(in crate::vim) fn replicate_in_block(
        &mut self,
        t: &mut Target<'_>,
    ) -> Result<Option<Selections>, VimError> {
        let Some(BlockInsert { pending, .. }) = self.block_insert.take() else {
            return Ok(None);
        };
        if pending.len() != t.selections.iter().len() {
            return Ok(None);
        }
        let columns = t.pane.viewport.columns();
        let finishes: Vec<_> = pending
            .iter()
            .zip(t.selections.iter())
            .map(|(pending, selection)| {
                let stayed = t.buffer.line_of(selection.head) == pending.first_line();
                let moved_off = pending.insertion() != Insertion::Change && !stayed;
                (!moved_off)
                    .then(|| pending.finish(t.buffer, columns))
                    .flatten()
            })
            .collect();
        let edits = finishes
            .iter()
            .flatten()
            .flat_map(|finish| finish.edits.iter().cloned());
        let map = t.buffer.apply(block::without_overlaps(edits))?;
        let mut corners = finishes
            .iter()
            .map(|finish| finish.as_ref().and_then(|finish| finish.cursor));
        if corners.clone().all(|corner| corner.is_none()) {
            return Ok(None);
        }
        let left = left_of_cursors(t.buffer, t.selections);
        Ok(Some(left.map(|selection| {
            let cursor = match corners.next().flatten() {
                Some(corner) => clamp_normal(t.buffer, map.map(corner, Bias::Before)),
                None => map.map(selection.head, Bias::Before),
            };
            Selection::cursor(cursor)
        })))
    }

    pub(super) fn start_block_insert(
        &mut self,
        append: bool,
        count: Option<usize>,
        blocks: &PerSelection<Block>,
        t: &mut Target<'_>,
    ) -> Result<(), VimError> {
        let columns = t.pane.viewport.columns();
        let starts = blocks.map(|block| Pending::insert(t.buffer, *block, append, columns));
        // Como vim, `u` vuelve a donde empezó a escribirse.
        t.buffer
            .begin_transaction(starts.to_selections(|start| Selection::cursor(start.at)))?;
        let paddings = starts.iter().filter_map(|start| start.padding.clone());
        let map = t.buffer.apply(block::without_overlaps(paddings))?;
        let starts = starts
            .iter()
            .map(|start| (map.map(start.at, Bias::After), start.pending));
        let change = Change::Block(BlockChange::Insert { append }, primary_size(blocks));
        self.enter_block_insert(starts, Dot::new(change, count, None), t);
        Ok(())
    }

    /// `c`: borra cada bloque y escribe donde empezaba en su primera línea.
    pub(super) fn change_blocks(
        &mut self,
        blocks: &PerSelection<Block>,
        dot: Dot,
        t: &mut Target<'_>,
    ) -> Result<(), VimError> {
        let columns = t.pane.viewport.columns();
        let corners: Vec<usize> = corners(blocks, t.buffer, columns)
            .into_iter()
            .zip(blocks.iter())
            .map(|(corner, block)| corner.get() - t.buffer.line_start(block.first).get())
            .collect();
        let plans: Vec<_> = blocks
            .iter()
            .map(|block| block::edit(t.buffer, block, BlockEdit::Delete, columns))
            .collect();
        t.buffer
            .begin_transaction(at_corners(blocks, t.buffer, t.pane.viewport.columns()))?;
        let (cursors, _) = block::apply(t.buffer, &plans)?;
        let starts: Vec<_> = blocks
            .iter()
            .zip(cursors.into_iter().zip(corners))
            .map(|(block, (cursor, corner))| {
                let cursor = clamp_normal(t.buffer, cursor);
                let start = Pending::change(t.buffer, *block, cursor, corner, columns);
                (start.at, start.pending)
            })
            .collect();
        self.enter_block_insert(starts, dot, t);
        Ok(())
    }

    /// Pasa a Insert con un cursor donde empieza cada bloque.
    fn enter_block_insert(
        &mut self,
        starts: impl IntoIterator<Item = (ByteOffset, Pending)>,
        dot: Dot,
        t: &mut Target<'_>,
    ) {
        let (cursors, pending): (Vec<_>, Vec<_>) = starts.into_iter().unzip();
        if let Some(start) = cursors.first().copied() {
            self.block_insert = Some(BlockInsert { pending, start });
        }
        if let Some(entered) = Selections::new(cursors.into_iter().map(Selection::cursor)) {
            *t.selections = entered;
        }
        self.start_insert(dot);
    }
}

/// Un Insert sobre un bloque: lo que se repite por selección y dónde empezó
/// a escribirse en la primera línea.
#[derive(Debug)]
pub(in crate::vim) struct BlockInsert {
    pending: Vec<Pending>,
    pub start: ByteOffset,
}
