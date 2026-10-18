use std::ops::Range;

use unicode_properties::{GeneralCategoryGroup, UnicodeGeneralCategory};

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

/// Lo que puede ir en un tag: letras, números, `_`, `-`, `/` para anidar y,
/// fuera de ASCII, letras, marcas, números y símbolos (emoji incluidos). La
/// puntuación y los espacios de Unicode lo cortan; de los caracteres de
/// formato solo caben el ZWJ y los de etiqueta de las banderas de emoji.
fn is_tag_char(c: char) -> bool {
    if c.is_ascii() {
        return c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '/');
    }
    match c.general_category_group() {
        GeneralCategoryGroup::Letter
        | GeneralCategoryGroup::Mark
        | GeneralCategoryGroup::Number
        | GeneralCategoryGroup::Symbol => true,
        GeneralCategoryGroup::Punctuation | GeneralCategoryGroup::Separator => false,
        GeneralCategoryGroup::Other => matches!(c, '\u{200D}' | '\u{E0020}'..='\u{E007F}'),
    }
}

/// Un nombre de tag válido: solo caracteres de tag, no solo separadores
/// (`-`, `_`, `/`) y sin niveles vacíos (`a//b`, `/a`). Los números solos sí
/// valen: `#2024` agrupa un año.
#[must_use]
pub fn is_valid_tag_name(name: &str) -> bool {
    name.chars().all(is_tag_char)
        && name.chars().any(|c| !matches!(c, '-' | '_' | '/'))
        && name.split('/').all(|level| !level.is_empty())
}

/// El tag cuyo `#` está en `at`, si lo hay. Quien llama ya comprobó que el
/// `#` va al inicio de línea o tras un espacio. Un `/` final no es parte del
/// tag: `#proyecto/` es `#proyecto`.
pub(crate) fn tag_at(text: &str, at: usize) -> Option<Tag> {
    let start = at + 1;
    let run: usize = text[start..]
        .chars()
        .take_while(|&c| is_tag_char(c))
        .map(char::len_utf8)
        .sum();
    let name = start..start + text[start..start + run].trim_end_matches('/').len();
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
    #[case("1984", true)]
    #[case("1", true)]
    #[case("2024/10", true)]
    #[case("１２", true)]
    #[case("", false)]
    #[case("con espacio", false)]
    #[case("canción", true)]
    #[case("🔥", true)]
    #[case("a.b", false)]
    #[case("-", false)]
    #[case("_", false)]
    #[case("--", false)]
    #[case("/", false)]
    #[case("a//b", false)]
    #[case("/a", false)]
    #[case("a/", false)]
    #[case("a-", true)]
    #[case("2024-10", true)]
    #[case("xª", true)]
    #[case("x々", true)]
    #[case("x،", false)]
    #[case("x।", false)]
    #[case("x\u{200B}", false)]
    fn valid_names(#[case] name: &str, #[case] valid: bool) {
        assert_eq!(is_valid_tag_name(name), valid);
    }
}
