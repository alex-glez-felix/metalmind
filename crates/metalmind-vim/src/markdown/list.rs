//! Ítems de lista: `- `, `* `, `+ `, `1. ` o `1) ` al inicio de la línea,
//! con o sin checkbox (`- [ ] `).

use std::ops::Range;

use metalmind_buffer::{Buffer, Edit};

use crate::columns::Columns;
use crate::indent::leading_blanks;
use crate::lines::is_blank;

/// Como en la especificación de markdown, un número de lista tiene a lo
/// sumo nueve dígitos.
const MAX_DIGITS: usize = 9;
const MAX_NUMBER: u32 = 999_999_999;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Marker {
    Bullet(char),
    Number { value: u32, delimiter: char },
}

/// Un ítem al inicio de una línea. Los rangos son bytes dentro de la línea.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ListItem<'a> {
    line: &'a str,
    /// Hasta dónde llegan los `>` de una cita: `> - a` es un ítem.
    quote: usize,
    pub marker: Marker,
    /// El marcador: `-` o `12.`.
    marker_range: Range<usize>,
    /// Lo que hay entre `[` y `]`, si hay checkbox.
    pub checkbox: Option<Range<usize>>,
    /// Donde empieza el texto del ítem, tras el marcador y el checkbox.
    pub content: usize,
}

impl<'a> ListItem<'a> {
    /// El ítem que empieza `line`, que no lleva su `\n`, tras la sangría y
    /// los `>` de una cita. El marcador va seguido de un blanco: `-` solo o
    /// `-x` no son ítems.
    pub(crate) fn parse(line: &'a str) -> Option<Self> {
        if metalmind_syntax::is_thematic_break(line) {
            return None;
        }
        let quote = metalmind_syntax::quote_prefix_len(line);
        let start = indent_of(line).len();
        let rest = &line[start..];
        let (marker, len) = marker(rest)?;
        let after = start + len;
        let spacing = leading_blanks(&line[after..]).len();
        if spacing == 0 {
            return None;
        }
        let mut item = Self {
            line,
            quote,
            marker,
            marker_range: start..after,
            checkbox: None,
            content: after + spacing,
        };
        item.read_checkbox();
        Some(item)
    }

    /// `[ ]`, `[x]` o cualquier otro carácter entre corchetes, seguido de un
    /// blanco o del fin de línea.
    fn read_checkbox(&mut self) {
        let rest = &self.line[self.content..];
        let mut chars = rest.char_indices();
        let (Some((_, '[')), Some((inside, _)), Some((close, ']'))) =
            (chars.next(), chars.next(), chars.next())
        else {
            return;
        };
        let after = close + 1;
        if !rest[after..].chars().next().is_none_or(is_blank) {
            return;
        }
        let spacing = leading_blanks(&rest[after..]).len();
        self.checkbox = Some(self.content + inside..self.content + close);
        self.content += after + spacing;
    }

    /// Los bytes de la cita que lo contiene: terminar la lista los deja.
    pub(crate) fn quote(&self) -> usize {
        self.quote
    }

    /// Lo que va antes del marcador: la cita y la sangría.
    pub(crate) fn indent(&self) -> &'a str {
        &self.line[..self.marker_range.start]
    }

    pub(crate) fn is_checked(&self) -> bool {
        self.checkbox
            .as_ref()
            .is_some_and(|inside| !self.line[inside.clone()].chars().all(is_blank))
    }

    /// Si el ítem no tiene texto, tras el marcador y el checkbox.
    pub(crate) fn is_empty(&self) -> bool {
        self.line[self.content..].chars().all(is_blank)
    }

    /// El inicio de otro ítem de la misma lista, con `marker`: la misma
    /// indentación y los mismos blancos, y un checkbox sin marcar si este
    /// tiene uno.
    pub(crate) fn sibling(&self, marker: Marker) -> String {
        let mut text = self.indent().to_owned();
        match marker {
            Marker::Bullet(bullet) => text.push(bullet),
            Marker::Number { value, delimiter } => {
                text.push_str(&value.to_string());
                text.push(delimiter);
            }
        }
        let spacing = match &self.checkbox {
            Some(inside) => {
                let box_start = inside.start - 1;
                let after_box = &self.line[inside.end + 1..self.content];
                let after_box = if after_box.is_empty() { " " } else { after_box };
                format!(
                    "{}[ ]{after_box}",
                    &self.line[self.marker_range.end..box_start]
                )
            }
            None => self.line[self.marker_range.end..self.content].to_owned(),
        };
        text.push_str(&spacing);
        text
    }

    /// El marcador que sigue a este en la lista: el mismo, o el número
    /// siguiente; `None` tras el último que cabe en nueve dígitos.
    pub(crate) fn next_marker(&self) -> Option<Marker> {
        match self.marker {
            Marker::Bullet(_) => Some(self.marker),
            Marker::Number { value, delimiter } => Some(Marker::Number {
                value: following(value)?,
                delimiter,
            }),
        }
    }

    fn number(&self) -> Option<(u32, char)> {
        match self.marker {
            Marker::Number { value, delimiter } => Some((value, delimiter)),
            Marker::Bullet(_) => None,
        }
    }
}

