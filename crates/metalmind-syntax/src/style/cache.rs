//! El estilo de cada línea, recalculado por tramos y a pedido.
//!
//! Cada tramo empieza en una línea con estado conocido. Al editar, se vuelve
//! a la última línea "dura" antes del cambio (en blanco, de cerca o la
//! primera): ningún tramo anterior mira más allá de ella. Desde ahí se
//! repinta hasta lo que se pida y, pasado lo editado, en cuanto un tramo
//! empieza en el mismo estado que antes, lo de abajo vuelve a valer.

use super::region::{self, Region, Start, is_delimiter};
use super::{LineStyle, Lines};
use crate::code::{is_blank, opening_fence};

#[derive(Debug, Clone, Default)]
struct Entry {
    /// El estado con que empieza el tramo, si esta línea abre uno.
    start: Option<Start>,
    /// Ningún tramo de antes mira esta línea ni las de después.
    hard: bool,
    style: LineStyle,
}

/// Dónde cierra el frontmatter, que depende de líneas lejanas: un `---`
/// en la línea 0 sin cierre pasa a abrir un frontmatter en cuanto se
/// escribe otro `---` muy abajo.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Frontmatter {
    /// Si la línea 0 es un `---`.
    opens: bool,
    /// La línea del `---` que cierra.
    close: Option<usize>,
    pending: Pending,
}

/// Lo que falta revisar tras editar.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Pending {
    Nothing,
    /// Todo de nuevo: cambió la línea 0 o hubo varias ediciones.
    All,
    /// Solo estas líneas nuevas pueden cerrar antes.
    Inserted {
        from: usize,
        to: usize,
    },
    /// Se borró el cierre: el primero desde aquí.
    From(usize),
}

/// Los estilos de las líneas de una nota.
#[derive(Debug, Clone)]
pub struct Styles {
    lines: Vec<Entry>,
    /// Las líneas `0..valid` valen para el texto de hoy.
    valid: usize,
    /// El estado con que empieza la línea `valid`.
    frontier: Start,
    /// Si la línea `valid` es dura por cómo terminó el tramo de antes.
    frontier_hard: bool,
    /// Hasta aquí llega lo editado: antes no se puede reusar nada.
    dirty_end: usize,
    /// Las líneas de una pasada anterior, desde `valid` hasta aquí, con el
    /// estado en que esa pasada llegó al final.
    stale: Option<(usize, Start)>,
    frontmatter: Frontmatter,
}

impl Styles {
    /// Una nota de `lines` líneas, sin nada calculado todavía.
    #[must_use]
    pub fn new(lines: usize) -> Self {
        Self {
            lines: vec![Entry::default(); lines.max(1)],
            valid: 0,
            frontier: Start::Document,
            frontier_hard: true,
            dirty_end: 0,
            stale: None,
            frontmatter: Frontmatter {
                opens: false,
                close: None,
                pending: Pending::All,
            },
        }
    }

    #[must_use]
    pub fn line_count(&self) -> usize {
        self.lines.len()
    }

    /// Cuántas líneas ya tienen estilo, desde la primera.
    #[must_use]
    pub fn styled(&self) -> usize {
        self.valid
    }

    /// El estilo de `line`, si ya se calculó ([`Styles::ensure`]).
    #[must_use]
    pub fn line(&self, line: usize) -> Option<&LineStyle> {
        (line < self.valid).then(|| &self.lines[line].style)
    }

    /// Las líneas `first..first + removed` pasaron a ser `inserted` líneas.
    pub fn edit(&mut self, first: usize, removed: usize, inserted: usize) {
        let first = first.min(self.lines.len());
        let removed = removed.min(self.lines.len() - first);
        let old_end = first + removed;
        let shift = |line: usize| {
            if line <= first {
                line
            } else if line >= old_end {
                line - removed + inserted
            } else {
                first + inserted
            }
        };
        self.stale = if self.valid > old_end {
            Some((shift(self.valid), self.frontier))
        } else {
            self.stale
                .filter(|&(end, _)| end > old_end)
                .map(|(end, state)| (shift(end), state))
        };
        self.dirty_end = shift(self.dirty_end).max(first + inserted);
        self.lines.splice(
            first..old_end,
            std::iter::repeat_n(Entry::default(), inserted),
        );
        if self.lines.is_empty() {
            self.lines.push(Entry::default());
        }
        self.frontmatter = self.frontmatter.edited(first, removed, inserted);
        self.rewind(first.saturating_sub(1).min(self.valid));
    }

