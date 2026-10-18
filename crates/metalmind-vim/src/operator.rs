use std::ops::Range;

use metalmind_buffer::{
    Bias, Buffer, ByteOffset, Edit, EditMap, PerSelection, Selection, Selections,
};

use crate::VimError;
use crate::columns::Columns;
use crate::indent::{Indent, Shift};
use crate::lines::{Want, clamp_normal, first_non_blank, in_indent, is_blank, last_char};
use crate::register::Register;
use crate::shift;
use crate::span::{Missed, Region, Target};
use crate::transform::Transform;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Operator {
    Delete,
    Change,
    Yank,
    /// `gu`, o `u` en Visual.
    Lowercase,
    /// `gU`, o `U` en Visual.
    Uppercase,
    /// `g~`, o `~` en Visual.
    ToggleCase,
    /// `>`: corre la sangría un `shiftwidth` a la derecha.
    Indent,
    /// `<`: la corre a la izquierda.
    Outdent,
}

/// Una selección y lo que el operador abarca desde ella, o dónde queda el
/// cursor si el objeto falló ahí.
#[derive(Clone, Debug)]
pub(crate) struct Operand {
    pub selection: Selection,
    pub target: Result<Target, Missed>,
}

pub(crate) enum Outcome {
    /// Falló en todas las selecciones: no es un cambio, pero el cursor puede
    /// haberse movido.
    Missed(Selections),
    Applied {
        selections: Selections,
        register: Option<Register>,
        /// Abarca más de una línea o líneas enteras: va a `"1`, como en vim.
        spans_lines: bool,
    },
}

/// Lo que hace un operador con el texto que abarca.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Effect {
    /// Lo copia al registro y, salvo `y`, lo quita.
    Cut(Cut),
    /// Cambia cada carácter sin tocar el registro.
    Transform(Transform),
    /// Corre la sangría de cada línea que abarca.
    Shift(Shift),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Cut {
    Delete,
    Change,
    Yank,
}

impl Operator {
    fn effect(self) -> Effect {
        match self {
            Self::Delete => Effect::Cut(Cut::Delete),
            Self::Change => Effect::Cut(Cut::Change),
            Self::Yank => Effect::Cut(Cut::Yank),
            Self::Lowercase => Effect::Transform(Transform::Lowercase),
            Self::Uppercase => Effect::Transform(Transform::Uppercase),
            Self::ToggleCase => Effect::Transform(Transform::ToggleCase),
            Self::Indent => Effect::Shift(Shift::Right),
            Self::Outdent => Effect::Shift(Shift::Left),
        }
    }

    /// Si cambia cada carácter sin tocar el registro, como `gu`.
    pub(crate) fn transforms(self) -> bool {
        matches!(self.effect(), Effect::Transform(_))
    }

    /// Hacia dónde corre la sangría, si es `>` o `<`.
    pub(crate) fn shift(self) -> Option<Shift> {
        match self.effect() {
            Effect::Shift(shift) => Some(shift),
            Effect::Cut(_) | Effect::Transform(_) => None,
        }
    }

    /// `Change` deja la transacción abierta para el modo Insert. `levels`
    /// es cuánto corren `>` y `<` la sangría: el count en Visual y 1 fuera
    /// de él, como en vim; `indent`, con qué.
    pub(crate) fn apply(
        self,
        buffer: &mut Buffer,
        operands: &PerSelection<Operand>,
        columns: Columns,
        levels: usize,
        indent: Indent,
    ) -> Result<Outcome, VimError> {
        match self.effect() {
            Effect::Cut(cut) => cut.apply(buffer, operands, columns),
            Effect::Transform(transform) => transform.apply(buffer, operands),
            Effect::Shift(shift) => shift::apply(shift, levels, buffer, operands, columns, indent),
        }
    }
}