/// La cita y la sangría al inicio de `line`, de las que sale el nivel de un
/// ítem.
pub(crate) fn indent_of(line: &str) -> &str {
    let quote = metalmind_syntax::quote_prefix_len(line);
    &line[..quote + leading_blanks(&line[quote..]).len()]
}

fn marker(text: &str) -> Option<(Marker, usize)> {
    let digits = text.bytes().take_while(u8::is_ascii_digit).count();
    let next = text[digits..].chars().next()?;
    match next {
        '-' | '*' | '+' if digits == 0 => Some((Marker::Bullet(next), 1)),
        '.' | ')' if (1..=MAX_DIGITS).contains(&digits) => {
            let value = text[..digits].parse().ok()?;
            let marker = Marker::Number {
                value,
                delimiter: next,
            };
            Some((marker, digits + 1))
        }
        _ => None,
    }
}

/// Dónde sigue una lista numerada: el número que le toca al siguiente
/// ítem, su delimitador y la indentación de sus hermanos.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Numbering {
    pub value: u32,
    delimiter: char,
    level: usize,
}

impl ListItem<'_> {
    /// La numeración de este ítem, si es numerado.
    pub(crate) fn numbering(&self, columns: Columns) -> Option<Numbering> {
        let (value, delimiter) = self.number()?;
        Some(Numbering {
            value,
            delimiter,
            level: columns.line_width(self.indent()),
        })
    }
}

impl Numbering {
    pub(crate) fn next(self) -> Option<Self> {
        Some(Self {
            value: following(self.value)?,
            ..self
        })
    }
}

/// El número que sigue a `value`, si todavía es un número de lista.
fn following(value: u32) -> Option<u32> {
    value.checked_add(1).filter(|&next| next <= MAX_NUMBER)
}

