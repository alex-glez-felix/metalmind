//! `>` y `<` sobre las líneas que abarca cada operando, como `op_shift` de
//! nvim fuera de un bloque.

use std::ops::RangeInclusive;

use metalmind_buffer::{Buffer, Edit, PerSelection, Selection};

use crate::VimError;
use crate::columns::Columns;
use crate::indent::{self, Shift};
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
) -> Result<Outcome, VimError> {
    let steps = operands.map(|operand| Step::new(buffer, operand, columns));
    if steps.iter().all(|step| step.lines.is_none()) {
        return Ok(Outcome::Missed(steps.to_selections(|step| step.before)));
    }
    let mut lines: Vec<usize> = steps
        .iter()
        .filter_map(|step| step.lines.clone())
        .flatten()
        .collect();
    lines.sort_unstable();
    lines.dedup();
    let edits: Vec<Edit> = lines
        .into_iter()
        .filter_map(|line| {
            let (len, indent) = indent::shift(&buffer.line_text(line), shift, levels, columns)?;
            let start = buffer.line_start(line);
            Some(Edit::replace(start..start + len, indent))
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
                before: Selection::cursor(target.start),
                want: Want::of(buffer, selection, tab_end(target, selection), columns),
            },
            Err(missed) => Self {
                lines: None,
                before: missed.selection(selection),
                want: Want::of(buffer, selection, true, columns),
            },
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
        Region::Chars(ref range) => buffer.line_of(range.start)..=buffer.line_of(range.end),
        Region::Lines { first, last } => first..=last,
    }
}

/// Como el cursor de Visual sobre un tab, que se ve al inicio si va antes
/// del otro extremo (ver [`crate::lines::tab_end`]).
fn tab_end(target: &Target, selection: &Selection) -> bool {
    !target.visual || selection.head > selection.anchor
}
