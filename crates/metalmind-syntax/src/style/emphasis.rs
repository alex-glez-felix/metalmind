//! Negrita, cursiva, tachado y resaltado: las rachas de `*`, `_`, `~~` y
//! `==` y cómo se emparejan, con las reglas de flanqueo y de "múltiplo de
//! tres" de `CommonMark` (`process_emphasis` de cmark). `~~` y `==` son de a
//! dos, como en Obsidian.

use std::ops::Range;

use unicode_properties::{GeneralCategoryGroup, UnicodeGeneralCategory};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Kind {
    Star,
    Underscore,
    Tilde,
    Equals,
}

impl Kind {
    fn of(byte: u8) -> Option<Self> {
        match byte {
            b'*' => Some(Self::Star),
            b'_' => Some(Self::Underscore),
            b'~' => Some(Self::Tilde),
            b'=' => Some(Self::Equals),
            _ => None,
        }
    }

    fn index(self) -> usize {
        self as usize
    }

    /// `~~` y `==` solo valen de a dos.
    fn paired(self) -> bool {
        matches!(self, Self::Tilde | Self::Equals)
    }
}

/// Lo que formó una pareja: el estilo, el texto de adentro y los dos
/// marcadores.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Emphasis {
    pub style: Styled,
    pub inner: Range<usize>,
    pub open: Range<usize>,
    pub close: Range<usize>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Styled {
    Strong,
    Emphasis,
    Strike,
    Mark,
}

/// Una racha de delimitadores.
#[derive(Debug, Clone)]
struct Run {
    kind: Kind,
    /// Dónde empiezan los que quedan sin usar.
    start: usize,
    len: usize,
    original: usize,
    can_open: bool,
    can_close: bool,
}

fn is_punctuation(c: char) -> bool {
    c.is_ascii_punctuation()
        || matches!(
            c.general_category_group(),
            GeneralCategoryGroup::Punctuation | GeneralCategoryGroup::Symbol
        )
}

/// Las rachas de un bloque, para emparejar.
pub(crate) struct Delimiters {
    runs: Vec<Run>,
}

impl Delimiters {
    pub(crate) fn new() -> Self {
        Self { runs: Vec::new() }
    }

    /// Si en `at` empieza una racha, la anota y devuelve dónde termina. La
    /// racha no pasa de `limit` (lo que sigue es código, un link…), pero
    /// para flanquear se mira el carácter de verdad: `before`, el anterior,
    /// y el siguiente de `text`. `None` cuenta como espacio.
    pub(crate) fn scan(
        &mut self,
        text: &str,
        at: usize,
        before: Option<char>,
        limit: usize,
    ) -> Option<usize> {
        let byte = *text.as_bytes().get(at)?;
        let kind = Kind::of(byte)?;
        let len = text.as_bytes()[at..limit]
            .iter()
            .take_while(|&&b| b == byte)
            .count();
        let end = at + len;
        if kind.paired() && len != 2 {
            return Some(end);
        }
        let after = text[end..].chars().next();
        let space = |c: Option<char>| c.is_none_or(char::is_whitespace);
        let punct = |c: Option<char>| c.is_some_and(is_punctuation);
        let left = !space(after) && (!punct(after) || space(before) || punct(before));
        let right = !space(before) && (!punct(before) || space(after) || punct(after));
        let (can_open, can_close) = if kind == Kind::Underscore {
            (
                left && (!right || punct(before)),
                right && (!left || punct(after)),
            )
        } else {
            (left, right)
        };
        if can_open || can_close {
            self.runs.push(Run {
                kind,
                start: at,
                len,
                original: len,
                can_open,
                can_close,
            });
        }
        Some(end)
    }

