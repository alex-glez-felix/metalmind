//! Visual por bloque (`ctrl-v`): operadores, `r`, `O`, `.` y `[count]ctrl-v`;
//! `I`, `A` y `c` en `insert.rs`, y `p`/`P` en `paste.rs`.

mod insert;
mod paste;

pub(super) use paste::{put_block, put_block_over_chars};

use metalmind_buffer::{Buffer, ByteOffset, Goal, PerSelection, Selection, Selections};

use super::Vim;
use crate::Target;
use crate::block::{self, Block, BlockEdit, BlockSize};
use crate::columns::Columns;
use crate::dot::{BlockChange, Change, Dot};
use crate::indent::Shift;
use crate::lines::{at_byte_column, clamp_normal};
use crate::register::{Register, RegisterName, Shape, Write};
use crate::transform::Transform;
use crate::visual::{LastVisual, Reselect};
use crate::{Mode, Operator, VimError, VisualKind};

impl Vim {
    /// `d`, `c`, `y`, `~`, `u`, `U`, `>` y `<` sobre cada bloque. Como en
    /// vim, el count solo cuenta en `>` y `<`, que corren tantos niveles, y
    /// `.` repite sobre un bloque del mismo tamaño.
    pub(super) fn operate_block(
        &mut self,
        operator: Operator,
        t: &mut Target<'_>,
    ) -> Result<(), VimError> {
        let count = self.take_count();
        let register = self.selected.take();
        let blocks = self.finish_block(t);
        let operation = BlockOperation {
            operator,
            count,
            register,
        };
        self.block_operator(operation, &blocks, t)
    }

    /// Completa `r` en un bloque.
    pub(super) fn replace_in_block(&mut self, c: char, t: &mut Target<'_>) -> Result<(), VimError> {
        let blocks = self.finish_block(t);
        self.replace_blocks(c, &blocks, t)
    }

    /// `O`: en un bloque, el cursor va a la otra esquina de su línea y el
    /// otro extremo a la de la suya (`v_swap_corners` de vim). Si el cursor
    /// ya estaba en la esquina derecha, va a la izquierda.
    pub(super) fn other_corner(&mut self, kind: VisualKind, t: &mut Target<'_>) {
        if kind != VisualKind::Block {
            self.other_end(kind, t);
            return;
        }
        self.reset();
        let columns = t.pane.viewport.columns();
        *t.selections = t
            .selections
            .map(|selection| swap_corners(t.buffer, selection, columns));
    }

    /// `.` sobre un bloque de `size` desde cada cursor.
    pub(super) fn repeat_block(
        &mut self,
        change: BlockChange,
        size: BlockSize,
        t: &mut Target<'_>,
    ) -> Result<(), VimError> {
        let count = self.take_count();
        let register = self.selected.take();
        self.reset();
        let columns = t.pane.viewport.columns();
        let blocks = t
            .selections
            .map_to(|selection| Block::sized(t.buffer, selection.head, size, columns));
        match change {
            BlockChange::Operate(operator) => {
                let operation = BlockOperation {
                    operator,
                    count,
                    register,
                };
                self.block_operator(operation, &blocks, t)
            }
            BlockChange::Replace(c) => self.replace_blocks(c, &blocks, t),
            BlockChange::Insert { append } => self.start_block_insert(append, count, &blocks, t),
        }
    }

    /// Antes de operar: sale a Normal, `gv` recuerda la selección, y `.` y
    /// `[count]v` el tamaño del primer bloque.
    fn finish_block(&mut self, t: &Target<'_>) -> PerSelection<Block> {
        self.reset();
        self.mode = Mode::Normal;
        self.last_visual = Some(LastVisual::save(t.buffer, t.selections, VisualKind::Block));
        let columns = t.pane.viewport.columns();
        let blocks = t
            .selections
            .map_to(|selection| Block::of(t.buffer, selection, columns));
        self.last_visual_size = Some(Reselect::Block(primary_size(&blocks)));
        blocks
    }

