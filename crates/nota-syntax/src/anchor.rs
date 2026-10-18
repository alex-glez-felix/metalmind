//! Lo que un link puede nombrar dentro de una nota: headings (`# Título`) y
//! bloques (`^id` al final de una línea).

use std::ops::Range;

use crate::Scan;

/// Un heading ATX: de uno a seis `#` al inicio de línea, tras hasta tres
/// espacios, y un espacio o el fin de línea.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Heading {
    pub level: u8,
    /// La línea entera, sin el salto de línea.
    pub span: Range<usize>,
    /// El título, sin los `#` de cierre ni los espacios de los lados.
    pub text: Range<usize>,
}

/// Un `^id` al final de una línea, como en Obsidian: nombra el bloque donde
/// está.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlockId {
    /// Desde el `^`.
    pub span: Range<usize>,
    /// Sin el `^`.
    pub id: Range<usize>,
}

/// El heading que empieza en `at`, el inicio de una línea. Todo lo que mira
/// es ASCII, así que recorre bytes.
pub(crate) fn heading_at(text: &str, at: usize) -> Option<Heading> {
    let bytes = text.as_bytes();
    let indent = run(&bytes[at..], |b| b == b' ', 4);
    let open = at + indent;
    let marks = run(&bytes[open..], |b| b == b'#', 7);
    if indent > 3 || !(1..=6).contains(&marks) {
        return None;
    }
    let line_end = text[at..].find('\n').map_or(text.len(), |i| at + i);
    let line_end = if line_end > at && bytes[line_end - 1] == b'\r' {
        line_end - 1
    } else {
        line_end
    };
    let after = open + marks;
    if after < line_end && !is_blank(bytes[after]) {
        return None;
    }
    let start = after + run(&bytes[after..line_end], is_blank, usize::MAX);
    let mut end = trim_blanks_end(bytes, start, line_end);
    let closing = end - run_back(&bytes[start..end], |b| b == b'#');
    if closing == start || is_blank(bytes[closing - 1]) {
        end = trim_blanks_end(bytes, start, closing);
    }
    Some(Heading {
        level: u8::try_from(marks).ok()?,
        span: at..line_end,
        text: start..end,
    })
}

fn is_blank(b: u8) -> bool {
    b == b' ' || b == b'\t'
}

/// Cuántos bytes del principio de `bytes` cumplen `keep`, hasta `limit`.
fn run(bytes: &[u8], keep: impl Fn(u8) -> bool, limit: usize) -> usize {
    bytes.iter().take(limit).take_while(|&&b| keep(b)).count()
}

fn run_back(bytes: &[u8], keep: impl Fn(u8) -> bool) -> usize {
    bytes.iter().rev().take_while(|&&b| keep(b)).count()
}

fn trim_blanks_end(bytes: &[u8], start: usize, end: usize) -> usize {
    end - run_back(&bytes[start..end], is_blank)
}

/// El bloque cuyo `^` está en `at`, ya tras un espacio o al inicio de línea:
/// letras y números ASCII o `-`, y nada más que espacios hasta el fin de línea.
pub(crate) fn block_at(text: &str, at: usize) -> Option<BlockId> {
    let start = at + 1;
    let id = text[start..]
        .bytes()
        .take_while(|b| b.is_ascii_alphanumeric() || *b == b'-')
        .count();
    let end = start + id;
    let rest = text[end..].split('\n').next().unwrap_or_default();
    (id > 0 && rest.trim_end_matches([' ', '\t', '\r']).is_empty()).then_some(BlockId {
        span: at..end,
        id: start..end,
    })
}

impl Scan {
    /// El heading que nombra `path` (lo que va tras el `#` de un link),
    /// sin distinguir mayúsculas. `H1#H2` es el primer `H2` dentro de la
    /// sección del primer `H1`.
    #[must_use]
    pub fn heading(&self, text: &str, path: &str) -> Option<&Heading> {
        let mut within = &self.headings[..];
        let mut found = None;
        for part in path
            .split('#')
            .map(str::trim)
            .filter(|part| !part.is_empty())
        {
            let at = within
                .iter()
                .position(|heading| same(&text[heading.text.clone()], part))?;
            let heading = &within[at];
            let section = within[at + 1..]
                .iter()
                .position(|next| next.level <= heading.level)
                .map_or(within.len(), |end| at + 1 + end);
            within = &within[at + 1..section];
            found = Some(heading);
        }
        found
    }

    /// El bloque `^id`, sin distinguir mayúsculas.
    #[must_use]
    pub fn block(&self, text: &str, id: &str) -> Option<&BlockId> {
        self.blocks
            .iter()
            .find(|block| text[block.id.clone()].eq_ignore_ascii_case(id))
    }
}

fn same(heading: &str, wanted: &str) -> bool {
    heading == wanted || heading.to_lowercase() == wanted.to_lowercase()
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;

    use super::*;

    fn title(line: &str) -> Option<(u8, &str)> {
        heading_at(line, 0).map(|heading| (heading.level, &line[heading.text]))
    }

    #[test]
    fn atx_headings() {
        assert_eq!(title("# Uno"), Some((1, "Uno")));
        assert_eq!(title("###   Tres  ##  "), Some((3, "Tres")));
        assert_eq!(title("   ## Con sangría\r"), Some((2, "Con sangría")));
        assert_eq!(title("## C# #"), Some((2, "C#")));
        assert_eq!(title("## a#"), Some((2, "a#")));
        assert_eq!(title("#"), Some((1, "")));
        assert_eq!(title("#tag"), None);
        assert_eq!(title("####### siete"), None);
        assert_eq!(title("    # código"), None);
    }

    #[test]
    fn block_ids_end_the_line() {
        fn id(text: &str, at: usize) -> Option<&str> {
            block_at(text, at).map(|block| &text[block.id])
        }
        assert_eq!(id("x ^abc-1", 2), Some("abc-1"));
        assert_eq!(id("x ^abc  \r\ny", 2), Some("abc"));
        assert_eq!(id("x ^abc y", 2), None);
        assert_eq!(id("x ^", 2), None);
        assert_eq!(id("x ^a_b", 2), None);
    }
}
