//! El nombre de un adjunto nuevo: `<base>-<fecha>-<hora>.<ext>`, con
//! `-1`, `-2`, … si ya existe uno igual.

use jiff::civil::DateTime;

/// Base de un pegado cuando no queda nada legible del nombre de origen.
const FALLBACK_BASE: &str = "adjunto";
/// Extensión de un contenido que no se reconoce y no trae una propia.
const FALLBACK_EXTENSION: &str = "bin";
/// Largo máximo de la base, en caracteres: el resto del nombre es fijo.
const MAX_BASE_CHARS: usize = 60;
const MAX_EXTENSION_CHARS: usize = 10;

/// La base legible de un nombre: letras y números de cualquier alfabeto,
/// `-` y `_`; lo demás (espacios, puntos, `[`, `|`, `#`, …) se vuelve un
/// solo `-`. Así el nombre nunca rompe un `![[...]]` ni queda oculto.
pub(crate) fn base(raw: &str) -> String {
    let mut out = String::new();
    let mut chars = 0;
    for c in raw.chars() {
        if chars == MAX_BASE_CHARS {
            break;
        }
        if c.is_alphanumeric() || c == '_' {
            out.push(c);
        } else if !out.is_empty() && !out.ends_with('-') {
            out.push('-');
        } else {
            continue;
        }
        chars += 1;
    }
    let trimmed = out.trim_end_matches('-');
    if trimmed.is_empty() {
        FALLBACK_BASE.to_owned()
    } else {
        trimmed.to_owned()
    }
}

/// La extensión de un nombre de archivo, si es corta y alfanumérica ASCII.
pub(crate) fn extension_of(file_name: &str) -> Option<String> {
    let (stem, extension) = file_name.rsplit_once('.')?;
    let valid = !stem.is_empty()
        && !extension.is_empty()
        && extension.len() <= MAX_EXTENSION_CHARS
        && extension.bytes().all(|byte| byte.is_ascii_alphanumeric());
    valid.then(|| extension.to_ascii_lowercase())
}

/// `extension_of` o `bin`.
pub(crate) fn extension_or_fallback(file_name: Option<&str>) -> String {
    file_name
        .and_then(extension_of)
        .unwrap_or_else(|| FALLBACK_EXTENSION.to_owned())
}

/// El nombre del intento `attempt`: el primero sin sufijo.
pub(crate) fn file_name(base: &str, at: DateTime, extension: &str, attempt: u32) -> String {
    let stamp = at.strftime("%Y-%m-%d-%H%M%S");
    if attempt == 0 {
        format!("{base}-{stamp}.{extension}")
    } else {
        format!("{base}-{stamp}-{attempt}.{extension}")
    }
}

#[cfg(test)]
mod tests {
    use jiff::civil::date;
    use pretty_assertions::assert_eq;
    use rstest::rstest;

    use super::*;

    #[rstest]
    #[case::simple("captura", "captura")]
    #[case::espacios_y_puntos("Mi foto.final", "Mi-foto-final")]
    #[case::sintaxis_de_link("a[[b]]|c#d^e", "a-b-c-d-e")]
    #[case::oculto(".config", "config")]
    #[case::acentos("señal_ñandú", "señal_ñandú")]
    #[case::solo_simbolos("!!!", "adjunto")]
    #[case::vacio("", "adjunto")]
    fn base_legible(#[case] raw: &str, #[case] expected: &str) {
        assert_eq!(base(raw), expected);
    }

    #[test]
    fn la_base_se_corta_por_caracteres() {
        let long = "ñ".repeat(100);
        assert_eq!(base(&long).chars().count(), MAX_BASE_CHARS);
    }

    #[rstest]
    #[case::normal("informe.PDF", Some("pdf"))]
    #[case::sin_extension("LEEME", None)]
    #[case::oculto(".bashrc", None)]
    #[case::rara("x.tar gz", None)]
    #[case::larga("x.extensionlarga", None)]
    fn extension_propia(#[case] name: &str, #[case] expected: Option<&str>) {
        assert_eq!(extension_of(name).as_deref(), expected);
    }

    #[test]
    fn nombre_con_fecha_y_sufijo() {
        let at = date(2026, 10, 18).at(9, 5, 7, 0);
        assert_eq!(
            file_name("captura", at, "png", 0),
            "captura-2026-10-18-090507.png"
        );
        assert_eq!(
            file_name("captura", at, "png", 2),
            "captura-2026-10-18-090507-2.png"
        );
    }
}