/// Las ediciones que numeran desde `numbering` los ítems que siguen a
/// `line`, en su mismo nivel, hasta donde termina la lista: una línea en
/// blanco, una menos indentada o una que no es un ítem numerado con el
/// mismo delimitador. Los hijos, más indentados, se saltan. Devuelve también
/// las líneas que recorrió como hermanas.
pub(crate) fn renumber(
    buffer: &Buffer,
    line: usize,
    numbering: Numbering,
    columns: Columns,
) -> (Vec<Edit>, Vec<usize>) {
    let mut value = numbering.value;
    let mut edits = Vec::new();
    let mut siblings = Vec::new();
    for next in line + 1..buffer.line_count() {
        let text = buffer.line_text(next);
        if value > MAX_NUMBER || text.chars().all(is_blank) {
            break;
        }
        let level = columns.line_width(indent_of(&text));
        if level > numbering.level {
            continue;
        }
        let item = ListItem::parse(&text).filter(|_| level == numbering.level);
        let Some((current, digits)) = item.and_then(|item| {
            let (current, delimiter) = item.number()?;
            let digits = item.marker_range.start..item.marker_range.end - 1;
            (delimiter == numbering.delimiter).then_some((current, digits))
        }) else {
            break;
        };
        siblings.push(next);
        if current != value {
            let start = buffer.line_start(next);
            edits.push(Edit::replace(
                start + digits.start..start + digits.end,
                value.to_string(),
            ));
        }
        value += 1;
    }
    (edits, siblings)
}

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use super::*;

    #[rstest]
    #[case("- a", Some(Marker::Bullet('-')), "a")]
    #[case("  * a", Some(Marker::Bullet('*')), "a")]
    #[case("+\ta", Some(Marker::Bullet('+')), "a")]
    #[case("12. a", Some(Marker::Number { value: 12, delimiter: '.' }), "a")]
    #[case("3) a", Some(Marker::Number { value: 3, delimiter: ')' }), "a")]
    #[case("- ", Some(Marker::Bullet('-')), "")]
    #[case("- [ ] a", Some(Marker::Bullet('-')), "a")]
    #[case("- [x]", Some(Marker::Bullet('-')), "")]
    #[case("- [ab] a", Some(Marker::Bullet('-')), "[ab] a")]
    #[case("- [x]a", Some(Marker::Bullet('-')), "[x]a")]
    #[case("-", None, "")]
    #[case("-a", None, "")]
    #[case("--- ", None, "")]
    #[case("- - -", None, "")]
    #[case("* * *", None, "")]
    #[case("1234567890. a", None, "")]
    #[case("a. b", None, "")]
    #[case("# a", None, "")]
    fn items(#[case] line: &str, #[case] marker: Option<Marker>, #[case] content: &str) {
        let item = ListItem::parse(line);
        assert_eq!(item.as_ref().map(|item| item.marker), marker);
        if let Some(item) = item {
            assert_eq!(&line[item.content..], content);
        }
    }

    #[rstest]
    #[case("- a", "- ")]
    #[case("  *   a", "  *   ")]
    #[case("9. a", "10. ")]
    #[case("1) a", "2) ")]
    #[case("- [x] a", "- [ ] ")]
    #[case("- [ ]", "- [ ] ")]
    #[case("1.  [X]  a", "2.  [ ]  ")]
    fn next_sibling(#[case] line: &str, #[case] sibling: &str) {
        let item = ListItem::parse(line).unwrap();
        assert_eq!(item.sibling(item.next_marker().unwrap()), sibling);
    }

    #[rstest]
    #[case("- [ ] a", false)]
    #[case("- [x] a", true)]
    #[case("- [X] a", true)]
    #[case("- [-] a", true)]
    #[case("- a", false)]
    fn checked(#[case] line: &str, #[case] expected: bool) {
        assert_eq!(ListItem::parse(line).unwrap().is_checked(), expected);
    }

    #[rstest]
    #[case("1. a\n1. b\n1. c", 0, "1. a\n2. b\n3. c")]
    #[case("1. a\n2. b\n   - x\n2. c", 1, "1. a\n2. b\n   - x\n3. c")]
    #[case("1. a\n2. b\n\n2. c", 0, "1. a\n2. b\n\n2. c")]
    #[case("1. a\n2. b\n2) c", 0, "1. a\n2. b\n2) c")]
    #[case("1. a\n  1. b\n2. c", 0, "1. a\n  1. b\n2. c")]
    #[case("1. a\n- b\n2. c", 0, "1. a\n- b\n2. c")]
    #[case("- a\n- b", 0, "- a\n- b")]
    fn renumbers_the_rest_of_the_list(
        #[case] before: &str,
        #[case] line: usize,
        #[case] after: &str,
    ) {
        let mut buffer = Buffer::from(before);
        let text = buffer.line_text(line).into_owned();
        let numbering = ListItem::parse(&text)
            .and_then(|item| item.numbering(Columns::default()))
            .map_or(Vec::new(), |numbering| {
                renumber(&buffer, line, numbering.next().unwrap(), Columns::default()).0
            });
        let edits = numbering;
        buffer
            .begin_transaction(metalmind_buffer::Selections::single(
                metalmind_buffer::Selection::cursor(metalmind_buffer::ByteOffset::ZERO),
            ))
            .unwrap();
        buffer.apply(edits).unwrap();
        assert_eq!(buffer.to_string(), after);
    }
}
