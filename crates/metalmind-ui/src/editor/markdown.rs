//! De los tramos de estilo de una línea (`metalmind-syntax`) a lo que pinta
//! la vista, sin nada de GPUI: tramos seguidos que cubren toda la línea,
//! cuáles son links que no llevan a ninguna nota y cuáles son fórmulas que
//! no componen.

use std::ops::Range;

use metalmind_syntax::{LineStyle, LinkSpot, Role, Style};

/// Un tramo de la línea, en bytes, con su estilo.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Run {
    pub len: usize,
    pub style: Style,
    /// Es parte de un link a algo que no existe.
    pub unresolved: bool,
    /// Es parte de una fórmula que no compone.
    pub failed: bool,
}

/// Los tramos de una línea de `len` bytes, sin huecos. Sin estilo
/// calculado, todo es texto. `resolves` dice si un link lleva a algo, y
/// `failed` son los bytes de las fórmulas que no componen.
pub fn runs(
    len: usize,
    line: Option<&LineStyle>,
    resolves: impl Fn(&LinkSpot) -> bool,
    failed: &[Range<usize>],
) -> Vec<Run> {
    let plain = |len| Run {
        len,
        style: Style::default(),
        unresolved: false,
        failed: false,
    };
    let Some(line) = line else {
        return vec![plain(len)];
    };
    let mut out = Vec::with_capacity(line.spans.len() * 2 + 1);
    let mut at = 0;
    for span in &line.spans {
        let start = span.range.start.min(len);
        let end = span.range.end.min(len);
        if start > at {
            out.push(plain(start - at));
        }
        if end > start {
            let unresolved = is_link(span.style.role)
                && span
                    .link
                    .and_then(|link| line.links.get(link))
                    .is_some_and(|spot| !resolves(spot));
            out.push(Run {
                len: end - start,
                style: span.style,
                unresolved,
                failed: failed
                    .iter()
                    .any(|range| range.start <= start && end <= range.end),
            });
        }
        at = at.max(end);
    }
    if at < len {
        out.push(plain(len - at));
    }
    out
}

fn is_link(role: Role) -> bool {
    matches!(role, Role::Wikilink | Role::Link | Role::Embed)
}

#[cfg(test)]
mod tests {
    use metalmind_syntax::{Lines, Styles, TextLines};
    use pretty_assertions::assert_eq;

    use super::*;

    fn styled(text: &str) -> LineStyle {
        let lines = TextLines::new(text);
        let mut styles = Styles::new(lines.line_count());
        styles.ensure(0, &lines);
        styles.line(0).cloned().unwrap_or_default()
    }

    /// `[texto]rol` por tramo, con `!` si no resuelve.
    fn render(text: &str, resolves: impl Fn(&LinkSpot) -> bool) -> Vec<String> {
        let style = styled(text);
        let mut at = 0;
        runs(text.len(), Some(&style), resolves, &[])
            .into_iter()
            .map(|run| {
                let part = &text[at..at + run.len];
                at += run.len;
                let mark = if run.unresolved { "!" } else { "" };
                format!("[{part}]{:?}{mark}", run.style.role)
            })
            .collect()
    }

    #[test]
    fn covers_the_whole_line() {
        assert_eq!(
            render("a **b** c", |_| true),
            [
                "[a ]Text",
                "[**]Markup",
                "[b]Text",
                "[**]Markup",
                "[ c]Text"
            ]
        );
    }

    #[test]
    fn only_the_link_that_goes_nowhere_is_unresolved() {
        let found = render("[[hay]] y [[no]]", |spot| spot.target == "hay");
        assert_eq!(
            found,
            [
                "[[[]Markup",
                "[hay]Wikilink",
                "[]]]Markup",
                "[ y ]Text",
                "[[[]Markup",
                "[no]Wikilink!",
                "[]]]Markup",
            ]
        );
    }

    #[test]
    fn a_markdown_link_that_goes_nowhere_is_unresolved() {
        let found = render("[ver](no.md) y [ver](si.md)", |spot| spot.target == "si.md");
        assert_eq!(
            found,
            [
                "[[]Markup",
                "[ver]Link!",
                "[](no.md)]Markup",
                "[ y ]Text",
                "[[]Markup",
                "[ver]Link",
                "[](si.md)]Markup",
            ]
        );
    }

    #[test]
    fn a_formula_that_does_not_compose_is_marked_whole() {
        let text = r"a $\frac{$ b";
        let style = styled(text);
        let failed: Vec<bool> = runs(
            text.len(),
            Some(&style),
            |_| true,
            std::slice::from_ref(&(2..10)),
        )
        .iter()
        .map(|run| run.failed)
        .collect();
        assert_eq!(failed, [false, true, true, true, false]);
    }

    #[test]
    fn without_a_style_it_is_text() {
        assert_eq!(
            runs(3, None, |_| true, &[]),
            [Run {
                len: 3,
                style: Style::default(),
                unresolved: false,
                failed: false
            }]
        );
    }
}
