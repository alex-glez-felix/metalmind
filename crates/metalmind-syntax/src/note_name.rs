//! Qué archivo es una nota: el que se llama `<nombre>.md`, con la extensión
//! en cualquier mezcla de mayúsculas y un nombre no vacío. `.md` a secas es
//! un archivo oculto, no una nota sin nombre. Toda pregunta sobre si una ruta
//! es una nota, o cómo se llama sin su extensión, se contesta aquí.
//!
//! Las rutas van relativas al vault y separadas por `/`; solo cuenta el
//! último componente.

/// La extensión de una nota, con su punto, como se escribe al crear una.
pub const EXTENSION: &str = ".md";

/// `path` sin su `.md` si es una nota (`a/Casa.MD` da `a/Casa`).
#[must_use]
pub fn strip_extension(path: &str) -> Option<&str> {
    let at = path.len().checked_sub(EXTENSION.len())?;
    let stem = path
        .get(at..)
        .filter(|extension| extension.eq_ignore_ascii_case(EXTENSION))
        .map(|_| &path[..at])?;
    let named = !stem.is_empty() && !stem.ends_with('/');
    named.then_some(stem)
}

#[must_use]
pub fn is_note(path: &str) -> bool {
    strip_extension(path).is_some()
}

/// El archivo de la nota que se llama `name`: siempre le agrega `.md`, así
/// que `x.md` da `x.md.md`, la nota que se llama `x.md`.
#[must_use]
pub fn with_extension(name: &str) -> String {
    format!("{name}{EXTENSION}")
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;
    use rstest::rstest;

    use super::*;

    #[rstest]
    #[case("nota.md", Some("nota"))]
    #[case("a/b/nota.md", Some("a/b/nota"))]
    #[case("Casa.MD", Some("Casa"))]
    #[case("casa.mD", Some("casa"))]
    #[case("ñ.md", Some("ñ"))]
    #[case("x.md.md", Some("x.md"))]
    #[case("..md", Some("."))]
    #[case(".md", None)]
    #[case("a/.md", None)]
    #[case("md", None)]
    #[case("", None)]
    #[case("nota.md.tmp", None)]
    #[case("nota.markdown", None)]
    #[case("imagen.png", None)]
    #[case("ñ", None)]
    #[case("aé", None)]
    #[case("nota.ḿd", None)]
    fn el_nombre_de_una_nota(#[case] path: &str, #[case] stem: Option<&str>) {
        assert_eq!(strip_extension(path), stem);
        assert_eq!(is_note(path), stem.is_some());
    }

    #[test]
    fn agregar_la_extension_siempre_da_una_nota_con_ese_nombre() {
        for name in ["nota", "a/nota", "x.md", "Casa.MD", "md"] {
            let file = with_extension(name);
            assert_eq!(strip_extension(&file), Some(name));
        }
    }
}
