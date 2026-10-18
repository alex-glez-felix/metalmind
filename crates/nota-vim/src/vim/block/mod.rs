//! Visual por bloque (`ctrl-v`): operadores, `r`, `O`, `.` y `[count]ctrl-v`;
//! `I`, `A` y `c` en `insert.rs`, y `p`/`P` en `paste.rs`.

mod insert;
mod paste;

use nota_buffer::{Buffer, ByteOffset, Goal, PerSelection, Selection, Selections};

use super::Vim;
use crate::block::{self, Block, BlockEdit, BlockSize};
use crate::columns::Columns;
use crate::dot::{BlockChange, Change, Dot};
use crate::indent::Shift;
use crate::lines::{at_byte_column, clamp_normal};
use crate::register::{Clipboard, Register, RegisterName, Shape, Write};
use crate::transform::Transform;
use crate::visual::{LastVisual, Reselect};
use crate::{Mode, Operator, VimError, VisualKind};

impl Vim {
    fn columns(&self) -> Columns {
        self.viewport.columns()
    }

    /// `d`, `c`, `y`, `~`, `u`, `U`, `>` y `<` sobre cada bloque. Como en
    /// vim, el count solo cuenta en `>` y `<`, que corren tantos niveles, y
    /// `.` repite sobre un bloque del mismo tamaño.
    pub(super) fn operate_block(
        &mut self,
        operator: Operator,
        buffer: &mut Buffer,
        selections: &mut Selections,
        clipboard: &mut dyn Clipboard,
    ) -> Result<(), VimError> {
        let count = self.take_count();
        let register = self.selected.take();
        let blocks = self.finish_block(buffer, selections);
        let operation = BlockOperation {
            operator,
            count,
            register,
        };
        self.block_operator(operation, &blocks, buffer, selections, clipboard)
    }

    /// Completa `r` en un bloque.
    pub(super) fn replace_in_block(
        &mut self,
        c: char,
        buffer: &mut Buffer,
        selections: &mut Selections,
    ) -> Result<(), VimError> {
        let blocks = self.finish_block(buffer, selections);
        self.replace_blocks(c, &blocks, buffer, selections)
    }

    /// `O`: en un bloque, el cursor va a la otra esquina de su línea y el
    /// otro extremo a la de la suya (`v_swap_corners` de vim). Si el cursor
    /// ya estaba en la esquina derecha, va a la izquierda.
    pub(super) fn other_corner(
        &mut self,
        kind: VisualKind,
        buffer: &Buffer,
        selections: &mut Selections,
    ) {
        if kind != VisualKind::Block {
            self.other_end(kind, selections);
            return;
        }
        self.reset();
        let columns = self.columns();
        *selections = selections.map(|selection| swap_corners(buffer, selection, columns));
    }

