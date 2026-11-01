//! Editar ítems de lista: continuarlos, terminarlos y renumerarlos. Qué es
//! un ítem lo dice [`metalmind_syntax::ListItem`], igual que al pintarlo.

use std::ops::Range;

use metalmind_buffer::{Buffer, Edit};
use metalmind_syntax::{ListMarker, MAX_LIST_NUMBER};

use crate::columns::Columns;
use crate::indent::leading_blanks;
use crate::lines::is_blank;

/// Un ítem al inicio de una línea, con la línea para editarlo.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ListItem<'a> {
    line: &'a str,
    item: metalmind_syntax::ListItem,
}

impl<'a> ListItem<'a> {
    /// El ítem que empieza `line`, que no lleva su `\n`. Un marcador sin
    /// blanco detrás se pinta, pero todavía se está escribiendo: no se edita.
    pub(crate) fn parse(line: &'a str) -> Option<Self> {
        let item = metalmind_syntax::ListItem::parse(line)?;
        (item.content > item.marker_range.end).then_some(Self { line, item })
    }

    pub(crate) fn marker(&self) -> ListMarker {
        self.item.marker
    }

    /// Lo que hay entre `[` y `]`, si hay checkbox.
    pub(crate) fn checkbox(&self) -> Option<Range<usize>> {
        self.item
            .checkbox
            .as_ref()
            .map(metalmind_syntax::Checkbox::inside)
    }

    /// Donde empieza el texto del ítem, tras el marcador y el checkbox.
    pub(crate) fn content(&self) -> usize {
        self.item.content
    }

    /// Los bytes de la cita que lo contiene: terminar la lista los deja.
    pub(crate) fn quote(&self) -> usize {
        metalmind_syntax::quote_prefix_len(self.line)
    }

    /// Lo que va antes del marcador: la cita y la sangría.
    pub(crate) fn indent(&self) -> &'a str {
        &self.line[..self.item.marker_range.start]
    }

    pub(crate) fn is_checked(&self) -> bool {
        self.item
            .checkbox
            .as_ref()
            .is_some_and(|checkbox| checkbox.checked)
    }

    /// Si el ítem no tiene texto, tras el marcador y el checkbox.
    pub(crate) fn is_empty(&self) -> bool {
        self.line[self.item.content..].chars().all(is_blank)
    }

    /// El inicio de otro ítem de la misma lista, con `marker`: la misma
    /// indentación y los mismos blancos, y un checkbox sin marcar si este
    /// tiene uno.
    pub(crate) fn sibling(&self, marker: ListMarker) -> String {
        let mut text = self.indent().to_owned();
        match marker {
            ListMarker::Bullet(bullet) => text.push(bullet),
            ListMarker::Number { value, delimiter } => {
                text.push_str(&value.to_string());
                text.push(delimiter);
            }
        }
        let marker_end = self.item.marker_range.end;
        let spacing = match &self.item.checkbox {
            Some(checkbox) => {
                let after_box = &self.line[checkbox.range.end..self.item.content];
                let after_box = if after_box.is_empty() { " " } else { after_box };
                format!(
                    "{}[ ]{after_box}",
                    &self.line[marker_end..checkbox.range.start]
                )
            }
            None => self.line[marker_end..self.item.content].to_owned(),
        };
        text.push_str(&spacing);
        text
    }

    /// El marcador que sigue a este en la lista: el mismo, o el número
    /// siguiente; `None` tras el último que cabe en nueve dígitos.
    pub(crate) fn next_marker(&self) -> Option<ListMarker> {
        match self.item.marker {
            ListMarker::Bullet(_) => Some(self.item.marker),
            ListMarker::Number { value, delimiter } => Some(ListMarker::Number {
                value: following(value)?,
                delimiter,
            }),
        }
    }

    fn number(&self) -> Option<(u32, char)> {
        match self.item.marker {
            ListMarker::Number { value, delimiter } => Some((value, delimiter)),
            ListMarker::Bullet(_) => None,
        }
    }

    /// Los dígitos del número, sin su delimitador.
    fn digits(&self) -> Range<usize> {
        self.item.marker_range.start..self.item.marker_range.end - 1
    }
}

/// La cita y la sangría al inicio de `line`, de las que sale el nivel de un
/// ítem.
pub(crate) fn indent_of(line: &str) -> &str {
    let quote = metalmind_syntax::quote_prefix_len(line);
    &line[..quote + leading_blanks(&line[quote..]).len()]
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
    value.checked_add(1).filter(|&next| next <= MAX_LIST_NUMBER)
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
        if value > MAX_LIST_NUMBER || text.chars().all(is_blank) {
            break;
        }
        let level = columns.line_width(indent_of(&text));
        if level > numbering.level {
            continue;
        }
        let item = ListItem::parse(&text).filter(|_| level == numbering.level);
        let Some((current, digits)) = item.and_then(|item| {
            let (current, delimiter) = item.number()?;
            (delimiter == numbering.delimiter).then_some((current, item.digits()))
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
    #[case("- a", Some(ListMarker::Bullet('-')), "a")]
    #[case("  * a", Some(ListMarker::Bullet('*')), "a")]
    #[case("+\ta", Some(ListMarker::Bullet('+')), "a")]
    #[case("12. a", Some(ListMarker::Number { value: 12, delimiter: '.' }), "a")]
    #[case("3) a", Some(ListMarker::Number { value: 3, delimiter: ')' }), "a")]
    #[case("- ", Some(ListMarker::Bullet('-')), "")]
    #[case("- [ ] a", Some(ListMarker::Bullet('-')), "a")]
    #[case("- [x]", Some(ListMarker::Bullet('-')), "")]
    #[case("- [ab] a", Some(ListMarker::Bullet('-')), "[ab] a")]
    #[case("- [x]a", Some(ListMarker::Bullet('-')), "[x]a")]
    #[case("-", None, "")]
    #[case("-a", None, "")]
    #[case("--- ", None, "")]
    #[case("- - -", None, "")]
    #[case("* * *", None, "")]
    #[case("1234567890. a", None, "")]
    #[case("a. b", None, "")]
    #[case("# a", None, "")]
    fn items(#[case] line: &str, #[case] marker: Option<ListMarker>, #[case] content: &str) {
        let item = ListItem::parse(line);
        assert_eq!(item.as_ref().map(ListItem::marker), marker);
        if let Some(item) = item {
            assert_eq!(&line[item.content()..], content);
        }
    }

    #[rstest]
    #[case("- a", "- ")]
    #[case("  *   a", "  *   ")]
    #[case("9. a", "10. ")]
    #[case("1) a", "2) ")]
    #[case("- [x] a", "- [ ] ")]
    #[case("- [ ]", "- [ ] ")]
    #[case("1. [X]  a", "2. [ ]  ")]
    #[case("1.  [X]  a", "2.  ")]
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
