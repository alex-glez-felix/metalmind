use std::ops::Range;

/// Un `#tag` en el cuerpo de la nota.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tag {
    /// Desde el `#` hasta el último carácter del tag.
    pub span: Range<usize>,
    /// El nombre, sin el `#`; los anidados conservan sus `/`.
    pub name: Range<usize>,
}

impl Tag {
    #[must_use]
    pub fn name<'t>(&self, text: &'t str) -> &'t str {
        &text[self.name.clone()]
    }
}

/// Puntuación fuera de ASCII que corta un tag, como el espacio. El resto de
/// lo que no es ASCII (letras, emoji, sus modificadores y el ZWJ) sí cabe.
fn is_unicode_punctuation(c: char) -> bool {
    matches!(c,
        '\u{00A1}'..='\u{00BF}'
        | '\u{2000}'..='\u{200C}'
        | '\u{200E}'..='\u{206F}'
        | '\u{3000}'..='\u{303F}'
        | '\u{FF01}'..='\u{FF0F}'
        | '\u{FF1A}'..='\u{FF20}'
        | '\u{FF3B}'..='\u{FF40}'
        | '\u{FF5B}'..='\u{FF65}')
}

/// Lo que puede ir en un tag: letras, números, `_`, `-`, `/`
/// para anidar y casi cualquier cosa fuera de ASCII (emoji incluidos).
fn is_tag_char(c: char) -> bool {
    if c.is_ascii() {
        c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '/')
    } else {
        !c.is_whitespace() && !is_unicode_punctuation(c)
    }
}

/// Un nombre de tag válido: solo caracteres de tag y no solo números.
#[must_use]
pub fn is_valid_tag_name(name: &str) -> bool {
    !name.is_empty() && name.chars().all(is_tag_char) && !name.chars().all(char::is_numeric)
}

/// El tag cuyo `#` está en `at`, si lo hay. Quien llama ya comprobó que el
/// `#` va al inicio de línea o tras un espacio.
pub(crate) fn tag_at(text: &str, at: usize) -> Option<Tag> {
    let start = at + 1;
    let len: usize = text[start..]
        .chars()
        .take_while(|&c| is_tag_char(c))
        .map(char::len_utf8)
        .sum();
    let name = start..start + len;
    is_valid_tag_name(&text[name.clone()]).then_some(Tag {
        span: at..name.end,
        name,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    #[rstest]
    #[case("tag", true)]
    #[case("a/b/c", true)]
    #[case("y1984", true)]
    #[case("1984", false)]
    #[case("2024/10", true)]
    #[case("１２", false)]
    #[case("", false)]
    #[case("con espacio", false)]
    #[case("canción", true)]
    #[case("🔥", true)]
    #[case("a.b", false)]
    fn valid_names(#[case] name: &str, #[case] valid: bool) {
        assert_eq!(is_valid_tag_name(name), valid);
    }
}
