//! Cómo se recorta una ruta: el nombre del archivo (con la `/` de antes) es
//! lo que distingue una fila de otra, así que es lo último que se pierde.
//! Si lo que coincide cae en la carpeta, se ve también, con el trozo de
//! carpeta que lo rodea.

use std::ops::Range;

use super::{Kept, Widths, around_all, around_within, middle_all, middle_within};

/// Recorta la ruta de `widths` (el ancho de cada grafema) a `budget`. En
/// `slashes` van los grafemas que son `/`; `focus`, si hay, es lo que
/// coincide. Sin ninguna `/` es un texto cualquiera.
#[must_use]
pub fn path(
    widths: &[f32],
    slashes: &[usize],
    focus: Option<Range<usize>>,
    budget: f32,
    ellipsis: f32,
) -> Kept {
    let widths = Widths::new(widths);
    let len = widths.len();
    if widths.total() <= budget {
        return Kept::whole(len);
    }
    let Some(&name_slash) = slashes.last() else {
        return match focus {
            Some(focus) => around_all(&widths, focus, budget, ellipsis),
            None => middle_all(&widths, budget, ellipsis),
        };
    };
    let cut = Cut {
        widths,
        slashes,
        name_slash,
        ellipsis,
    };
    match focus {
        Some(focus) if !focus.is_empty() && focus.end <= name_slash => cut
            .folder_and_name(focus.clone(), budget)
            .unwrap_or_else(|| around_all(&cut.widths, focus, budget, ellipsis)),
        focus => cut.name_first(focus, budget),
    }
}

struct Cut<'a> {
    widths: Widths,
    slashes: &'a [usize],
    /// La última `/`: desde ahí va el nombre.
    name_slash: usize,
    ellipsis: f32,
}

impl Cut<'_> {
    fn len(&self) -> usize {
        self.widths.len()
    }

    /// El nombre entero si cabe, con las carpetas enteras de justo antes que
    /// quepan (`…/carpeta/nombre`); si no cabe, `…/` y el nombre recortado.
    fn name_first(&self, focus: Option<Range<usize>>, budget: f32) -> Kept {
        let len = self.len();
        let name = self.name_slash..len;
        let lead = self.gap_before(self.name_slash);
        let focus = focus.filter(|focus| !focus.is_empty());
        let kept = if self.widths.sum(name) + lead <= budget {
            Some(Kept::new(
                len,
                std::iter::once(self.with_folders(budget)..len),
            ))
        } else {
            let in_name = focus.clone().filter(|focus| focus.start > self.name_slash);
            self.fitting_cut(in_name, budget - lead)
                .map(|name| Kept::new(len, name))
        };
        match (kept, focus) {
            (Some(kept), focus) if focus.as_ref().is_none_or(|focus| kept.shows(focus)) => kept,
            (_, Some(focus)) => around_all(&self.widths, focus, budget, self.ellipsis),
            (_, None) => middle_all(&self.widths, budget, self.ellipsis),
        }
    }

    /// Desde qué `/` se ve la ruta: la de más atrás que quepa con todo lo que
    /// le sigue, así nunca queda un pedazo de carpeta. El nombre ya cabe.
    fn with_folders(&self, budget: f32) -> usize {
        let len = self.len();
        self.slashes
            .iter()
            .rev()
            .copied()
            .take_while(|&slash| self.widths.sum(slash..len) + self.gap_before(slash) <= budget)
            .last()
            .unwrap_or(self.name_slash)
    }

    /// Lo que coincide, con el trozo de carpeta que quepa alrededor, y el
    /// nombre; `None` si no caben los dos.
    fn folder_and_name(&self, focus: Range<usize>, budget: f32) -> Option<Kept> {
        let len = self.len();
        let slash_before = self
            .slashes
            .iter()
            .rev()
            .copied()
            .find(|&slash| slash < focus.start);
        let start = slash_before.unwrap_or(0);
        let end = self
            .slashes
            .iter()
            .copied()
            .find(|&slash| slash >= focus.end)
            .unwrap_or(self.name_slash);
        // Si lo que coincide abre una carpeta, su `/` va con él: así todas
        // las filas empiezan igual, `…/carpeta`.
        let focus = match slash_before {
            Some(slash) if slash + 1 == focus.start => slash..focus.end,
            _ => focus,
        };
        let folder_min = self.widths.sum(focus.clone())
            + self.gap_before(focus.start)
            + self.gap(focus.end < self.name_slash);
        let name = self.name(budget - folder_min)?;
        let name_cost = self.cost(&name);
        let gaps = 0..self.name_slash;
        let folder = around_within(
            &self.widths,
            start..end,
            &gaps,
            focus.clone(),
            budget - name_cost,
            self.ellipsis,
        );
        let kept = Kept::new(len, std::iter::once(folder).chain(name));
        kept.shows(&focus).then_some(kept)
    }

    /// El nombre, desde su `/`, entero o recortado, sin contar la elipsis de
    /// antes; `None` si no queda nada de él.
    fn name(&self, budget: f32) -> Option<Vec<Range<usize>>> {
        let len = self.len();
        if self.widths.sum(self.name_slash..len) <= budget {
            return Some(std::iter::once(self.name_slash..len).collect());
        }
        self.fitting_cut(None, budget)
    }

    /// El nombre recortado, si cabe en `budget` y deja algo más que su `/`.
    fn fitting_cut(&self, focus: Option<Range<usize>>, budget: f32) -> Option<Vec<Range<usize>>> {
        let name = self.cut_name(focus, budget);
        let shows_name = name.iter().map(Range::len).sum::<usize>() > 1;
        (shows_name && self.cost(&name) <= budget).then_some(name)
    }

    /// La `/` del nombre y el nombre recortado: alrededor de `focus` si cae
    /// en él, si no por el medio.
    fn cut_name(&self, focus: Option<Range<usize>>, budget: f32) -> Vec<Range<usize>> {
        let len = self.len();
        let slash = self.name_slash..self.name_slash + 1;
        let name = slash.end..len;
        let budget = budget - self.widths.at(self.name_slash);
        let rest = match focus {
            Some(focus) => vec![around_within(
                &self.widths,
                name.clone(),
                &name,
                focus,
                budget,
                self.ellipsis,
            )],
            None => middle_within(&self.widths, name, budget, self.ellipsis).to_vec(),
        };
        std::iter::once(slash).chain(rest).collect()
    }

    /// Lo que mide el nombre recortado en `ranges`: sus grafemas y una
    /// elipsis por hueco, sin la de antes de la `/`.
    fn cost(&self, ranges: &[Range<usize>]) -> f32 {
        let shown: f32 = ranges
            .iter()
            .map(|range| self.widths.sum(range.clone()))
            .sum();
        let inner = ranges
            .windows(2)
            .filter(|pair| pair[0].end < pair[1].start)
            .count();
        let trailing = ranges.last().is_some_and(|last| last.end < self.len());
        let gaps =
            f32::from(u16::try_from(inner).unwrap_or(u16::MAX)) + f32::from(u8::from(trailing));
        shown + gaps * self.ellipsis
    }

    fn gap_before(&self, start: usize) -> f32 {
        self.gap(start > 0)
    }

    fn gap(&self, needed: bool) -> f32 {
        f32::from(u8::from(needed)) * self.ellipsis
    }
}
