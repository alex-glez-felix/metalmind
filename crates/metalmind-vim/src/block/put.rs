//! Pegar por bloque, como `do_put` de vim: cada línea del registro va a la
//! misma columna de pantalla en una línea distinta.

use metalmind_buffer::{Buffer, ByteOffset, Edit};

use super::plan::Plan;
use crate::PasteSide;
use crate::columns::Columns;

/// `p`/`P` de un registro por bloque desde `head`, `count` veces a lo
/// ancho. Las líneas que falten se agregan al final; las cortas se rellenan
/// con espacios hasta la columna, y cada trozo, hasta el ancho del bloque si
/// le sigue texto.
pub(crate) fn put(
    buffer: &Buffer,
    head: ByteOffset,
    lines: &[&str],
    width: usize,
    side: PasteSide,
    count: usize,
    columns: Columns,
) -> Plan {
    let first = buffer.line_of(head);
    let line_start = buffer.line_start(first);
    let (column, at) = start_column(
        &buffer.line_text(first),
        head.get() - line_start.get(),
        side,
        columns,
    );
    let mut plan = Plan {
        edits: Vec::new(),
        at: line_start + at,
        shift: 0,
    };
    let mut appended = String::new();
    for (index, piece) in lines.iter().enumerate() {
        let line = first + index;
        let exists = line <= buffer.last_line();
        let text = if exists {
            buffer.line_text(line)
        } else {
            "".into()
        };
        let row = Row::at(&text, column, columns);
        let padding = (width + 1).saturating_sub(columns.loose_width(piece));
        let mut middle = " ".repeat(row.startspaces);
        for time in 0..count {
            middle.push_str(piece);
            if time + 1 < count || !row.short {
                middle.push_str(&" ".repeat(padding));
            }
        }
        middle.push_str(&" ".repeat(row.endspaces));
        if index == 0 {
            plan.shift = row.startspaces;
        }
        if exists {
            let start = buffer.line_start(line) + row.textcol;
            plan.edits
                .push(Edit::replace(start..start + row.removed, middle));
        } else {
            appended.push('\n');
            appended.push_str(&middle);
        }
    }
    if !appended.is_empty() {
        plan.edits.push(Edit::insert(buffer.len(), appended));
    }
    plan
}

/// Un registro de una línea por caracteres sobre un bloque ya borrado: se
/// pega en cada línea donde algún carácter empiece en la columna del cursor,
/// o que termine justo ahí (el `VIsual_active` de `do_put`). El cursor va al
/// último carácter pegado en la primera.
pub(crate) fn put_in_each_line(
    buffer: &Buffer,
    head: ByteOffset,
    last: usize,
    text: &str,
    side: PasteSide,
    count: usize,
    columns: Columns,
) -> Plan {
    let first = buffer.line_of(head);
    let line_start = buffer.line_start(first);
    let line = buffer.line_text(first);
    let mut at = head.get() - line_start.get();
    let next = columns.cells(&line).find(|cell| cell.end > at);
    if let Some(cell) = next.filter(|_| side == PasteSide::After) {
        at = cell.end;
    }
    let (vcol, _) = columns.span_at(&line, at);
    let pasted = text.repeat(count);
    let last_char = pasted.chars().last().map_or(0, char::len_utf8);
    let mut plan = Plan {
        edits: Vec::new(),
        at: line_start + at,
        shift: pasted.len() - last_char,
    };
    if pasted.is_empty() {
        return plan;
    }
    plan.edits
        .push(Edit::insert(line_start + at, pasted.clone()));
    for line in first + 1..=last.min(buffer.last_line()) {
        let text = buffer.line_text(line);
        if let Some(byte) = starts_at(&text, vcol, columns) {
            plan.edits
                .push(Edit::insert(buffer.line_start(line) + byte, pasted.clone()));
        }
    }
    plan
}

/// La columna donde va el bloque y el byte del cursor: con `p`, tras el
/// carácter del cursor, salvo en el fin de línea.
fn start_column(line: &str, byte: usize, side: PasteSide, columns: Columns) -> (usize, usize) {
    let cell = columns.cells(line).find(|cell| cell.end > byte);
    match (side, cell) {
        (PasteSide::After, Some(cell)) => (cell.vcol + cell.width, cell.end),
        _ => (columns.span_at(line, byte).0, byte),
    }
}

/// Dónde entra el bloque en una línea, como el `bd` de `do_put`.
struct Row {
    textcol: usize,
    /// Bytes que se quitan: un tab partido, que pasa a ser espacios.
    removed: usize,
    startspaces: usize,
    endspaces: usize,
    /// La línea no tiene texto tras la columna del bloque.
    short: bool,
}

impl Row {
    fn at(line: &str, column: usize, columns: Columns) -> Self {
        let mut row = Self {
            textcol: line.len(),
            removed: 0,
            startspaces: 0,
            endspaces: 0,
            short: false,
        };
        let mut reached = 0;
        let mut previous = None;
        for cell in columns.cells(line) {
            if reached >= column {
                row.textcol = cell.start;
                break;
            }
            reached = cell.vcol + cell.width;
            previous = Some(cell);
        }
        if reached >= column && row.textcol == line.len() {
            row.short = reached == column;
        }
        if reached < column {
            row.short = true;
            row.startspaces = column - reached;
        } else if let Some(cell) = previous.filter(|_| reached > column) {
            row.textcol = cell.start;
            if cell.is_tab {
                row.endspaces = reached - column;
                row.startspaces = cell.width - row.endspaces;
                row.removed = cell.end - cell.start;
            } else {
                row.startspaces = cell.width - (reached - column);
            }
        }
        row
    }
}

/// El byte de `line` donde empieza la columna `vcol`, o su fin si termina
/// justo ahí; `None` si un carácter la parte o la línea no llega.
fn starts_at(line: &str, vcol: usize, columns: Columns) -> Option<usize> {
    let mut reached = 0;
    for cell in columns.cells(line) {
        if cell.vcol == vcol {
            return Some(cell.start);
        }
        if cell.vcol > vcol {
            return None;
        }
        reached = cell.vcol + cell.width;
    }
    (reached == vcol).then_some(line.len())
}
