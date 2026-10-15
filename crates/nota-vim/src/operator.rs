use std::ops::Range;

use nota_buffer::{Bias, Buffer, ByteOffset, Edit, EditMap, Selection, Selections};

use crate::VimError;
use crate::lines::{Want, clamp_normal, first_non_blank, in_indent, is_blank, last_char};
use crate::register::Register;
use crate::span::{Region, Target};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Operator {
    Delete,
    Change,
    Yank,
}

/// Una selección y lo que el operador abarca desde ella (`None` si la motion
/// falló ahí).
#[derive(Clone, Debug)]
pub(crate) struct Operand {
    pub selection: Selection,
    pub target: Option<Target>,
}

pub(crate) struct Outcome {
    pub selections: Selections,
    pub register: Option<Register>,
}

impl Operator {
    /// `Change` deja la transacción abierta para el modo Insert. `None` si
    /// falló en todas las selecciones.
    pub(crate) fn apply(
        self,
        buffer: &mut Buffer,
        current: &Selections,
        operands: &[Operand],
    ) -> Result<Option<Outcome>, VimError> {
        if operands.iter().all(|operand| operand.target.is_none()) {
            return Ok(None);
        }
        let plans: Vec<Option<Plan>> = operands
            .iter()
            .map(|operand| Plan::new(buffer, self, operand))
            .collect();
        let regions = || plans.iter().flatten().map(|plan| &plan.region);
        let has_text = regions().any(|region| !region.is_empty());
        let register = if has_text || self == Self::Yank {
            Some(Register::from_regions(buffer, regions())?)
        } else {
            None
        };
        let selections = match self {
            Self::Yank => at_starts(buffer, current, operands, &plans),
            Self::Delete if !has_text => at_starts(buffer, current, operands, &plans),
            Self::Delete | Self::Change => {
                let selections = edit(buffer, current, operands, &plans)?;
                if self == Self::Delete {
                    buffer.commit_transaction(selections.clone())?;
                }
                selections
            }
        };
        Ok(Some(Outcome {
            selections,
            register,
        }))
    }
}

/// Lo que un operador hará con una selección, calculado antes de editar.
struct Plan {
    region: Region,
    /// Rango que se borra; las líneas completas se llevan su `\n`.
    removal: Range<ByteOffset>,
    start: ByteOffset,
    want: Want,
    adjusted: bool,
    operator: Operator,
}

impl Plan {
    fn new(buffer: &Buffer, operator: Operator, operand: &Operand) -> Option<Self> {
        let target = operand.target.clone()?;
        let region = match operator {
            Operator::Delete => whole_lines_if_delimited(buffer, target.region),
            Operator::Change | Operator::Yank => target.region,
        };
        let removal = match operator {
            Operator::Delete => removal(buffer, &region),
            Operator::Change | Operator::Yank => content(buffer, &region),
        };
        Some(Self {
            region,
            removal,
            start: target.start,
            want: Want::of(buffer, &operand.selection),
            adjusted: target.adjusted,
            operator,
        })
    }

    /// Cursor sobre el texto ya editado.
    fn cursor_after(&self, buffer: &Buffer, map: &EditMap) -> Selection {
        let start = map.map(self.removal.start, Bias::Before);
        match (&self.region, self.operator) {
            (Region::Lines { .. }, Operator::Delete) => {
                let line = buffer.line_of(start);
                if self.adjusted {
                    Selection::cursor(first_non_blank(buffer, line).min(last_char(buffer, line)))
                } else {
                    Selection::cursor(self.want.offset_on(buffer, line)).with_goal(self.want.goal())
                }
            }
            (_, Operator::Delete) => Selection::cursor(clamp_normal(buffer, start)),
            _ => Selection::cursor(start),
        }
    }
}

/// Vim mueve el cursor al inicio del objetivo aunque no edite nada.
fn at_starts(
    buffer: &Buffer,
    current: &Selections,
    operands: &[Operand],
    plans: &[Option<Plan>],
) -> Selections {
    collect(
        current,
        operands
            .iter()
            .zip(plans)
            .map(|(operand, plan)| match plan {
                Some(plan) => Selection::cursor(clamp_normal(buffer, plan.start)),
                None => operand.selection,
            }),
    )
}

/// Abre la transacción, borra y devuelve los cursores resultantes.
fn edit(
    buffer: &mut Buffer,
    current: &Selections,
    operands: &[Operand],
    plans: &[Option<Plan>],
) -> Result<Selections, VimError> {
    let before = collect(
        current,
        operands.iter().zip(plans).map(|(operand, plan)| {
            plan.as_ref()
                .map_or(operand.selection, |plan| Selection::cursor(plan.start))
        }),
    );
    buffer.begin_transaction(before)?;
    let map = buffer.apply(union(
        plans.iter().flatten().map(|plan| plan.removal.clone()),
    ))?;
    let buffer = &*buffer;
    Ok(collect(
        current,
        operands
            .iter()
            .zip(plans)
            .map(|(operand, plan)| match plan {
                Some(plan) => plan.cursor_after(buffer, &map),
                None => Selection::cursor(map.map(operand.selection.head, Bias::Before)),
            }),
    ))
}

/// Un `d` multi-línea que va de la indentación a un final seguido solo de
/// blancos borra líneas completas (`op_delete` de vim).
fn whole_lines_if_delimited(buffer: &Buffer, region: Region) -> Region {
    let Region::Chars(range) = &region else {
        return region;
    };
    let first = buffer.line_of(range.start);
    let last = buffer.line_of(range.end);
    let rest_is_blank = buffer
        .text_in(range.end..buffer.line_end(last))
        .is_ok_and(|rest| rest.chars().all(is_blank));
    if last > first && rest_is_blank && in_indent(buffer, range.start) {
        Region::Lines { first, last }
    } else {
        region
    }
}

fn content(buffer: &Buffer, region: &Region) -> Range<ByteOffset> {
    match *region {
        Region::Chars(ref range) => range.clone(),
        Region::Lines { first, last } => buffer.line_start(first)..buffer.line_end(last),
    }
}

fn removal(buffer: &Buffer, region: &Region) -> Range<ByteOffset> {
    match *region {
        Region::Chars(ref range) => range.clone(),
        Region::Lines { first, last } if last < buffer.last_line() => {
            buffer.line_start(first)..buffer.line_start(last + 1)
        }
        Region::Lines { first, .. } if first > 0 => buffer.line_end(first - 1)..buffer.len(),
        Region::Lines { .. } => ByteOffset::ZERO..buffer.len(),
    }
}

/// Hay una selección por operando, así que nunca queda vacío.
fn collect(current: &Selections, selections: impl Iterator<Item = Selection>) -> Selections {
    Selections::new(selections).unwrap_or_else(|| current.clone())
}

/// Une rangos que se solapan para que varias selecciones no choquen.
fn union(ranges: impl IntoIterator<Item = Range<ByteOffset>>) -> Vec<Edit> {
    let mut ranges: Vec<_> = ranges.into_iter().collect();
    ranges.sort_by_key(|range| range.start);
    let mut merged: Vec<Range<ByteOffset>> = Vec::with_capacity(ranges.len());
    for range in ranges {
        match merged.last_mut() {
            Some(last) if range.start < last.end || range == *last => {
                last.end = last.end.max(range.end);
            }
            _ => merged.push(range),
        }
    }
    merged.into_iter().map(Edit::delete).collect()
}