impl Cut {
    fn apply(
        self,
        buffer: &mut Buffer,
        operands: &PerSelection<Operand>,
        columns: Columns,
    ) -> Result<Outcome, VimError> {
        let steps = operands.map(|operand| Step {
            selection: operand.selection,
            plan: Plan::new(buffer, self, operand, columns),
        });
        if steps.iter().all(|step| step.plan.is_err()) {
            return Ok(Outcome::Missed(at_starts(buffer, &steps)));
        }
        let regions = || steps.iter().filter_map(Step::region);
        let has_text = regions().any(|region| !region.is_empty());
        let spans_lines = regions().any(|region| region.spans_lines(buffer));
        let register = if has_text || self == Self::Yank {
            Some(Register::from_regions(buffer, regions())?)
        } else {
            None
        };
        let selections = match self {
            Self::Yank => at_starts(buffer, &steps),
            Self::Delete if !has_text => at_starts(buffer, &steps),
            Self::Delete | Self::Change => {
                let selections = edit(buffer, self, &steps)?;
                if self == Self::Delete {
                    buffer.commit_transaction(selections.clone())?;
                }
                selections
            }
        };
        Ok(Outcome::Applied {
            selections,
            register,
            spans_lines,
        })
    }
}

/// Una selección y su plan, si el operador aplica ahí.
struct Step {
    selection: Selection,
    plan: Result<Plan, Missed>,
}

impl Step {
    fn region(&self) -> Option<&Region> {
        self.plan.as_ref().ok().map(|plan| &plan.region)
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
    cut: Cut,
    columns: Columns,
}

impl Plan {
    fn new(buffer: &Buffer, cut: Cut, operand: &Operand, columns: Columns) -> Result<Self, Missed> {
        let target = operand.target.clone()?;
        let region = match cut {
            Cut::Delete if !target.visual => whole_lines_if_delimited(buffer, target.region),
            Cut::Delete | Cut::Change | Cut::Yank => target.region,
        };
        let removal = match cut {
            Cut::Delete => removal(buffer, &region),
            Cut::Change | Cut::Yank => content(buffer, &region),
        };
        Ok(Self {
            region,
            removal,
            start: target.start,
            want: Want::of(buffer, &operand.selection, true, columns),
            adjusted: target.adjusted,
            cut,
            columns,
        })
    }

    /// Cursor sobre el texto ya editado.
    fn cursor_after(&self, buffer: &Buffer, map: &EditMap) -> Selection {
        let start = map.map(self.removal.start, Bias::Before);
        match (&self.region, self.cut) {
            (Region::Lines { .. }, Cut::Delete) => {
                let line = buffer.line_of(start);
                if self.adjusted {
                    Selection::cursor(first_non_blank(buffer, line).min(last_char(buffer, line)))
                } else {
                    let offset = self.want.offset_on(buffer, line, self.columns);
                    Selection::cursor(offset).with_goal(self.want.goal())
                }
            }
            (_, Cut::Delete) => Selection::cursor(clamp_normal(buffer, start)),
            _ => Selection::cursor(start),
        }
    }
}

/// Vim mueve el cursor al inicio del objetivo aunque no edite nada.
fn at_starts(buffer: &Buffer, steps: &PerSelection<Step>) -> Selections {
    steps.to_selections(|step| match &step.plan {
        Ok(plan) => Selection::cursor(clamp_normal(buffer, plan.start)),
        Err(missed) => missed.selection(&step.selection),
    })
}

/// Abre la transacción, borra y devuelve los cursores resultantes.
fn edit(buffer: &mut Buffer, cut: Cut, steps: &PerSelection<Step>) -> Result<Selections, VimError> {
    let before = steps.to_selections(|step| match &step.plan {
        Ok(plan) => Selection::cursor(plan.start),
        Err(_) => step.selection,
    });
    buffer.begin_transaction(before)?;
    let removals = steps
        .iter()
        .filter_map(|step| step.plan.as_ref().ok())
        .map(|plan| plan.removal.clone());
    let map = buffer.apply(union(removals).into_iter().map(Edit::delete).collect())?;
    let buffer = &*buffer;
    Ok(steps.to_selections(|step| match &step.plan {
        Ok(plan) => plan.cursor_after(buffer, &map),
        // Lo que borró otra selección puede dejarlo sobre un fin de línea.
        Err(missed) => {
            let head = map.map(missed.selection(&step.selection).head, Bias::Before);
            Selection::cursor(match cut {
                Cut::Delete => clamp_normal(buffer, head),
                Cut::Change | Cut::Yank => head,
            })
        }
    }))
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

pub(crate) fn content(buffer: &Buffer, region: &Region) -> Range<ByteOffset> {
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

/// Une rangos que se solapan para que varias selecciones no choquen.
pub(crate) fn union(ranges: impl IntoIterator<Item = Range<ByteOffset>>) -> Vec<Range<ByteOffset>> {
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
    merged
}
