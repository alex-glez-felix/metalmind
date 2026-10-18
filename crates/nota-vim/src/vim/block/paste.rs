//! `p` y `P` con bloques: sobre un bloque seleccionado, y de un registro por
//! bloque en Normal o sobre una selección por caracteres.

use nota_buffer::{Bias, Buffer, ByteOffset, Edit, PerSelection, Selection, Selections};

use super::{Vim, cursors_at, deletion, primary_size};
use crate::block::{self, Plan};
use crate::dot::{BlockChange, Change, Dot};
use crate::lines::clamp_normal;
use crate::operator::{content, union};
use crate::register::{Clipboard, Register, RegisterName, Shape};
use crate::span::Target;
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
        buffer: &mut Buffer,
        selections: &mut Selections,
        clipboard: &mut dyn Clipboard,
    ) -> Result<(), VimError> {
        let times = self.take_count().unwrap_or(1);
        let register = self.selected.take();
        let heads: Vec<ByteOffset> = selections.iter().map(|selection| selection.head).collect();
        let blocks = self.finish_block(buffer, selections);
        let content = self.read_register(register, clipboard);
        let deleted = self.yank_blocks(&blocks, buffer);
        let corners = self.corners(&blocks, buffer);
        let plans: Vec<_> = blocks
            .iter()
            .map(|block| block::delete(buffer, block, self.columns()))
            .collect();
        buffer.begin_transaction(self.at_corners(&blocks, buffer))?;
        let (cursors, map) = block::apply(buffer, &plans)?;
        let landings: Vec<Landing> = cursors
            .into_iter()
            .zip(corners.into_iter().zip(heads))
            .zip(blocks.iter())
            .map(|((cursor, (corner, head)), block)| {
                let cursor = clamp_normal(buffer, cursor);
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
                self.put_over_blocks(&put, &landings, buffer)?
            }
            None => cursors_at(
                buffer,
                landings.iter().map(|landing| landing.cursor).collect(),
            ),
        };
        if let Some(after) = after {
            *selections = after;
        }
        buffer.commit_transaction(selections.clone())?;
        let repeated_register = match side {
            PasteSide::After => {
                self.registers
                    .write(None, deleted, deletion(&blocks), clipboard);
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

    /// `p` en Normal con un registro por bloque.
    pub(in crate::vim) fn put_block(
        &mut self,
        content: &Register,
        width: usize,
        side: PasteSide,
        times: usize,
        buffer: &mut Buffer,
        selections: &mut Selections,
    ) -> Result<(), VimError> {
        let lines: Vec<&str> = content.text.split('\n').collect();
        let columns = self.columns();
        let plans: Vec<_> = selections
            .iter()
            .map(|selection| {
                block::put(buffer, selection.head, &lines, width, side, times, columns)
            })
            .collect();
        buffer.begin_transaction(selections.clone())?;
        let (cursors, _) = block::apply(buffer, &plans)?;
        if let Some(after) = cursors_at(buffer, cursors) {
            *selections = after;
        }
        buffer.commit_transaction(selections.clone())?;
        Ok(())
    }

    /// `p` en Visual por caracteres con un registro por bloque: borra lo
    /// seleccionado y pega el bloque donde quedó el cursor; tras el último
    /// carácter si el borrado llegó al fin de línea.
    pub(in crate::vim) fn put_block_over_chars(
        &self,
        targets: &PerSelection<Target>,
        register: &Register,
        width: usize,
        times: usize,
        buffer: &mut Buffer,
    ) -> Result<Selections, VimError> {
        let lines: Vec<&str> = register.text.split('\n').collect();
        let columns = self.columns();
        let ranges: Vec<_> = targets
            .iter()
            .map(|target| content(buffer, &target.region))
            .collect();
        let before = targets.to_selections(|target| Selection::cursor(target.start));
        buffer.begin_transaction(before.clone())?;
        let deletions = union(ranges.iter().cloned());
        let map = buffer.apply(deletions.into_iter().map(Edit::delete).collect())?;
        let plans: Vec<_> = ranges
            .iter()
            .map(|range| {
                let start = map.map(range.start, Bias::Before);
                let cursor = clamp_normal(buffer, start);
                let side = if cursor < start {
                    PasteSide::After
                } else {
                    PasteSide::Before
                };
                block::put(buffer, cursor, &lines, width, side, times, columns)
            })
            .collect();
        let (cursors, _) = block::apply(buffer, &plans)?;
        let after = cursors_at(buffer, cursors).unwrap_or(before);
        buffer.commit_transaction(after.clone())?;
        Ok(after)
    }

    /// Pega `put` donde quedó cada bloque borrado.
    fn put_over_blocks(
        &self,
        put: &Put<'_>,
        landings: &[Landing],
        buffer: &mut Buffer,
    ) -> Result<Option<Selections>, VimError> {
        let columns = self.columns();
        let content = put.content;
        let plans: Vec<Plan> = match content.shape {
            Shape::Block { width } => {
                let lines: Vec<&str> = content.text.split('\n').collect();
                landings
                    .iter()
                    .map(|landing| {
                        let (cursor, side) = (landing.cursor, landing.side);
                        block::put(buffer, cursor, &lines, width, side, put.times, columns)
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
                        buffer,
                        cursor,
                        last,
                        &content.text,
                        side,
                        put.times,
                        columns,
                    )
                })
                .collect(),
            Shape::Chars | Shape::Lines => return put_text(put, landings, buffer),
        };
        let (cursors, _) = block::apply(buffer, &plans)?;
        Ok(cursors_at(buffer, cursors))
    }
}

/// Lo que no se pega por bloque, como `p` normal desde el cursor; un
/// registro por líneas con `p` va debajo de la línea del extremo que se
/// movía (`PUT_LINE_FORWARD` de vim). Con varios bloques, el primero
/// decide de qué lado.
fn put_text(
    put: &Put<'_>,
    landings: &[Landing],
    buffer: &mut Buffer,
) -> Result<Option<Selections>, VimError> {
    let below_head = put.content.is_linewise() && put.side == PasteSide::After;
    let (targets, side): (Vec<_>, _) = if below_head {
        (
            landings.iter().map(|landing| landing.head).collect(),
            PasteSide::After,
        )
    } else {
        let side = landings
            .first()
            .map_or(PasteSide::Before, |landing| landing.side);
        (
            landings.iter().map(|landing| landing.cursor).collect(),
            side,
        )
    };
    let Some(at) = Selections::new(targets.into_iter().map(Selection::cursor)) else {
        return Ok(None);
    };
    paste::put(buffer, &at, put.content, side, put.times).map(Some)
}
