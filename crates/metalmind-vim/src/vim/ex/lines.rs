//! `:d`, `:y`, `:m`, `:t`, `:j`, `:>`, `:<` y el salto de `:12`.

use std::ops::{Range, RangeInclusive};

use metalmind_buffer::{Buffer, ByteOffset, Edit, Selection, Selections};

use super::Failure;
use super::range::Lines;
use crate::columns::Columns;
use crate::ex;
use crate::indent::{self, Shift};
use crate::lines::{Want, first_non_blank, last_char};
use crate::register::{Clipboard, Register, RegisterName, Write};
use crate::vim::Vim;
use crate::{CommandError, VimError};

impl Vim {
    /// Como vim, el cursor queda en el primer no blanco de la línea que
    /// sigue y lo de antes es la mark de `''`.
    pub(super) fn delete_lines(
        &mut self,
        lines: RangeInclusive<usize>,
        register: Option<RegisterName>,
        buffer: &mut Buffer,
        selections: &mut Selections,
        clipboard: &mut dyn Clipboard,
    ) -> Result<(), VimError> {
        self.jumped(buffer, selections)?;
        let deleted = lines_register(buffer, &lines);
        let write = Write::Delete {
            spans_lines: true,
            always_numbered: false,
        };
        self.registers.write(register, deleted, write, clipboard);
        let edits = vec![Edit::delete(whole_lines(buffer, &lines))];
        let first = *lines.start();
        let undo = keep_column(buffer, self.viewport.columns(), selections.primary(), first);
        edit_lines(buffer, selections, undo, edits, |buffer| {
            line_start_cursor(buffer, first.min(buffer.last_line()))
        })
    }

    pub(super) fn yank_lines(
        &mut self,
        lines: &RangeInclusive<usize>,
        register: Option<RegisterName>,
        buffer: &Buffer,
        clipboard: &mut dyn Clipboard,
    ) {
        let yanked = lines_register(buffer, lines);
        self.registers
            .write(register, yanked, Write::Yank, clipboard);
    }

    /// `:j` une la línea con la siguiente; con dos direcciones iguales no
    /// hace nada y en la última línea falla, como vim.
    pub(super) fn join_lines(
        &mut self,
        lines: Lines,
        spaces: bool,
        buffer: &mut Buffer,
        selections: &mut Selections,
    ) -> Result<(), VimError> {
        let (first, mut last) = (lines.first - 1, lines.last - 1);
        if first == last {
            if lines.given >= 2 {
                return Ok(());
            }
            if last == buffer.last_line() {
                self.fail();
                return Ok(());
            }
            last += 1;
        }
        let texts: Vec<_> = (first..=last).map(|line| buffer.line_text(line)).collect();
        let joined = ex::join(texts.iter().map(AsRef::as_ref), spaces);
        let range = buffer.line_start(first)..buffer.line_end(last);
        let edits = vec![Edit::replace(range, joined)];
        let undo = keep_column(buffer, self.viewport.columns(), selections.primary(), first);
        edit_lines(buffer, selections, undo, edits, |buffer| {
            line_start_cursor(buffer, first)
        })
    }

    /// `:>` y `:<`: el cursor queda en el primer no blanco de la última
    /// línea y lo de antes es la mark de `''`.
    pub(super) fn shift_lines(
        &mut self,
        lines: RangeInclusive<usize>,
        shift: Shift,
        levels: usize,
        buffer: &mut Buffer,
        selections: &mut Selections,
    ) -> Result<(), VimError> {
        self.jumped(buffer, selections)?;
        let columns = self.viewport.columns();
        let edits = lines
            .clone()
            .filter_map(|line| {
                let (len, indent) = indent::shift(&buffer.line_text(line), shift, levels, columns)?;
                let start = buffer.line_start(line);
                Some(Edit::replace(start..start + len, indent))
            })
            .collect();
        let last = *lines.end();
        let undo = keep_column(
            buffer,
            self.viewport.columns(),
            selections.primary(),
            *lines.start(),
        );
        edit_lines(buffer, selections, undo, edits, |buffer| {
            line_start_cursor(buffer, last)
        })
    }
}

/// `:m`: `below` es la línea desde 1 debajo de la que van, y 0 arriba
/// de todo. Como vim, el cursor queda en la última movida, en la
/// columna buscada.
pub(super) fn move_lines(
    lines: RangeInclusive<usize>,
    below: usize,
    columns: Columns,
    buffer: &mut Buffer,
    selections: &mut Selections,
) -> Result<(), Failure> {
    let (first, last) = (*lines.start(), *lines.end());
    if below > first && below <= last {
        return Err(CommandError::MoveIntoItself.into());
    }
    let moved = last - first + 1;
    let cursor_line = if below > last {
        below - 1
    } else {
        below + moved - 1
    };
    let mut edits = Vec::new();
    // Debajo de la línea de antes o de la última es quedarse donde están.
    if below != first && below != last + 1 {
        edits.push(Edit::delete(whole_lines(buffer, &lines)));
        edits.push(insert_lines(
            buffer,
            below,
            &buffer.lines_text(first..last + 1),
        ));
    }
    let cursor = buffer.line_of(selections.primary().head);
    let top = first.min(below.min(buffer.last_line()));
    let undo = undo_cursor(buffer, columns, selections, lines.contains(&cursor), top);
    let want = Want::of(buffer, selections.primary(), true, columns);
    Ok(edit_lines(buffer, selections, undo, edits, |buffer| {
        at_column(buffer, columns, want, cursor_line)
    })?)
}

