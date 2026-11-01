//! Lo que se ve al principio de una línea: los `>` de una cita, un ítem de
//! lista ([`ListItem`]) y las reglas horizontales. Se mira línea a
//! línea, sin el árbol de bloques de `CommonMark`: una línea sin `>` tras una
//! cita no es parte de ella.

use std::ops::Range;

use crate::list_item::ListItem;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct BlockLine {
    /// Dónde está cada `>`.
    pub quotes: Vec<usize>,
    pub kind: BlockKind,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) enum BlockKind {
    #[default]
    Paragraph,
    Rule(Range<usize>),
    Item(ListItem),
}

fn is_space(b: u8) -> bool {
    b == b' ' || b == b'\t'
}

pub(crate) fn block_line(line: &str) -> BlockLine {
    let (quotes, at) = quote_marks(line);
    let kind = rule(line, at)
        .map(BlockKind::Rule)
        .or_else(|| ListItem::parse_at(line, at).map(BlockKind::Item))
        .unwrap_or_default();
    BlockLine { quotes, kind }
}

/// Dónde termina el prefijo de cita de `line` (sus `>` y el blanco que
/// sigue a cada uno); 0 si no es una cita.
pub(crate) fn quote_end(line: &str) -> usize {
    quote_marks(line).1
}

/// Dónde está cada `>` de la cita y dónde termina el prefijo.
fn quote_marks(line: &str) -> (Vec<usize>, usize) {
    let bytes = line.as_bytes();
    let mut quotes = Vec::new();
    let mut at = 0;
    loop {
        let indent = bytes[at..]
            .iter()
            .take(4)
            .take_while(|&&b| b == b' ')
            .count();
        if indent > 3 || bytes.get(at + indent) != Some(&b'>') {
            break;
        }
        quotes.push(at + indent);
        at += indent + 1;
        if bytes.get(at).copied().is_some_and(is_space) {
            at += 1;
        }
    }
    (quotes, at)
}

pub(crate) fn is_rule(line: &str) -> bool {
    rule(line, quote_end(line)).is_some()
}

/// Tres o más `-`, `*` o `_` iguales, con espacios entre ellos si hace falta
/// y hasta tres de sangría.
fn rule(line: &str, from: usize) -> Option<Range<usize>> {
    let rest = &line[from..];
    let indent = rest.bytes().take_while(|&b| b == b' ').count();
    let marker = *rest.as_bytes().get(indent)?;
    if indent > 3 || !matches!(marker, b'-' | b'*' | b'_') {
        return None;
    }
    let body = rest[indent..].trim_end_matches(['\r']);
    let marks = body.bytes().filter(|&b| b == marker).count();
    let only = body.bytes().all(|b| b == marker || is_space(b));
    let end = from + indent + body.trim_end().len();
    (marks >= 3 && only).then_some(from + indent..end)
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;
    use rstest::rstest;

    use super::*;

    #[rstest]
    #[case("---", Some(0..3))]
    #[case(" * * *  ", Some(1..6))]
    #[case("___", Some(0..3))]
    #[case("--", None)]
    #[case("--a", None)]
    #[case("-*-", None)]
    #[case("    ---", None)]
    #[case("> ---", Some(2..5))]
    fn rules(#[case] line: &str, #[case] expected: Option<Range<usize>>) {
        let found = match block_line(line).kind {
            BlockKind::Rule(range) => Some(range),
            _ => None,
        };
        assert_eq!(found, expected);
    }

    #[test]
    fn quotes_nest() {
        assert_eq!(block_line("> a").quotes, [0]);
        assert_eq!(block_line(">> a").quotes, [0, 1]);
        assert_eq!(block_line(" > > a").quotes, [1, 3]);
        assert_eq!(block_line("    > a").quotes, Vec::<usize>::new());
        assert_eq!(block_line("a > b").quotes, Vec::<usize>::new());
    }
}
