//! Lo que la lista de notas muestra de cada una sin abrirla: su título y el
//! principio de su texto.

use std::ops::Range;

/// Hasta cuántos caracteres del cuerpo guarda el extracto: más de lo que
/// cabe en las líneas de una fila, que lo recortan.
const EXCERPT_CHARS: usize = 280;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Summary {
    /// El texto del primer `# ` de la nota; sin uno, la nota se nombra por su
    /// archivo.
    pub title: Option<String>,
    /// Las primeras líneas de texto, sin frontmatter, headings ni código, en
    /// una sola línea.
    pub excerpt: String,
}

impl Summary {
    #[must_use]
    pub fn of(text: &str) -> Self {
        let scan = metalmind_syntax::scan(text);
        let title = scan
            .headings
            .iter()
            .find(|heading| heading.level == 1)
            .map(|heading| text[heading.text.clone()].to_owned());
        let body = scan
            .frontmatter
            .as_ref()
            .map_or(0, |frontmatter| frontmatter.span.end);
        let headings: Vec<Range<usize>> = scan
            .headings
            .iter()
            .map(|heading| heading.span.clone())
            .collect();
        Self {
            title,
            excerpt: excerpt(text, body, &headings),
        }
    }
}

/// Las líneas de prosa desde `from`, unidas con un espacio y recortadas a
/// [`EXCERPT_CHARS`].
fn excerpt(text: &str, from: usize, headings: &[Range<usize>]) -> String {
    let mut excerpt = String::new();
    let mut fence: Option<&str> = None;
    let mut start = from;
    for line in text[from..].split_inclusive('\n') {
        let at = start;
        start += line.len();
        let trimmed = line.trim();
        if let Some(open) = fence {
            if trimmed.starts_with(open) {
                fence = None;
            }
            continue;
        }
        if let Some(open) = fence_marker(trimmed) {
            fence = Some(open);
            continue;
        }
        if trimmed.is_empty() || headings.iter().any(|span| span.start == at) {
            continue;
        }
        if !excerpt.is_empty() {
            excerpt.push(' ');
        }
        excerpt.push_str(trimmed);
        if excerpt.chars().count() >= EXCERPT_CHARS {
            break;
        }
    }
    match excerpt.char_indices().nth(EXCERPT_CHARS) {
        Some((cut, _)) => excerpt[..cut].to_owned(),
        None => excerpt,
    }
}

/// El marcador de una cerca de código que abre en esta línea.
fn fence_marker(line: &str) -> Option<&'static str> {
    ["```", "~~~"]
        .into_iter()
        .find(|marker| line.starts_with(marker))
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;
    use rstest::rstest;

    use super::*;

    #[rstest]
    #[case::con_titulo(
        "# El render loop\n\nCada frame pinta.\n",
        Some("El render loop"),
        "Cada frame pinta."
    )]
    #[case::sin_titulo(
        "Una idea suelta\ny su segunda línea\n",
        None,
        "Una idea suelta y su segunda línea"
    )]
    #[case::solo_un_h2("## Sección\ntexto\n", None, "texto")]
    #[case::el_primer_h1("texto\n# Uno\n# Dos\n", Some("Uno"), "texto")]
    #[case::sin_frontmatter(
        "---\ntags: [rust]\n---\n# Título\nCuerpo.\n",
        Some("Título"),
        "Cuerpo."
    )]
    #[case::sin_codigo(
        "Antes\n```rust\nfn main() {}\n```\n~~~\notro\n~~~\nDespués\n",
        None,
        "Antes Después"
    )]
    #[case::un_heading_en_codigo_no_es_titulo("```\n# no\n```\n", None, "")]
    #[case::vacia("", None, "")]
    fn titulo_y_extracto(#[case] text: &str, #[case] title: Option<&str>, #[case] excerpt: &str) {
        let summary = Summary::of(text);
        assert_eq!(summary.title.as_deref(), title);
        assert_eq!(summary.excerpt, excerpt);
    }

    #[test]
    fn el_extracto_se_corta_en_un_caracter_entero() {
        let text = "ñandú ".repeat(200);
        let excerpt = Summary::of(&text).excerpt;
        assert_eq!(excerpt.chars().count(), EXCERPT_CHARS);
        assert!(text.starts_with(&excerpt));
    }
}