/// `:t` o `:co`, como `:m` pero sin quitarlas de donde estaban.
pub(super) fn copy_lines(
    lines: RangeInclusive<usize>,
    below: usize,
    columns: Columns,
    buffer: &mut Buffer,
    selections: &mut Selections,
) -> Result<(), VimError> {
    let (first, last) = (*lines.start(), *lines.end());
    let text = buffer.lines_text(first..last + 1).into_owned();
    let edits = vec![insert_lines(buffer, below, &text)];
    let cursor_line = below + last - first;
    let above = buffer.line_of(selections.primary().head) < below;
    let undo = undo_cursor(
        buffer,
        columns,
        selections,
        above,
        below.min(buffer.last_line()),
    );
    let want = Want::of(buffer, selections.primary(), true, columns);
    edit_lines(buffer, selections, undo, edits, |buffer| {
        at_column(buffer, columns, want, cursor_line)
    })
}

/// `:12`: a esa línea, desde 0, en la columna buscada.
pub(super) fn go_to(line: usize, columns: Columns, buffer: &Buffer, selections: &mut Selections) {
    *selections = Selections::single(keep_column(buffer, columns, selections.primary(), line));
}

/// Las ediciones de un comando de `:` en un paso de undo, que vuelve a
/// `undo`; el cursor queda donde diga `cursor`.
pub(super) fn edit_lines(
    buffer: &mut Buffer,
    selections: &mut Selections,
    undo: Selection,
    edits: Vec<Edit>,
    cursor: impl FnOnce(&Buffer) -> Selection,
) -> Result<(), VimError> {
    buffer.begin_transaction(Selections::single(undo))?;
    buffer.apply(edits)?;
    let after = Selections::single(cursor(buffer));
    buffer.commit_transaction(after.clone())?;
    *selections = after;
    Ok(())
}

/// Como vim, el undo vuelve al cursor de antes si `stays`, y si no a
/// `line` en la columna buscada.
pub(super) fn undo_cursor(
    buffer: &Buffer,
    columns: Columns,
    selections: &Selections,
    stays: bool,
    line: usize,
) -> Selection {
    let primary = selections.primary();
    if stays {
        *primary
    } else {
        keep_column(buffer, columns, primary, line)
    }
}

/// A `line` en la columna que busca `selection`, como `beginline` de vim
/// con `nostartofline`.
pub(super) fn keep_column(
    buffer: &Buffer,
    columns: Columns,
    selection: &Selection,
    line: usize,
) -> Selection {
    at_column(
        buffer,
        columns,
        Want::of(buffer, selection, true, columns),
        line,
    )
}

fn at_column(buffer: &Buffer, columns: Columns, want: Want, line: usize) -> Selection {
    Selection {
        goal: want.goal(),
        ..Selection::cursor(want.offset_on(buffer, line, columns))
    }
}

/// El primer no blanco de `line`, sin pasar del último carácter.
pub(super) fn line_start_cursor(buffer: &Buffer, line: usize) -> Selection {
    Selection::cursor(first_non_blank(buffer, line).min(last_char(buffer, line)))
}

fn lines_register(buffer: &Buffer, lines: &RangeInclusive<usize>) -> Register {
    Register::lines(buffer.lines_text(*lines.start()..lines.end() + 1))
}

/// Lo que hay que borrar para quitar `lines` con un salto de línea: el de
/// después o, si son las últimas, el de antes.
fn whole_lines(buffer: &Buffer, lines: &RangeInclusive<usize>) -> Range<ByteOffset> {
    let (first, last) = (*lines.start(), *lines.end());
    if last < buffer.last_line() {
        buffer.line_start(first)..buffer.line_start(last + 1)
    } else if first > 0 {
        buffer.line_end(first - 1)..buffer.len()
    } else {
        ByteOffset::new(0)..buffer.len()
    }
}

/// Mete `text`, líneas sin el salto final, debajo de la línea `below` desde
/// 1; 0 es arriba de todo.
fn insert_lines(buffer: &Buffer, below: usize, text: &str) -> Edit {
    if below < buffer.line_count() {
        Edit::insert(buffer.line_start(below), format!("{text}\n"))
    } else {
        Edit::insert(buffer.len(), format!("\n{text}"))
    }
}