    fn block_operator(
        &mut self,
        operation: BlockOperation,
        blocks: &PerSelection<Block>,
        t: &mut Target<'_>,
    ) -> Result<(), VimError> {
        let BlockOperation {
            operator, register, ..
        } = operation;
        let how = match operator {
            // Como vim, `".d` no opera, pero el cursor ya fue a la esquina.
            _ if register.is_some_and(|name| !name.is_writable()) => {
                *t.selections = at_corners(blocks, t.buffer, t.pane.viewport.columns());
                self.fail();
                return Ok(());
            }
            Operator::Yank => {
                let yanked = yank_blocks(blocks, t.buffer, t.pane.viewport.columns());
                self.registers
                    .write(register, yanked, Write::Yank, t.clipboard.as_deref_mut());
                *t.selections = at_corners(blocks, t.buffer, t.pane.viewport.columns());
                return Ok(());
            }
            Operator::Delete | Operator::Change => BlockEdit::Delete,
            Operator::Lowercase => BlockEdit::Transform(Transform::Lowercase),
            Operator::Uppercase => BlockEdit::Transform(Transform::Uppercase),
            Operator::ToggleCase => BlockEdit::Transform(Transform::ToggleCase),
            Operator::Indent | Operator::Outdent => {
                return self.shift_blocks(operation, blocks, t);
            }
        };
        // Como `op_delete` de vim, `d` en una sola línea vacía no copia nada.
        let empty_line =
            |block: &Block| block.first == block.last && t.buffer.line_text(block.first).is_empty();
        let copies = how == BlockEdit::Delete
            && !(operator == Operator::Delete && blocks.iter().all(empty_line));
        if copies {
            let deleted = yank_blocks(blocks, t.buffer, t.pane.viewport.columns());
            self.registers.write(
                register,
                deleted,
                deletion(blocks),
                t.clipboard.as_deref_mut(),
            );
        }
        let change = Change::Block(BlockChange::Operate(operator), primary_size(blocks));
        let dot = Dot::new(change, None, register);
        if operator == Operator::Change {
            return self.change_blocks(blocks, dot, t);
        }
        edit_blocks(how, blocks, t)?;
        self.last_change = Some(dot);
        Ok(())
    }

    /// Corre cada bloque `count` niveles en un paso de undo; el cursor
    /// vuelve al byte de la esquina en la primera línea, como en vim.
    fn shift_blocks(
        &mut self,
        operation: BlockOperation,
        blocks: &PerSelection<Block>,
        t: &mut Target<'_>,
    ) -> Result<(), VimError> {
        let BlockOperation {
            operator,
            count,
            register,
        } = operation;
        let shift = operator.shift().unwrap_or(Shift::Right);
        let columns = t.pane.viewport.columns();
        let corners: Vec<usize> = blocks
            .iter()
            .map(|block| {
                block.top_left(t.buffer, columns).get() - t.buffer.line_start(block.first).get()
            })
            .collect();
        let plans: Vec<_> = blocks
            .iter()
            .map(|block| {
                block::shift(
                    t.buffer,
                    block,
                    shift,
                    count.unwrap_or(1),
                    columns,
                    self.indent,
                )
            })
            .collect();
        t.buffer
            .begin_transaction(at_corners(blocks, t.buffer, t.pane.viewport.columns()))?;
        let (line_starts, _) = block::apply(t.buffer, &plans)?;
        let cursors = line_starts
            .into_iter()
            .zip(corners)
            .map(|(start, column)| at_byte_column(t.buffer, t.buffer.line_of(start), column))
            .collect();
        if let Some(after) = cursors_at(t.buffer, cursors) {
            *t.selections = after;
        }
        t.buffer.commit_transaction(t.selections.clone())?;
        let change = Change::Block(BlockChange::Operate(operator), primary_size(blocks));
        self.last_change = Some(Dot::new(change, count, register));
        Ok(())
    }

    fn replace_blocks(
        &mut self,
        c: char,
        blocks: &PerSelection<Block>,
        t: &mut Target<'_>,
    ) -> Result<(), VimError> {
        let how = BlockEdit::Transform(Transform::Replace(c));
        edit_blocks(how, blocks, t)?;
        let change = Change::Block(BlockChange::Replace(c), primary_size(blocks));
        self.last_change = Some(Dot::new(change, None, None));
        Ok(())
    }
}

/// Lo que `y` copia de todos los bloques, uno debajo del otro.
pub(super) fn yank_blocks(
    blocks: &PerSelection<Block>,
    buffer: &Buffer,
    columns: Columns,
) -> Register {
    let mut yanked = blocks
        .iter()
        .map(|block| block::yank(buffer, block, columns));
    let first = yanked.next().unwrap_or_else(|| Register::chars(""));
    yanked.fold(first, |all, more| {
        let width = match (all.shape, more.shape) {
            (Shape::Block { width: a }, Shape::Block { width: b }) => a.max(b),
            (Shape::Block { width }, _) | (_, Shape::Block { width }) => width,
            _ => 0,
        };
        Register {
            text: format!("{}\n{}", all.text, more.text),
            shape: Shape::Block { width },
        }
    })
}

