//! `>` y `<` sobre las líneas que abarca cada operando, como `op_shift` de
//! nvim fuera de un bloque.

use std::collections::BTreeMap;
use std::ops::RangeInclusive;

use metalmind_buffer::{Buffer, Edit, PerSelection, Selection};

use crate::VimError;
use crate::columns::Columns;
use crate::indent::{self, Indent, Shift, leading_blanks};
use crate::lines::Want;
use crate::operator::{Operand, Outcome};
use crate::span::{Region, Target};

/// Corre la sangría de cada línea una sola vez aunque la abarquen varias
/// selecciones, sin tocar las vacías. El cursor va a la primera línea, en
/// la columna que buscaba (`beginline` de vim con `nostartofline`), y el
/// undo vuelve a donde empezaba lo abarcado.
pub(crate) fn apply(
    shift: Shift,
    levels: usize,
    buffer: &mut Buffer,
    operands: &PerSelection<Operand>,
    columns: Columns,
    indent: Indent,
) -> Result<Outcome, VimError> {
    let steps = operands.map(|operand| Step::new(buffer, operand, columns));
    if steps.iter().all(|step| step.lines.is_none()) {
        return Ok(Outcome::Missed(steps.to_selections(|step| step.before)));
    }
    let width = indent.levels_width(levels);
    let mut lines = BTreeMap::new();
    for step in steps.iter() {
        let Some(range) = step.lines.clone() else {
            continue;
        };
        let moved = step.width(buffer, shift, width, columns);
        for line in range {
            lines.entry(line).or_insert(moved);
        }
    }
    let edits: Vec<Edit> = lines
        .into_iter()
        .filter_map(|(line, width)| {
            let text = buffer.line_text(line);
            let (len, new) = indent::shift(&text, shift, width, columns, indent)?;
            let start = buffer.line_start(line);
            Some(Edit::replace(start..start + len, new))
        })
        .collect();
    buffer.begin_transaction(steps.to_selections(|step| step.before))?;
    buffer.apply(edits)?;
    let selections = steps.to_selections(|step| step.after(buffer, columns));
    buffer.commit_transaction(selections.clone())?;
    Ok(Outcome::Applied {
        selections,
        register: None,
        spans_lines: false,
    })
}

/// Lo que el operador hará con una selección.
struct Step {
    /// Las líneas que corre; `None` si el objeto falló ahí.
    lines: Option<RangeInclusive<usize>>,
    /// Un ítem de lista con sus hijos: `<` los corre a todos lo mismo.
    nested: bool,
    /// Donde vuelve el undo: el inicio de lo abarcado, o adonde lo mandó el
    /// fallo.
    before: Selection,
    want: Want,
}

impl Step {
    fn new(buffer: &Buffer, operand: &Operand, columns: Columns) -> Self {
        let selection = &operand.selection;
        match &operand.target {
            Ok(target) => Self {
                lines: Some(lines_of(buffer, target)),
                nested: target.nested,
                before: Selection::cursor(target.start),
                want: Want::of(buffer, selection, tab_end(target, selection), columns),
            },
            Err(missed) => Self {
                lines: None,
                nested: false,
                before: missed.selection(selection),
                want: Want::of(buffer, selection, true, columns),
            },
        }
    }

    /// Cuántas columnas corre cada línea. Un ítem con hijos que `<` no
    /// puede correr entero se corre lo que pueda su primera línea, para que
    /// los hijos sigan más adentro que él.
    fn width(&self, buffer: &Buffer, shift: Shift, width: usize, columns: Columns) -> usize {
        match (&self.lines, shift, self.nested) {
            (Some(lines), Shift::Left, true) => {
                let first = buffer.line_text(*lines.start());
                width.min(columns.line_width(leading_blanks(&first)))
            }
            _ => width,
        }
    }

    fn after(&self, buffer: &Buffer, columns: Columns) -> Selection {
        match &self.lines {
            Some(lines) => Selection::cursor(self.want.offset_on(buffer, *lines.start(), columns)),
            None => self.before,
        }
    }
}

fn lines_of(buffer: &Buffer, target: &Target) -> RangeInclusive<usize> {
    match target.region {
        // El final es exclusivo: un `v$` que llega al salto termina en su línea.
        Region::Chars(ref range) if range.end > range.start => {
            buffer.line_of(range.start)..=buffer.line_of(buffer.prev_boundary(range.end))
        }
        Region::Chars(ref range) => buffer.line_of(range.start)..=buffer.line_of(range.start),
        Region::Lines { first, last } => first..=last,
    }
}

/// Como el cursor de Visual sobre un tab, que se ve al inicio si va antes
/// del otro extremo (ver [`crate::lines::tab_end`]).
fn tab_end(target: &Target, selection: &Selection) -> bool {
    !target.visual || selection.head > selection.anchor
}
