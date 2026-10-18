use std::ops::Range;

use nota_buffer::{Bias, Buffer, ByteOffset, Edit, EditMap, PerSelection, Selection, Selections};

use crate::lines::{clamp_normal, first_non_blank};
use crate::operator::content;
use crate::register::Register;
use crate::span::{Region, Target};
use crate::{PasteSide, VimError};

/// `p`/`P`. El cursor queda donde lo deja vim: en el primer no blanco de la
/// primera línea pegada, al inicio de un pegado de varias líneas o en el
/// último carácter de un pegado de una línea.
pub(crate) fn paste(
    buffer: &mut Buffer,
    selections: &Selections,
    register: &Register,
    side: PasteSide,
    count: usize,
) -> Result<Selections, VimError> {
    if repeated(register, count, register.is_linewise()).is_empty() {
        return Ok(selections.clone());
    }
    buffer.begin_transaction(selections.clone())?;
    let after = put(buffer, selections, register, side, count)?;
    buffer.commit_transaction(after.clone())?;
    Ok(after)
}

/// Como [`paste`], dentro de una transacción ya abierta.
pub(crate) fn put(
    buffer: &mut Buffer,
    selections: &Selections,
    register: &Register,
    side: PasteSide,
    count: usize,
) -> Result<Selections, VimError> {
    let linewise = register.is_linewise();
    let text = repeated(register, count, linewise);
    if text.is_empty() {
        return Ok(selections.clone());
    }
    let insertions =
        selections.map_to(|selection| insertion_point(buffer, selection.head, linewise, side));
    let piece = match (linewise, side) {
        (true, PasteSide::After) => format!("\n{text}"),
        (true, PasteSide::Before) => format!("{text}\n"),
        (false, _) => text,
    };
    let map = buffer.apply(
        insertions
            .iter()
            .map(|&at| Edit::insert(at, piece.clone()))
            .collect(),
    )?;
    Ok(insertions
        .to_selections(|&at| Selection::cursor(cursor_after(buffer, &map, at, register, side))))
}

fn repeated(register: &Register, count: usize, linewise: bool) -> String {
    let separator = if linewise { "\n" } else { "" };
    vec![register.text.as_str(); count].join(separator)
}

fn insertion_point(
    buffer: &Buffer,
    head: ByteOffset,
    linewise: bool,
    side: PasteSide,
) -> ByteOffset {
    let line = buffer.line_of(head);
    match (linewise, side) {
        (true, PasteSide::After) => buffer.line_end(line),
        (true, PasteSide::Before) => buffer.line_start(line),
        (false, PasteSide::After) if head < buffer.line_end(line) => buffer.next_boundary(head),
        (false, _) => head,
    }
}

fn cursor_after(
    buffer: &Buffer,
    map: &EditMap,
    at: ByteOffset,
    register: &Register,
    side: PasteSide,
) -> ByteOffset {
    let start = map.map(at, Bias::Before);
    if register.is_linewise() {
        let first_pasted = match side {
            PasteSide::After => buffer.line_of(start) + 1,
            PasteSide::Before => buffer.line_of(start),
        };
        return first_non_blank(buffer, first_pasted);
    }
    // Lo pegado puede empezar con un salto de línea, al final de la línea.
    if register.text.contains('\n') {
        clamp_normal(buffer, start)
    } else {
        buffer.prev_boundary(map.map(at, Bias::After))
    }
}

/// `p`/`P` en Visual: lo pegado reemplaza lo seleccionado, como
/// `nv_put_opt` de vim. Sobre líneas se pega por líneas; sobre caracteres,
/// un registro por líneas parte la línea y va en medio.
pub(crate) fn replace(
    buffer: &mut Buffer,
    targets: &PerSelection<Target>,
    register: &Register,
    count: usize,
) -> Result<Selections, VimError> {
    let replacements = targets.map(|target| Replacement::new(buffer, target, register, count));
    buffer.begin_transaction(targets.to_selections(|target| Selection::cursor(target.start)))?;
    let map = buffer.apply(without_overlaps(&replacements))?;
    let after = replacements.to_selections(|replacement| replacement.cursor_after(buffer, &map));
    buffer.commit_transaction(after.clone())?;
    Ok(after)
}

/// Lo que va en lugar de una selección.
struct Replacement {
    range: Range<ByteOffset>,
    text: String,
    /// Se pega por líneas: el cursor va al primer no blanco de la primera.
    lines: bool,
    /// Un registro por líneas partió la línea: lo pegado empieza en la de
    /// abajo.
    split: bool,
}

impl Replacement {
    fn new(buffer: &Buffer, target: &Target, register: &Register, count: usize) -> Self {
        let range = content(buffer, &target.region);
        match target.region {
            Region::Lines { .. } => Self {
                range,
                text: repeated(register, count, true),
                lines: true,
                split: false,
            },
            Region::Chars(_) if register.is_linewise() => Self {
                range,
                text: format!("\n{}\n", repeated(register, count, true)),
                lines: true,
                split: true,
            },
            Region::Chars(_) => Self {
                range,
                text: repeated(register, count, false),
                lines: false,
                split: false,
            },
        }
    }

    fn cursor_after(&self, buffer: &Buffer, map: &EditMap) -> Selection {
        let start = map.map(self.range.start, Bias::Before);
        let offset = if self.lines {
            first_non_blank(buffer, buffer.line_of(start) + usize::from(self.split))
        } else if self.text.contains('\n') {
            clamp_normal(buffer, start)
        } else {
            buffer.prev_boundary(map.map(self.range.start, Bias::After))
        };
        Selection::cursor(offset)
    }
}

/// Dos selecciones por líneas pueden tocar la misma línea; vale la primera.
fn without_overlaps(replacements: &PerSelection<Replacement>) -> Vec<Edit> {
    let mut edits: Vec<Edit> = Vec::with_capacity(replacements.iter().len());
    for replacement in replacements.iter() {
        let overlaps = edits.last().is_some_and(|last| {
            replacement.range.start < last.range.end || replacement.range == last.range
        });
        if !overlaps {
            edits.push(Edit::replace(
                replacement.range.clone(),
                replacement.text.clone(),
            ));
        }
    }
    edits
}