    /// Todo cambió (otro texto, una edición que no se pudo seguir).
    pub fn reset(&mut self, lines: usize) {
        *self = Self::new(lines);
    }

    /// Calcula el estilo de las líneas hasta `through` incluida.
    pub fn ensure(&mut self, through: usize, source: &impl Lines) {
        if source.line_count() != self.lines.len() {
            self.reset(source.line_count());
        }
        self.settle_frontmatter(source);
        let through = through.min(self.lines.len() - 1);
        while self.valid <= through {
            let first = self.valid;
            let hard = self.frontier_hard || is_hard(&source.line(first), self.frontier);
            let Region {
                lines,
                next,
                next_hard,
                ..
            } = region::style(source, first, self.frontier, self.frontmatter.close);
            let count = lines.len().max(1);
            for (offset, style) in lines.into_iter().enumerate() {
                if let Some(entry) = self.lines.get_mut(first + offset) {
                    *entry = Entry {
                        start: (offset == 0).then_some(self.frontier),
                        hard: offset == 0 && hard,
                        style,
                    };
                }
            }
            self.valid = (first + count).min(self.lines.len());
            self.frontier = next;
            self.frontier_hard = next_hard;
            self.converge();
        }
    }

    fn converge(&mut self) {
        let Some((end, state)) = self.stale else {
            return;
        };
        if self.valid >= end {
            self.stale = None;
        } else if self.valid >= self.dirty_end
            && self.lines[self.valid].start == Some(self.frontier)
        {
            self.valid = end;
            self.frontier = state;
            self.frontier_hard = false;
            self.stale = None;
        }
    }

    /// Vuelve a la última línea dura hasta `line`.
    fn rewind(&mut self, line: usize) {
        if self.valid == 0 {
            return;
        }
        let from = line.min(self.valid - 1);
        let hard = (0..=from)
            .rev()
            .find(|&index| self.lines[index].hard)
            .unwrap_or(0);
        if hard < self.valid {
            self.valid = hard;
            self.frontier = self.lines[hard].start.unwrap_or(Start::Document);
            self.frontier_hard = true;
        }
    }

    /// Si lo editado pudo mover el cierre del frontmatter, lo busca de
    /// nuevo; si se movió, todo lo que cambia de lado se repinta.
    fn settle_frontmatter(&mut self, source: &impl Lines) {
        let before = self.frontmatter.close;
        let delimiter = |lines: std::ops::Range<usize>| {
            lines
                .filter(|&line| line > 0)
                .find(|&line| is_delimiter(&source.line(line)))
        };
        let close = match self.frontmatter.pending {
            Pending::Nothing => return,
            Pending::All => {
                self.frontmatter.opens = region::opens_frontmatter(&source.line(0));
                if self.frontmatter.opens {
                    delimiter(1..source.line_count())
                } else {
                    None
                }
            }
            Pending::Inserted { from, to } => delimiter(from..to).or(before),
            Pending::From(from) => delimiter(from..source.line_count()),
        };
        self.frontmatter.close = close;
        self.frontmatter.pending = Pending::Nothing;
        if close != before {
            let end = |close: Option<usize>| close.map_or(1, |line| line + 1);
            self.dirty_end = self.dirty_end.max(end(before)).max(end(close));
            self.valid = 0;
            self.frontier = Start::Document;
            self.frontier_hard = true;
        }
    }
}

impl Frontmatter {
    /// Tras reemplazar `removed` líneas desde `first` por `inserted`. Un
    /// cierre nuevo solo puede estar en lo insertado.
    fn edited(self, first: usize, removed: usize, inserted: usize) -> Self {
        let pending = |pending| Self { pending, ..self };
        match (self.pending, self.close) {
            _ if first == 0 => pending(Pending::All),
            (Pending::Nothing, _) if !self.opens => self,
            (Pending::Nothing, Some(close)) if close < first => self,
            (Pending::Nothing, Some(close)) if close >= first + removed => Self {
                close: Some(close - removed + inserted),
                pending: Pending::Inserted {
                    from: first,
                    to: first + inserted,
                },
                ..self
            },
            (Pending::Nothing, Some(_)) => pending(Pending::From(first)),
            (Pending::Nothing, None) => pending(Pending::Inserted {
                from: first,
                to: first + inserted,
            }),
            _ => pending(Pending::All),
        }
    }
}

/// Si una línea es dura: ningún tramo de antes la cruza.
pub(crate) fn is_hard(line: &str, start: Start) -> bool {
    matches!(start, Start::Document | Start::Fence(_))
        || is_blank(line)
        || opening_fence(line).is_some()
}