    /// `[count]v` tras operar sobre un bloque: uno del mismo tamaño, con las
    /// líneas y el ancho multiplicados, medido desde donde se ve el cursor.
    pub(super) fn reselect_block(
        &self,
        size: BlockSize,
        times: usize,
        buffer: &Buffer,
        head: ByteOffset,
    ) -> Selection {
        let line = buffer.line_of(head);
        let lines = size.lines.saturating_mul(times);
        let last = line
            .saturating_add(lines.saturating_sub(1))
            .min(buffer.last_line());
        let (end, goal) = match size.width {
            None => (buffer.line_end(last), Goal::LineEnd),
            Some(width) => {
                let columns = self.columns();
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

    /// `.` sobre un bloque de `size` desde cada cursor.
    pub(super) fn repeat_block(
        &mut self,
        change: BlockChange,
        size: BlockSize,
        buffer: &mut Buffer,
        selections: &mut Selections,
        clipboard: &mut dyn Clipboard,
    ) -> Result<(), VimError> {
        let count = self.take_count();
        let register = self.selected.take();
        self.reset();
        let columns = self.columns();
        let blocks =
            selections.map_to(|selection| Block::sized(buffer, selection.head, size, columns));
        match change {
            BlockChange::Operate(operator) => {
                let operation = BlockOperation {
                    operator,
                    count,
                    register,
                };
                self.block_operator(operation, &blocks, buffer, selections, clipboard)
            }
            BlockChange::Replace(c) => self.replace_blocks(c, &blocks, buffer, selections),
            BlockChange::Insert { append } => {
                self.start_block_insert(append, count, &blocks, buffer, selections)
            }
        }
    }

    /// Antes de operar: sale a Normal, `gv` recuerda la selección, y `.` y
    /// `[count]v` el tamaño del primer bloque.
    fn finish_block(&mut self, buffer: &Buffer, selections: &Selections) -> PerSelection<Block> {
        self.reset();
        self.mode = Mode::Normal;
        self.last_visual = Some(LastVisual::save(buffer, selections, VisualKind::Block));
        let columns = self.columns();
        let blocks = selections.map_to(|selection| Block::of(buffer, selection, columns));
        self.last_visual_size = Some(Reselect::Block(primary_size(&blocks)));
        blocks
    }

    fn block_operator(
        &mut self,
        operation: BlockOperation,
        blocks: &PerSelection<Block>,
        buffer: &mut Buffer,
        selections: &mut Selections,
        clipboard: &mut dyn Clipboard,
    ) -> Result<(), VimError> {
        let BlockOperation {
            operator, register, ..
        } = operation;
        let how = match operator {
            // Como vim, `".d` no opera, pero el cursor ya fue a la esquina.
            _ if register.is_some_and(|name| !name.is_writable()) => {
                *selections = self.at_corners(blocks, buffer);
                self.fail();
                return Ok(());
            }
            Operator::Yank => {
                let yanked = self.yank_blocks(blocks, buffer);
                self.registers
                    .write(register, yanked, Write::Yank, clipboard);
                *selections = self.at_corners(blocks, buffer);
                return Ok(());
            }
            Operator::Delete | Operator::Change => BlockEdit::Delete,
            Operator::Lowercase => BlockEdit::Transform(Transform::Lowercase),
            Operator::Uppercase => BlockEdit::Transform(Transform::Uppercase),
            Operator::ToggleCase => BlockEdit::Transform(Transform::ToggleCase),
            Operator::Indent | Operator::Outdent => {
                return self.shift_blocks(operation, blocks, buffer, selections);
            }
        };
        // Como `op_delete` de vim, `d` en una sola línea vacía no copia nada.
        let empty_line =
            |block: &Block| block.first == block.last && buffer.line_text(block.first).is_empty();
        let copies = how == BlockEdit::Delete
            && !(operator == Operator::Delete && blocks.iter().all(empty_line));
        if copies {
            let deleted = self.yank_blocks(blocks, buffer);
            self.registers
                .write(register, deleted, deletion(blocks), clipboard);
        }
        let change = Change::Block(BlockChange::Operate(operator), primary_size(blocks));
        let dot = Dot::new(change, None, register);
        if operator == Operator::Change {
            return self.change_blocks(blocks, dot, buffer, selections);
        }
        self.edit_blocks(how, blocks, buffer, selections)?;
        self.last_change = Some(dot);
        Ok(())
    }

    /// Corre cada bloque `count` niveles en un paso de undo; el cursor
    /// vuelve al byte de la esquina en la primera línea, como en vim.
    fn shift_blocks(
        &mut self,
        operation: BlockOperation,
        blocks: &PerSelection<Block>,
        buffer: &mut Buffer,
        selections: &mut Selections,
    ) -> Result<(), VimError> {
        let BlockOperation {
            operator,
            count,
            register,
        } = operation;
        let shift = operator.shift().unwrap_or(Shift::Right);
        let columns = self.columns();
        let corners: Vec<usize> = blocks
            .iter()
            .map(|block| {
                block.top_left(buffer, columns).get() - buffer.line_start(block.first).get()
            })
            .collect();
        let plans: Vec<_> = blocks
            .iter()
            .map(|block| block::shift(buffer, block, shift, count.unwrap_or(1), columns))
            .collect();
        buffer.begin_transaction(self.at_corners(blocks, buffer))?;
        let (line_starts, _) = block::apply(buffer, &plans)?;
        let cursors = line_starts
            .into_iter()
            .zip(corners)
            .map(|(start, column)| at_byte_column(buffer, buffer.line_of(start), column))
            .collect();
        if let Some(after) = cursors_at(buffer, cursors) {
            *selections = after;
        }
        buffer.commit_transaction(selections.clone())?;
        let change = Change::Block(BlockChange::Operate(operator), primary_size(blocks));
        self.last_change = Some(Dot::new(change, count, register));
        Ok(())
    }

    fn replace_blocks(
        &mut self,
        c: char,
        blocks: &PerSelection<Block>,
        buffer: &mut Buffer,
        selections: &mut Selections,
    ) -> Result<(), VimError> {
        let how = BlockEdit::Transform(Transform::Replace(c));
        self.edit_blocks(how, blocks, buffer, selections)?;
        let change = Change::Block(BlockChange::Replace(c), primary_size(blocks));
        self.last_change = Some(Dot::new(change, None, None));
        Ok(())
    }

    /// Borra o transforma cada bloque en un solo paso de undo.
    fn edit_blocks(
        &self,
        how: BlockEdit,
        blocks: &PerSelection<Block>,
        buffer: &mut Buffer,
        selections: &mut Selections,
    ) -> Result<(), VimError> {
        let plans: Vec<_> = blocks
            .iter()
            .map(|block| block::edit(buffer, block, how, self.columns()))
            .collect();
        buffer.begin_transaction(self.at_corners(blocks, buffer))?;
        let (cursors, _) = block::apply(buffer, &plans)?;
        if let Some(after) = cursors_at(buffer, cursors) {
            *selections = after;
        }
        buffer.commit_transaction(selections.clone())?;
        Ok(())
    }

    /// Lo que `y` copia de todos los bloques, uno debajo del otro.
    fn yank_blocks(&self, blocks: &PerSelection<Block>, buffer: &Buffer) -> Register {
        let mut yanked = blocks
            .iter()
            .map(|block| block::yank(buffer, block, self.columns()));
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

    fn corners(&self, blocks: &PerSelection<Block>, buffer: &Buffer) -> Vec<ByteOffset> {
        blocks
            .iter()
            .map(|block| block.top_left(buffer, self.columns()))
            .collect()
    }

    /// Un cursor en la esquina de cada bloque, donde vim lo deja antes de
    /// operar.
    fn at_corners(&self, blocks: &PerSelection<Block>, buffer: &Buffer) -> Selections {
        blocks.to_selections(|block| {
            Selection::cursor(clamp_normal(buffer, block.top_left(buffer, self.columns())))
        })
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
