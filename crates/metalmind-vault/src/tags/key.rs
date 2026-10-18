use std::fmt;

use unicode_normalization::UnicodeNormalization as _;

/// La identidad de un tag: su nombre en NFC y con el *case folding* de
/// Unicode, sin el `#`. `#Rust`, `#RUST` y `#rust` son la misma clave;
/// `#canción` y `#cancion`, no.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct TagKey(String);

impl TagKey {
    /// La clave de un nombre ya validado (sin `#`).
    #[must_use]
    pub fn new(name: &str) -> Self {
        // El folding puede dejar una cadena que ya no está en NFC.
        let folded = caseless::default_case_fold_str(&name.nfc().collect::<String>());
        Self(folded.nfc().collect())
    }

    /// La clave de lo que escribió el usuario (`#Rust` o `Rust`), si es un
    /// nombre de tag válido.
    #[must_use]
    pub fn parse(text: &str) -> Option<Self> {
        tag_name(text).map(Self::new)
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for TagKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// El nombre de `text` sin el `#` del principio, si es un nombre de tag válido.
#[must_use]
pub fn tag_name(text: &str) -> Option<&str> {
    let name = text.strip_prefix('#').unwrap_or(text);
    metalmind_syntax::is_valid_tag_name(name).then_some(name)
}

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use super::*;

    #[rstest]
    #[case::case("Rust", "rust", true)]
    #[case::upper("RUST", "rust", true)]
    #[case::sharp_s("STRASSE", "straße", true)]
    #[case::accents("canción", "cancion", false)]
    #[case::nfd("canci\u{F3}n", "cancio\u{301}n", true)]
    #[case::enye("año", "ano", false)]
    #[case::turkish_dotless("ı", "i", false)]
    #[case::greek_sigma("ΣΊΣΥΦΟΣ", "σίσυφος", true)]
    fn identity(#[case] a: &str, #[case] b: &str, #[case] same: bool) {
        assert_eq!(TagKey::new(a) == TagKey::new(b), same);
    }

    #[rstest]
    #[case("#Rust", Some("rust"))]
    #[case("Rust", Some("rust"))]
    #[case("##rust", None)]
    #[case("#2024/10", None)]
    #[case("#", None)]
    fn parse(#[case] text: &str, #[case] key: Option<&str>) {
        assert_eq!(TagKey::parse(text).as_ref().map(TagKey::as_str), key);
    }
}
