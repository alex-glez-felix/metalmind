//! La gramática de un ítem de lista: el marcador (`-`, `*`, `+`, `1.`, `1)`),
//! la casilla (`[ ]`, `[x]`) y dónde empieza su texto. El estilo la pinta y
//! el motor de vim la edita con estas mismas reglas.

use std::ops::Range;

use crate::style::{is_thematic_break, quote_prefix_len};

/// Como en `CommonMark`, un número de lista tiene a lo sumo nueve dígitos.
const MAX_DIGITS: usize = 9;
/// El número más grande que puede llevar un ítem numerado.
pub const MAX_LIST_NUMBER: u32 = 999_999_999;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ListMarker {
    /// `-`, `*` o `+`.
    Bullet(char),
    /// `12.` o `3)`.
    Number { value: u32, delimiter: char },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Checkbox {
    /// Los corchetes con lo de adentro.
    pub range: Range<usize>,
    /// Marcada con cualquier cosa que no sea un espacio, como en Obsidian.
    pub checked: bool,
}

impl Checkbox {
    /// Lo que hay entre `[` y `]`.
    #[must_use]
    pub fn inside(&self) -> Range<usize> {
        self.range.start + 1..self.range.end - 1
    }
}

/// Un ítem al inicio de una línea. Los rangos son bytes dentro de la línea.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ListItem {
    pub marker: ListMarker,
    /// El marcador: `-` o `12.`.
    pub marker_range: Range<usize>,
    /// `[ ]` o `[x]` tras el marcador y un blanco, y luego un blanco o nada.
    pub checkbox: Option<Checkbox>,
    /// Donde empieza el texto del ítem, tras el marcador, la casilla y sus
    /// blancos.
    pub content: usize,
}

impl ListItem {
    /// El ítem que empieza `line`, que no lleva su `\n`, tras la sangría y
    /// los `>` de una cita. El marcador y la casilla van seguidos de un
    /// blanco o del fin de la línea, con o sin su `\r`: `-x` no es un ítem,
    /// ni una regla como `- - -`.
    #[must_use]
    pub fn parse(line: &str) -> Option<Self> {
        if is_thematic_break(line) {
            return None;
        }
        Self::parse_at(line, quote_prefix_len(line))
    }

    /// Como [`ListItem::parse`], para quien ya sabe que no es una regla y
    /// dónde termina la cita (`from`).
    pub(crate) fn parse_at(line: &str, from: usize) -> Option<Self> {
        let bytes = line.as_bytes();
        let start = from + blanks(&line[from..]);
        let (marker, len) = marker(&line[start..])?;
        let end = start + len;
        if bytes.get(end).is_some_and(|&b| !is_blank(b) && b != b'\r') {
            return None;
        }
        let checkbox = checkbox(line, end + 1);
        let text = checkbox.as_ref().map_or(end, |checkbox| checkbox.range.end);
        Some(Self {
            marker,
            marker_range: start..end,
            checkbox,
            content: text + blanks(&line[text..]),
        })
    }
}

/// El marcador al principio de `text` y cuántos bytes ocupa.
pub(crate) fn marker(text: &str) -> Option<(ListMarker, usize)> {
    let digits = text.bytes().take_while(u8::is_ascii_digit).count();
    let next = text[digits..].chars().next()?;
    match next {
        '-' | '*' | '+' if digits == 0 => Some((ListMarker::Bullet(next), 1)),
        '.' | ')' if (1..=MAX_DIGITS).contains(&digits) => {
            let value = text[..digits].parse().ok()?;
            let marker = ListMarker::Number {
                value,
                delimiter: next,
            };
            Some((marker, digits + 1))
        }
        _ => None,
    }
}

fn checkbox(line: &str, at: usize) -> Option<Checkbox> {
    let rest = line.get(at..)?;
    let mut chars = rest.char_indices();
    let (_, open) = chars.next()?;
    let (_, mark) = chars.next()?;
    let (close_at, close) = chars.next()?;
    let end = at + close_at + 1;
    let after = line.as_bytes().get(end).copied();
    (open == '[' && close == ']' && mark != ']' && after.is_none_or(|b| is_blank(b) || b == b'\r'))
        .then_some(Checkbox {
            range: at..end,
            checked: mark != ' ',
        })
}

fn is_blank(b: u8) -> bool {
    b == b' ' || b == b'\t'
}

fn blanks(text: &str) -> usize {
    text.bytes().take_while(|&b| is_blank(b)).count()
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;
    use rstest::rstest;

    use super::*;

    /// El marcador, la casilla con si está marcada y el texto.
    type ItemCase = Option<(ListMarker, Option<(Range<usize>, bool)>, &'static str)>;

    const DASH: ListMarker = ListMarker::Bullet('-');

    #[rstest]
    #[case("- a", Some((DASH, None, "a")))]
    #[case("  * a", Some((ListMarker::Bullet('*'), None, "a")))]
    #[case("+\ta", Some((ListMarker::Bullet('+'), None, "a")))]
    #[case("12. a", Some((ListMarker::Number { value: 12, delimiter: '.' }, None, "a")))]
    #[case("3) a", Some((ListMarker::Number { value: 3, delimiter: ')' }, None, "a")))]
    #[case("999999999. a", Some((ListMarker::Number { value: MAX_LIST_NUMBER, delimiter: '.' }, None, "a")))]
    #[case("-", Some((DASH, None, "")))]
    #[case("- ", Some((DASH, None, "")))]
    #[case("- [ ] a", Some((DASH, Some((2..5, false)), "a")))]
    #[case("- [x]", Some((DASH, Some((2..5, true)), "")))]
    #[case("- [é]", Some((DASH, Some((2..6, true)), "")))]
    #[case("- [x]\r", Some((DASH, Some((2..5, true)), "\r")))]
    #[case("-  [ ] a", Some((DASH, None, "[ ] a")))]
    #[case("- [ab] a", Some((DASH, None, "[ab] a")))]
    #[case("- [x]a", Some((DASH, None, "[x]a")))]
    #[case("- []] a", Some((DASH, None, "[]] a")))]
    #[case("- [ ]a", Some((DASH, None, "[ ]a")))]
    #[case("- []", Some((DASH, None, "[]")))]
    #[case("> > - b", Some((DASH, None, "b")))]
    #[case("-a", None)]
    #[case("*a*", None)]
    #[case("--- ", None)]
    #[case("- - -", None)]
    #[case("> * * *", None)]
    #[case("1234567890. a", None)]
    #[case("a. b", None)]
    #[case("# a", None)]
    fn items(#[case] line: &str, #[case] expected: ItemCase) {
        let found = ListItem::parse(line).map(|item| {
            (
                item.marker,
                item.checkbox
                    .map(|checkbox| (checkbox.range, checkbox.checked)),
                &line[item.content..],
            )
        });
        assert_eq!(found, expected);
    }

    #[test]
    fn inside_the_checkbox() {
        let item = ListItem::parse("1. [x] a").unwrap();
        assert_eq!(item.checkbox.unwrap().inside(), 4..5);
    }
}
