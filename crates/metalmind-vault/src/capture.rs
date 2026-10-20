//! Capturar una nota nueva en la raíz del vault, lo que hace `metalmind add`. Sin
//! tags, la nota queda en el buzón (06 §9): la captura nunca pone uno por su
//! cuenta.

use std::fmt::Write as _;
use std::path::Path;

use jiff::civil::DateTime;
use metalmind_syntax::note_name;

use crate::file::{FileError, create_atomic};
use crate::path::{InvalidPath, VaultPath, is_word_char};
use crate::tags::TagKey;

/// Lo que se le pide a la captura; todo tal como lo escribió el usuario.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct NewNote {
    pub text: String,
    /// Va como `# Título` al principio del cuerpo y da el nombre del archivo.
    pub title: Option<String>,
    /// Con o sin `#`; van en `tags:` del frontmatter.
    pub tags: Vec<String>,
}

/// La nota que quedó en disco.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Captured {
    pub path: VaultPath,
    /// Sin `#` ni repetidos (con la clave de 05 §6), en el orden en que se
    /// pidieron.
    pub tags: Vec<String>,
}

#[derive(Debug, thiserror::Error)]
pub enum CaptureError {
    #[error("la nota está vacía: falta el texto o el título")]
    Empty,
    #[error("el título tiene que ser una sola línea")]
    MultilineTitle,
    #[error("`{0}` no es un tag válido")]
    InvalidTag(String),
    #[error("el texto ya trae frontmatter: pon los tags en él")]
    FrontmatterWithTags,
    #[error("ya hay demasiadas notas llamadas {0}")]
    NameTaken(String),
    #[error(transparent)]
    InvalidName(#[from] InvalidPath),
    #[error(transparent)]
    Write(FileError),
}

/// Cuántos caracteres del título o la primera línea entran en el nombre: con
/// 4 bytes por carácter en el peor caso, el nombre cabe en los 255 de ext4.
const MAX_SLUG_CHARS: usize = 50;
/// `nombre.md`, `nombre-2.md`, … hasta aquí.
const MAX_ATTEMPT: u32 = 999;

/// Escribe `note` en la raíz de `root` como `slug.md`, sin pisar
/// nada: si el nombre está tomado, prueba `-2`, `-3`, … Sin slug (el texto
/// no tiene letras ni números), el nombre es la fecha y la hora de `now`.
pub(crate) fn capture(
    root: &Path,
    note: &NewNote,
    now: DateTime,
) -> Result<Captured, CaptureError> {
    let tags = tags(&note.tags)?;
    let title = title(note.title.as_deref())?;
    let (frontmatter, body) = split_frontmatter(&note.text);
    if note.text.trim().is_empty() && title.is_none() {
        return Err(CaptureError::Empty);
    }
    if frontmatter.is_some() && !tags.is_empty() {
        return Err(CaptureError::FrontmatterWithTags);
    }
    let contents = contents(frontmatter, &tags, title, body);
    let stem = match title.map_or_else(|| first_slug(body), slug) {
        slug if slug.is_empty() => now.strftime("%Y-%m-%d-%H%M%S").to_string(),
        slug => slug,
    };
    let path = create_unique(root, &stem, &contents)?;
    Ok(Captured { path, tags })
}

fn tags(tags: &[String]) -> Result<Vec<String>, CaptureError> {
    let mut valid: Vec<String> = Vec::with_capacity(tags.len());
    let mut seen: Vec<TagKey> = Vec::with_capacity(tags.len());
    for tag in tags {
        let name = tag.strip_prefix('#').unwrap_or(tag);
        if !metalmind_syntax::is_valid_tag_name(name) {
            return Err(CaptureError::InvalidTag(tag.clone()));
        }
        // `Rust` y `rust` son el mismo tag (05 §6): queda la primera forma.
        let key = TagKey::new(name);
        if !seen.contains(&key) {
            seen.push(key);
            valid.push(name.to_owned());
        }
    }
    Ok(valid)
}

fn title(title: Option<&str>) -> Result<Option<&str>, CaptureError> {
    let Some(title) = title.map(str::trim).filter(|title| !title.is_empty()) else {
        return Ok(None);
    };
    if title.contains(['\n', '\r']) {
        return Err(CaptureError::MultilineTitle);
    }
    Ok(Some(title))
}

/// El frontmatter que ya traiga el texto, con su línea de cierre, y el resto.
fn split_frontmatter(text: &str) -> (Option<&str>, &str) {
    match metalmind_syntax::scan(text).frontmatter {
        Some(frontmatter) => {
            let (head, body) = text.split_at(frontmatter.span.end);
            (Some(head), body.strip_prefix('\n').unwrap_or(body))
        }
        None => (None, text),
    }
}

fn contents(frontmatter: Option<&str>, tags: &[String], title: Option<&str>, body: &str) -> String {
    let mut out = String::new();
    if let Some(frontmatter) = frontmatter {
        out.push_str(frontmatter);
        out.push('\n');
    } else if !tags.is_empty() {
        out.push_str(&tags_frontmatter(tags));
    }
    // Solo los saltos: dos espacios antes de `\n` son un salto de línea de markdown.
    let body = body.trim_end_matches(['\n', '\r']);
    let body = if body.trim().is_empty() { "" } else { body };
    if let Some(title) = title {
        let _ = writeln!(out, "# {title}");
        let body = body.trim_start_matches(['\n', '\r']);
        if !body.is_empty() {
            out.push('\n');
        }
        out.push_str(body);
    } else {
        out.push_str(body);
    }
    if !out.ends_with('\n') {
        out.push('\n');
    }
    out
}

/// Entre comillas: sin ellas, `#2024` o `#true` se leerían como número o
/// booleano. Un tag válido no lleva `"` ni `\`, así que no hay nada que escapar.
fn tags_frontmatter(tags: &[String]) -> String {
    let list: Vec<String> = tags.iter().map(|tag| format!("\"{tag}\"")).collect();
    format!("---\ntags: [{}]\n---\n", list.join(", "))
}

/// El slug de la primera línea que tenga letras o números.
fn first_slug(body: &str) -> String {
    body.lines()
        .map(slug)
        .find(|slug| !slug.is_empty())
        .unwrap_or_default()
}

/// Las palabras de `line` en minúsculas y unidas por `-`, sin cortar ninguna
/// salvo que la primera sola ya no quepa. Acentos y otros alfabetos se quedan.
fn slug(line: &str) -> String {
    let mut slug = String::new();
    let mut chars = 0;
    for word in line
        .split(|c: char| !is_word_char(c))
        .filter(|word| !word.is_empty())
    {
        let word = word.to_lowercase();
        let len = word.chars().count();
        if slug.is_empty() {
            slug.extend(word.chars().take(MAX_SLUG_CHARS));
            chars = len.min(MAX_SLUG_CHARS);
        } else if chars + 1 + len <= MAX_SLUG_CHARS {
            slug.push('-');
            slug.push_str(&word);
            chars += 1 + len;
        } else {
            break;
        }
    }
    slug
}

fn create_unique(root: &Path, stem: &str, contents: &str) -> Result<VaultPath, CaptureError> {
    for attempt in 1..=MAX_ATTEMPT {
        let name = if attempt == 1 {
            note_name::with_extension(stem)
        } else {
            note_name::with_extension(&format!("{stem}-{attempt}"))
        };
        let path = VaultPath::root().join(&name)?;
        let write = |out: &mut dyn std::io::Write| out.write_all(contents.as_bytes());
        match create_atomic(&path.to_fs(root), write) {
            Ok(()) => return Ok(path),
            Err(FileError::Exists { .. }) => {}
            Err(error) => return Err(CaptureError::Write(error)),
        }
    }
    Err(CaptureError::NameTaken(note_name::with_extension(stem)))
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;
    use rstest::rstest;

    use super::*;

    #[rstest]
    #[case("Comprar leche", "comprar-leche")]
    #[case("  # Idea: probar GPUI!  ", "idea-probar-gpui")]
    #[case("- [ ] llamar a la imprenta", "llamar-a-la-imprenta")]
    #[case("Canción de AÑO nuevo", "canción-de-año-nuevo")]
    #[case("日本語のメモ", "日本語のメモ")]
    #[case("cancio\u{301}n de an\u{303}o", "cancio\u{301}n-de-an\u{303}o")]
    #[case("नमस्ते दुनिया", "नमस्ते-दुनिया")]
    #[case("#rust y #gpui", "rust-y-gpui")]
    #[case("🔥 --- ...", "")]
    #[case("", "")]
    fn slug_de_una_linea(#[case] line: &str, #[case] expected: &str) {
        assert_eq!(slug(line), expected);
    }

    #[test]
    fn el_slug_corta_entre_palabras() {
        let line = "palabra ".repeat(20);
        let slug = slug(&line);
        assert_eq!(slug, ["palabra"; 6].join("-"));
        assert!(slug.chars().count() <= MAX_SLUG_CHARS);
    }

    #[test]
    fn una_palabra_enorme_se_corta() {
        assert_eq!(slug(&"ñ".repeat(80)), "ñ".repeat(MAX_SLUG_CHARS));
    }

    #[test]
    fn el_slug_sale_de_la_primera_linea_con_letras() {
        assert_eq!(first_slug("\n---\n\n  Hola mundo\notra"), "hola-mundo");
        assert_eq!(first_slug("🔥\n"), "");
    }

    #[rstest]
    #[case(&[], None, "idea", "idea\n")]
    #[case(&[], None, "idea\n\n\n", "idea\n")]
    #[case(&[], None, "línea  \nsigue  \n\n", "línea  \nsigue  \n")]
    #[case(&[], None, "a\r\n\r\n", "a\n")]
    #[case(&[], Some("Título"), "", "# Título\n")]
    #[case(&[], Some("Título"), " \t\n", "# Título\n")]
    #[case(&[], Some("Título"), "\n\ncuerpo\n", "# Título\n\ncuerpo\n")]
    #[case(&["rust"], None, "idea", "---\ntags: [\"rust\"]\n---\nidea\n")]
    #[case(
        &["2024", "gpui"],
        Some("T"),
        "x",
        "---\ntags: [\"2024\", \"gpui\"]\n---\n# T\n\nx\n"
    )]
    fn contenido(
        #[case] tags: &[&str],
        #[case] title: Option<&str>,
        #[case] body: &str,
        #[case] expected: &str,
    ) {
        let tags: Vec<String> = tags.iter().map(|&tag| tag.to_owned()).collect();
        assert_eq!(contents(None, &tags, title, body), expected);
    }

    #[test]
    fn el_titulo_va_despues_del_frontmatter_del_texto() {
        let (frontmatter, body) = split_frontmatter("---\nalias: x\n---\ncuerpo\n");
        assert_eq!(frontmatter, Some("---\nalias: x\n---"));
        assert_eq!(
            contents(frontmatter, &[], Some("T"), body),
            "---\nalias: x\n---\n# T\n\ncuerpo\n"
        );
    }

    #[test]
    fn los_tags_se_validan_y_se_dejan_sin_almohadilla_ni_repetidos() {
        let input = |tags: &[&str]| tags.iter().map(|&t| t.to_owned()).collect::<Vec<_>>();
        assert_eq!(
            tags(&input(&[
                "#rust",
                "gpui",
                "rust",
                "Rust",
                "GPUI",
                "cancio\u{301}n",
                "canción"
            ]))
            .unwrap(),
            ["rust", "gpui", "cancio\u{301}n"]
        );
        assert!(matches!(
            tags(&input(&["con espacio"])),
            Err(CaptureError::InvalidTag(tag)) if tag == "con espacio"
        ));
        assert!(matches!(
            tags(&input(&["#"])),
            Err(CaptureError::InvalidTag(_))
        ));
    }
}