pub(super) fn corners(
    blocks: &PerSelection<Block>,
    buffer: &Buffer,
    columns: Columns,
) -> Vec<ByteOffset> {
    blocks
        .iter()
        .map(|block| block.top_left(buffer, columns))
        .collect()
}

/// Un cursor en la esquina de cada bloque, donde vim lo deja antes de
/// operar.
pub(super) fn at_corners(
    blocks: &PerSelection<Block>,
    buffer: &Buffer,
    columns: Columns,
) -> Selections {
    blocks.to_selections(|block| {
        Selection::cursor(clamp_normal(buffer, block.top_left(buffer, columns)))
    })
}

/// `[count]v` tras operar sobre un bloque: uno del mismo tamaño, con las
/// líneas y el ancho multiplicados, medido desde donde se ve el cursor.
pub(super) fn reselect_block(
    size: BlockSize,
    times: usize,
    buffer: &Buffer,
    head: ByteOffset,
    columns: Columns,
) -> Selection {
    let line = buffer.line_of(head);
    let lines = size.lines.saturating_mul(times);
    let last = line
        .saturating_add(lines.saturating_sub(1))
        .min(buffer.last_line());
    let (end, goal) = match size.width {
        None => (buffer.line_end(last), Goal::LineEnd),
        Some(width) => {
            let start = columns.cursor_at(buffer, head, true);
            let want = start + width.saturating_mul(times).saturating_sub(1);
            let end = columns.offset_at(buffer, last, want, true);
            (end, Goal::Column(want))
        }
    };
    Selection {
        anchor: head,
        head: end,
        goal,
    }
}

/// Un operador sobre bloques, con su count y su registro.
#[derive(Clone, Copy)]
struct BlockOperation {
    operator: Operator,
    count: Option<usize>,
    register: Option<RegisterName>,
}

/// Un cursor en cada offset, fuera del fin de línea; `None` si no hay
/// ninguno.
fn cursors_at(buffer: &Buffer, cursors: Vec<ByteOffset>) -> Option<Selections> {
    Selections::new(
        cursors
            .into_iter()
            .map(|cursor| Selection::cursor(clamp_normal(buffer, cursor))),
    )
}

fn primary_size(blocks: &PerSelection<Block>) -> BlockSize {
    blocks.iter().next().map_or(
        BlockSize {
            lines: 1,
            width: Some(1),
        },
        Block::size,
    )
}

/// Como en vim, el borrado de un bloque de varias líneas va a `"1`; el de
/// una, a `"-`.
fn deletion(blocks: &PerSelection<Block>) -> Write {
    Write::Delete {
        spans_lines: blocks.iter().any(|block| block.first != block.last),
        always_numbered: false,
    }
}

/// `v_swap_corners` de vim para `O`.
fn swap_corners(buffer: &Buffer, selection: &Selection, columns: Columns) -> Selection {
    let (head_start, head_end) = columns.span_of(buffer, selection.head);
    let (anchor_start, anchor_end) = columns.span_of(buffer, selection.anchor);
    let left = head_start.min(anchor_start);
    let right = head_end.max(anchor_end);
    let head_line = buffer.line_of(selection.head);
    let anchor_line = buffer.line_of(selection.anchor);
    let at = |line, vcol| columns.offset_at(buffer, line, vcol, true);
    let head = at(head_line, right);
    let (anchor, head, want) = if head == selection.head {
        (at(anchor_line, right), at(head_line, left), left)
    } else {
        (at(anchor_line, left), head, right)
    };
    Selection {
        anchor,
        head,
        goal: Goal::Column(want),
    }
}

/// Borra o transforma cada bloque en un solo paso de undo.
fn edit_blocks(
    how: BlockEdit,
    blocks: &PerSelection<Block>,
    t: &mut Target<'_>,
) -> Result<(), VimError> {
    let plans: Vec<_> = blocks
        .iter()
        .map(|block| block::edit(t.buffer, block, how, t.pane.viewport.columns()))
        .collect();
    t.buffer
        .begin_transaction(at_corners(blocks, t.buffer, t.pane.viewport.columns()))?;
    let (cursors, _) = block::apply(t.buffer, &plans)?;
    if let Some(after) = cursors_at(t.buffer, cursors) {
        *t.selections = after;
    }
    t.buffer.commit_transaction(t.selections.clone())?;
    Ok(())
}
