//! `p` y `P` con bloques: sobre un bloque seleccionado, y de un registro por
//! bloque en Normal o sobre una selección por caracteres.

use metalmind_buffer::{Bias, Buffer, ByteOffset, Edit, PerSelection, Selection, Selections};

use super::{Vim, at_corners, corners, cursors_at, deletion, primary_size, yank_blocks};
use crate::Target;
use crate::block::{self, Plan};
use crate::dot::{BlockChange, Change, Dot};
use crate::lines::clamp_normal;
use crate::operator::{content, union};
use crate::register::{Register, RegisterName, Shape};
use crate::span::Extent;
use crate::{Operator, PasteSide, VimError, paste};

/// Lo que se pega y cómo.
struct Put<'a> {
    content: &'a Register,
    side: PasteSide,
    times: usize,
}

/// Dónde quedó cada bloque tras borrarlo, para pegar ahí.
#[derive(Clone, Copy, Debug)]
struct Landing {
    cursor: ByteOffset,
    /// Como vim, si el borrado dejó el cursor antes de la esquina (el bloque
    /// llegaba al fin de línea), se pega tras él.
    side: PasteSide,
    /// El extremo que movía el cursor: `p` pega las líneas debajo del suyo.
    head: ByteOffset,
    last: usize,
}

impl Vim {
    /// `p` y `P` sobre un bloque, como `nv_put_opt` de vim: borra el bloque
    /// y pega donde quedó el cursor. Un registro de una sola línea por
    /// caracteres se pega en cada línea del bloque (`blockwise-put`); uno
    /// por líneas va debajo de la línea del cursor con `p` y encima con `P`.
    /// Con `p` lo borrado va a los registros como con `d`; con `P`, a
    /// ninguno, y `.` repite el borrado.
    pub(in crate::vim) fn paste_over_block(
        &mut self,
        side: PasteSide,
        t: &mut Target<'_>,
    ) -> Result<(), VimError> {
        let times = self.take_count().unwrap_or(1);
        let register = self.selected.take();
        let heads: Vec<ByteOffset> = t
            .selections
            .iter()
            .map(|selection| selection.head)
            .collect();
        let blocks = self.finish_block(t);
        let content = self.read_register(register, t.clipboard.as_deref_mut());
        let deleted = yank_blocks(&blocks, t.buffer, t.pane.viewport.columns());
        let block_corners = corners(&blocks, t.buffer, t.pane.viewport.columns());
        let plans: Vec<_> = blocks
            .iter()
            .map(|block| block::delete(t.buffer, block, t.pane.viewport.columns()))
            .collect();
        t.buffer
            .begin_transaction(at_corners(&blocks, t.buffer, t.pane.viewport.columns()))?;
        let (cursors, map) = block::apply(t.buffer, &plans)?;
        let landings: Vec<Landing> = cursors
            .into_iter()
            .zip(block_corners.into_iter().zip(heads))
            .zip(blocks.iter())
            .map(|((cursor, (corner, head)), block)| {
                let cursor = clamp_normal(t.buffer, cursor);
                let side = if cursor < map.map(corner, Bias::Before) {
                    PasteSide::After
                } else {
                    PasteSide::Before
                };
                Landing {
                    cursor,
                    side,
                    head: map.map(head, Bias::Before),
                    last: block.last,
                }
            })
            .collect();
        let after = match &content {
            Some(content) => {
                let put = Put {
                    content,
                    side,
                    times,
                };
                put_over_blocks(&put, &landings, t)?
            }
            None => cursors_at(
                t.buffer,
                landings.iter().map(|landing| landing.cursor).collect(),
            ),
        };
        if let Some(after) = after {
            *t.selections = after;
        }
        t.buffer.commit_transaction(t.selections.clone())?;
        let repeated_register = match side {
            PasteSide::After => {
                self.registers
                    .write(None, deleted, deletion(&blocks), t.clipboard.as_deref_mut());
                None
            }
            PasteSide::Before => Some(RegisterName::BlackHole),
        };
        let change = Change::Block(
            BlockChange::Operate(Operator::Delete),
            primary_size(&blocks),
        );
        self.last_change = Some(Dot::new(change, None, repeated_register));
        Ok(())
    }
}

