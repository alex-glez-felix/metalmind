//! Capas que se pisan (un link dentro de una negrita dentro de un heading)
//! a tramos planos, sin solaparse, partidos por línea.

use std::ops::Range;

use super::{Conceal, Decorations, Role, Span, Style};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Layer {
    Role(Role),
    Strong,
    Emphasis,
    Strike,
    Mark,
    Code,
    Conceal(Conceal),
    /// El índice del link en los de su línea.
    Link(usize),
}

/// Cuál color gana cuando se pisan: la sintaxis siempre se ve como
/// sintaxis, y lo de bloque (cita, heading) solo es el fondo.
fn priority(role: Role) -> u8 {
    match role {
        Role::Text => 0,
        Role::Quote => 1,
        Role::TaskDone => 2,
        Role::Frontmatter => 3,
        Role::Heading => 4,
        Role::FrontmatterKey => 5,
        Role::Checkbox | Role::ListMarker | Role::BlockId => 6,
        Role::Link | Role::Url | Role::Wikilink | Role::Embed | Role::Tag => 7,
        Role::Code | Role::Token(_) | Role::FenceInfo | Role::Math => 8,
        Role::Comment => 9,
        Role::Markup => 10,
    }
}

/// Lo que cuenta cada posición de `Active::flags`.
const FLAGS: [Decorations; 5] = [
    Decorations::STRONG,
    Decorations::EMPHASIS,
    Decorations::STRIKE,
    Decorations::MARK,
    Decorations::CODE,
];

#[derive(Debug, Default)]
pub(crate) struct Layers {
    items: Vec<(Range<usize>, Layer)>,
}

#[derive(Debug, Default)]
struct Active {
    roles: Vec<Role>,
    flags: [usize; 5],
    conceals: Vec<Conceal>,
    links: Vec<usize>,
}

impl Active {
    fn apply(&mut self, layer: Layer, on: bool) {
        match layer {
            Layer::Role(role) => toggle(&mut self.roles, role, on),
            Layer::Conceal(conceal) => toggle(&mut self.conceals, conceal, on),
            Layer::Link(link) => toggle(&mut self.links, link, on),
            Layer::Strong => count(&mut self.flags[0], on),
            Layer::Emphasis => count(&mut self.flags[1], on),
            Layer::Strike => count(&mut self.flags[2], on),
            Layer::Mark => count(&mut self.flags[3], on),
            Layer::Code => count(&mut self.flags[4], on),
        }
    }

    fn span(&self, range: Range<usize>) -> Option<Span> {
        let role = self
            .roles
            .iter()
            .copied()
            .max_by_key(|&role| priority(role))
            .unwrap_or_default();
        let decorations = FLAGS
            .iter()
            .zip(self.flags)
            .filter(|&(_, count)| count > 0)
            .fold(Decorations::default(), |all, (&flag, _)| all.with(flag));
        let style = Style { role, decorations };
        let conceal = self.conceals.last().copied();
        let link = self.links.last().copied();
        (style != Style::default() || conceal.is_some() || link.is_some()).then_some(Span {
            range,
            style,
            conceal,
            link,
        })
    }
}

fn toggle<T: PartialEq>(active: &mut Vec<T>, item: T, on: bool) {
    if on {
        active.push(item);
    } else if let Some(at) = active.iter().position(|other| *other == item) {
        active.swap_remove(at);
    }
}

fn count(counter: &mut usize, on: bool) {
    *counter = if on {
        *counter + 1
    } else {
        counter.saturating_sub(1)
    };
}

impl Layers {
    pub(crate) fn push(&mut self, range: Range<usize>, layer: Layer) {
        if !range.is_empty() {
            self.items.push((range, layer));
        }
    }

    /// Los tramos de cada línea de `lines`, rangos sobre el mismo texto que
    /// las capas; cada tramo queda relativo al inicio de su línea.
    pub(crate) fn flatten(self, lines: &[Range<usize>]) -> Vec<Vec<Span>> {
        let mut events: Vec<(usize, bool, Layer)> = Vec::with_capacity(self.items.len() * 2);
        for (range, layer) in self.items {
            events.push((range.start, true, layer));
            events.push((range.end, false, layer));
        }
        // En la misma posición, lo que termina antes que lo que empieza.
        events.sort_by_key(|&(at, on, _)| (at, on));
        let mut out: Vec<Vec<Span>> = vec![Vec::new(); lines.len()];
        let mut active = Active::default();
        let mut line = 0;
        let mut from = 0;
        for (at, on, layer) in events {
            if at > from
                && let Some(span) = active.span(from..at)
            {
                place(&mut out, lines, &mut line, &span);
            }
            active.apply(layer, on);
            from = at;
        }
        out
    }
}

/// Reparte `span` entre las líneas que toca, sin los saltos de línea.
fn place(out: &mut [Vec<Span>], lines: &[Range<usize>], line: &mut usize, span: &Span) {
    while *line < lines.len() && lines[*line].end < span.range.start {
        *line += 1;
    }
    let mut index = *line;
    while let Some(bounds) = lines.get(index) {
        if bounds.start >= span.range.end {
            break;
        }
        let start = span.range.start.max(bounds.start);
        let end = span.range.end.min(bounds.end);
        if start < end {
            let piece = Span {
                range: start - bounds.start..end - bounds.start,
                ..span.clone()
            };
            push_merged(&mut out[index], piece);
        }
        index += 1;
    }
}

fn push_merged(spans: &mut Vec<Span>, span: Span) {
    if let Some(last) = spans.last_mut()
        && last.range.end == span.range.start
        && (last.style, last.conceal, last.link) == (span.style, span.conceal, span.link)
    {
        last.range.end = span.range.end;
        return;
    }
    spans.push(span);
}