    /// Empareja las rachas como `process_emphasis` de cmark: cada cierre
    /// busca hacia atrás la apertura más cercana que le sirva, y lo que
    /// queda entre las dos ya no empareja con nada de afuera.
    pub(crate) fn pair(mut self) -> Vec<Emphasis> {
        let mut found = Vec::new();
        // Hasta dónde vale buscar aperturas, por tipo, por si el cierre
        // también abre y por su largo módulo 3, como `openers_bottom`.
        let mut bottom = [[0usize; 6]; 4];
        let mut closer = 0;
        while closer < self.runs.len() {
            let run = &self.runs[closer];
            if !run.can_close || run.len == 0 {
                closer += 1;
                continue;
            }
            let slot = usize::from(run.can_open) * 3 + run.original % 3;
            let floor = bottom[run.kind.index()][slot];
            if let Some(opener) = self.opener(closer, floor) {
                found.push(self.join(opener, closer));
                for between in &mut self.runs[opener + 1..closer] {
                    between.len = 0;
                }
                if self.runs[closer].len == 0 {
                    closer += 1;
                }
            } else {
                bottom[run.kind.index()][slot] = closer;
                closer += 1;
            }
        }
        found
    }

    fn opener(&self, closer: usize, floor: usize) -> Option<usize> {
        let close = &self.runs[closer];
        (floor..closer).rev().find(|&at| {
            let open = &self.runs[at];
            open.kind == close.kind
                && open.can_open
                && open.len > 0
                && !((open.can_close || close.can_open)
                    && (open.original + close.original).is_multiple_of(3)
                    && !(open.original.is_multiple_of(3) && close.original.is_multiple_of(3)))
        })
    }

    fn join(&mut self, opener: usize, closer: usize) -> Emphasis {
        let kind = self.runs[closer].kind;
        let both = self.runs[opener].len.min(self.runs[closer].len);
        let used = if kind.paired() || both >= 2 { 2 } else { 1 };
        let open = &mut self.runs[opener];
        open.len -= used;
        let open_end = open.start + open.len + used;
        let open_range = open_end - used..open_end;
        let close = &mut self.runs[closer];
        let close_range = close.start..close.start + used;
        close.start += used;
        close.len -= used;
        let style = match kind {
            Kind::Tilde => Styled::Strike,
            Kind::Equals => Styled::Mark,
            Kind::Star | Kind::Underscore if used == 2 => Styled::Strong,
            Kind::Star | Kind::Underscore => Styled::Emphasis,
        };
        Emphasis {
            style,
            inner: open_range.end..close_range.start,
            open: open_range,
            close: close_range,
        }
    }
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;
    use rstest::rstest;

    use super::*;

    /// Lo que queda con estilo, en el formato `estilo:texto`.
    fn styled(text: &str) -> Vec<String> {
        let mut delimiters = Delimiters::new();
        let mut at = 0;
        while at < text.len() {
            let before = text[..at].chars().next_back();
            at = delimiters
                .scan(text, at, before, text.len())
                .unwrap_or_else(|| at + text[at..].chars().next().map_or(1, char::len_utf8));
        }
        let mut found: Vec<String> = delimiters
            .pair()
            .into_iter()
            .map(|e| format!("{:?}:{}", e.style, &text[e.inner]))
            .collect();
        found.sort();
        found
    }

    #[rstest]
    #[case("*a*", &["Emphasis:a"])]
    #[case("**a**", &["Strong:a"])]
    #[case("***a***", &["Emphasis:**a**", "Strong:a"])]
    #[case("_a_", &["Emphasis:a"])]
    #[case("snake_case_name", &[])]
    #[case("a * b * c", &[])]
    #[case("**a *b* c**", &["Emphasis:b", "Strong:a *b* c"])]
    #[case("*a **b***", &["Emphasis:a **b**", "Strong:b"])]
    #[case("~~a~~", &["Strike:a"])]
    #[case("~a~", &[])]
    #[case("~~~a~~~", &[])]
    #[case("==a==", &["Mark:a"])]
    #[case("a == b == c", &[])]
    #[case("**a", &[])]
    #[case("*a**", &["Emphasis:a"])]
    #[case("*(a)*", &["Emphasis:(a)"])]
    #[case("_(a)_x", &[])]
    #[case("**ñ**", &["Strong:ñ"])]
    fn pairs_like_commonmark(#[case] text: &str, #[case] expected: &[&str]) {
        assert_eq!(styled(text), expected);
    }
}