/// Lo que no se pega por bloque, como `p` normal desde el cursor; un
/// registro por líneas con `p` va debajo de la línea del extremo que se
/// movía (`PUT_LINE_FORWARD` de vim). Si no, cada bloque pega del lado
/// que le dejó su borrado, como `nv_put_opt` con el cursor de ese bloque.
fn put_text(
    put: &Put<'_>,
    landings: &[Landing],
    buffer: &mut Buffer,
) -> Result<Option<Selections>, VimError> {
    let below_head = put.content.is_linewise() && put.side == PasteSide::After;
    let targets: Vec<(ByteOffset, PasteSide)> = landings
        .iter()
        .map(|landing| {
            if below_head {
                (landing.head, PasteSide::After)
            } else {
                (landing.cursor, landing.side)
            }
        })
        .collect();
    let Some(at) = Selections::new(targets.iter().map(|&(at, _)| Selection::cursor(at))) else {
        return Ok(None);
    };
    // Dos bloques que quedan en el mismo cursor se funden en una selección.
    let side_of = |selection: &Selection| {
        targets
            .iter()
            .find(|&&(at, _)| at == selection.head)
            .map_or(PasteSide::Before, |&(_, side)| side)
    };
    paste::put(buffer, &at, put.content, side_of, put.times).map(Some)
}

/// `p` en Normal con un registro por bloque.
pub(in crate::vim) fn put_block(
    content: &Register,
    width: usize,
    side: PasteSide,
    times: usize,
    t: &mut Target<'_>,
) -> Result<(), VimError> {
    let lines: Vec<&str> = content.text.split('\n').collect();
    let columns = t.pane.viewport.columns();
    let plans: Vec<_> = t
        .selections
        .iter()
        .map(|selection| {
            block::put(
                t.buffer,
                selection.head,
                &lines,
                width,
                side,
                times,
                columns,
            )
        })
        .collect();
    t.buffer.begin_transaction(t.selections.clone())?;
    let (cursors, _) = block::apply(t.buffer, &plans)?;
    if let Some(after) = cursors_at(t.buffer, cursors) {
        *t.selections = after;
    }
    t.buffer.commit_transaction(t.selections.clone())?;
    Ok(())
}

/// `p` en Visual por caracteres con un registro por bloque: borra lo
/// seleccionado y pega el bloque donde quedó el cursor; tras el último
/// carácter si el borrado llegó al fin de línea.
pub(in crate::vim) fn put_block_over_chars(
    targets: &PerSelection<Extent>,
    register: &Register,
    width: usize,
    times: usize,
    t: &mut Target<'_>,
) -> Result<Selections, VimError> {
    let lines: Vec<&str> = register.text.split('\n').collect();
    let columns = t.pane.viewport.columns();
    let ranges: Vec<_> = targets
        .iter()
        .map(|target| content(t.buffer, &target.region))
        .collect();
    let before = targets.to_selections(|target| Selection::cursor(target.start));
    t.buffer.begin_transaction(before.clone())?;
    let deletions = union(ranges.iter().cloned());
    let map = t
        .buffer
        .apply(deletions.into_iter().map(Edit::delete).collect())?;
    let plans: Vec<_> = ranges
        .iter()
        .map(|range| {
            let start = map.map(range.start, Bias::Before);
            let cursor = clamp_normal(t.buffer, start);
            let side = if cursor < start {
                PasteSide::After
            } else {
                PasteSide::Before
            };
            block::put(t.buffer, cursor, &lines, width, side, times, columns)
        })
        .collect();
    let (cursors, _) = block::apply(t.buffer, &plans)?;
    let after = cursors_at(t.buffer, cursors).unwrap_or(before);
    t.buffer.commit_transaction(after.clone())?;
    Ok(after)
}

/// Pega `put` donde quedó cada bloque borrado.
fn put_over_blocks(
    put: &Put<'_>,
    landings: &[Landing],
    t: &mut Target<'_>,
) -> Result<Option<Selections>, VimError> {
    let columns = t.pane.viewport.columns();
    let content = put.content;
    let plans: Vec<Plan> = match content.shape {
        Shape::Block { width } => {
            let lines: Vec<&str> = content.text.split('\n').collect();
            landings
                .iter()
                .map(|landing| {
                    let (cursor, side) = (landing.cursor, landing.side);
                    block::put(t.buffer, cursor, &lines, width, side, put.times, columns)
                })
                .collect()
        }
        Shape::Chars if !content.text.contains('\n') => landings
            .iter()
            .map(|landing| {
                let Landing {
                    cursor, side, last, ..
                } = *landing;
                block::put_in_each_line(
                    t.buffer,
                    cursor,
                    last,
                    &content.text,
                    side,
                    put.times,
                    columns,
                )
            })
            .collect(),
        Shape::Chars | Shape::Lines => return put_text(put, landings, t.buffer),
    };
    let (cursors, _) = block::apply(t.buffer, &plans)?;
    Ok(cursors_at(t.buffer, cursors))
}
