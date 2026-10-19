//! Lo que se ve al principio de una línea: los `>` de una cita, el marcador
//! de un ítem con su casilla y las reglas horizontales. Se mira línea a
//! línea, sin el árbol de bloques de `CommonMark`: una línea sin `>` tras una
//! cita no es parte de ella.

use std::ops::Range;

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
    Item(Item),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Item {
    pub marker: Range<usize>,
    /// `-`, `*` o `+`, no `1.`.
    pub bullet: bool,
    pub task: Option<Task>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Task {
    /// Los corchetes con lo de adentro.
    pub range: Range<usize>,
    /// Marcada con cualquier cosa que no sea un espacio, como en Obsidian.
    pub checked: bool,
}

fn is_space(b: u8) -> bool {
    b == b' ' || b == b'\t'
}

pub(crate) fn block_line(line: &str) -> BlockLine {
    let (quotes, at) = quote_marks(line);
    let kind = rule(line, at)
        .map(BlockKind::Rule)
        .or_else(|| item(line, at).map(BlockKind::Item))
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

fn item(line: &str, from: usize) -> Option<Item> {
    let bytes = line.as_bytes();
    let start = from + bytes[from..].iter().take_while(|&&b| is_space(b)).count();
    let digits = bytes[start..]
        .iter()
        .take_while(|b| b.is_ascii_digit())
        .count();
    let (len, bullet) = match bytes.get(start + digits) {
        Some(b'.' | b')') if (1..=9).contains(&digits) => (digits + 1, false),
        Some(b'-' | b'*' | b'+') if digits == 0 => (1, true),
        _ => return None,
    };
    let end = start + len;
    let after = bytes.get(end).copied();
    if after.is_some_and(|b| !is_space(b) && b != b'\r') {
        return None;
    }
    Some(Item {
        marker: start..end,
        bullet,
        task: task(line, end + 1),
    })
}

/// `[ ]` o `[x]` tras el marcador y un espacio, y luego un espacio o nada.
fn task(line: &str, at: usize) -> Option<Task> {
    let rest = line.get(at..)?;
    let mut chars = rest.char_indices();
    let (_, open) = chars.next()?;
    let (_, mark) = chars.next()?;
    let (close_at, close) = chars.next()?;
    let end = at + close_at + 1;
    let after = line.as_bytes().get(end).copied();
    (open == '[' && close == ']' && mark != ']' && after.is_none_or(|b| is_space(b) || b == b'\r'))
        .then_some(Task {
            range: at..end,
            checked: mark != ' ',
        })
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

    /// El marcador, si es viñeta y la casilla con si está marcada.
    type ItemCase = Option<(Range<usize>, bool, Option<(Range<usize>, bool)>)>;

    #[rstest]
    #[case("- a", Some((0..1, true, None)))]
    #[case("  * a", Some((2..3, true, None)))]
    #[case("12. a", Some((0..3, false, None)))]
    #[case("3) a", Some((0..2, false, None)))]
    #[case("-", Some((0..1, true, None)))]
    #[case("- [ ] a", Some((0..1, true, Some((2..5, false)))))]
    #[case("- [x] a", Some((0..1, true, Some((2..5, true)))))]
    #[case("- [é]", Some((0..1, true, Some((2..6, true)))))]
    #[case("- [ ]a", Some((0..1, true, None)))]
    #[case("- []", Some((0..1, true, None)))]
    #[case("-a", None)]
    #[case("*a*", None)]
    #[case("1234567890. a", None)]
    #[case("> > - b", Some((4..5, true, None)))]
    fn items(#[case] line: &str, #[case] expected: ItemCase) {
        let found = match block_line(line).kind {
            BlockKind::Item(item) => Some((
                item.marker,
                item.bullet,
                item.task.map(|task| (task.range, task.checked)),
            )),
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
