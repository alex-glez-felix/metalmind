//! Lo que la lista de notas muestra de cada una sin abrirla: su título y el
//! principio de su texto.

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
        // El scanner salta la BOM; los headings empiezan después.
        let body = scan.frontmatter.as_ref().map_or_else(
            || text.len() - text.trim_start_matches('\u{feff}').len(),
            |frontmatter| frontmatter.span.end,
        );
        let headings: Vec<usize> = scan
            .headings
            .iter()
            .map(|heading| heading.span.start)
            .collect();
        Self {
            title,
            excerpt: excerpt(text, body, &headings),
        }
    }
}

/// Las líneas de prosa desde `from`, unidas con un espacio y recortadas a
/// [`EXCERPT_CHARS`]. Los bloques cercados se saltan con las reglas del
/// scanner. `headings` es dónde empieza cada uno, en orden.
fn excerpt(text: &str, from: usize, headings: &[usize]) -> String {
    let mut excerpt = String::new();
    let mut chars = 0;
    let mut headings = headings.iter().copied().peekable();
    let mut at = from;
    while at < text.len() && chars < EXCERPT_CHARS {
        if let Some(end) = metalmind_syntax::fenced_block_end(text, at) {
            at = end;
            continue;
        }
        let end = text[at..]
            .find('\n')
            .map_or(text.len(), |newline| at + newline + 1);
        while headings.next_if(|&start| start < at).is_some() {}
        let line = text[at..end].trim();
        if !line.is_empty() && headings.peek() != Some(&at) {
            if !excerpt.is_empty() {
                excerpt.push(' ');
                chars += 1;
            }
            // Una línea larguísima no se copia entera: basta lo que se ve.
            let taken: String = line.chars().take(EXCERPT_CHARS - chars).collect();
            chars += taken.chars().count();
            excerpt.push_str(&taken);
        }
        at = end;
    }
    excerpt.truncate(excerpt.trim_end().len());
    excerpt
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
    #[case::una_cerca_mas_corta_no_cierra(
        "````md\n```\ncódigo\n```\n````\nTexto.\n",
        None,
        "Texto."
    )]
    #[case::una_cerca_sin_cerrar_llega_al_final("Antes\n```\nnada más\n", None, "Antes")]
    #[case::vacia("", None, "")]
    #[case::con_bom("\u{feff}# Título\nTexto.\n", Some("Título"), "Texto.")]
    fn titulo_y_extracto(#[case] text: &str, #[case] title: Option<&str>, #[case] excerpt: &str) {
        let summary = Summary::of(text);
        assert_eq!(summary.title.as_deref(), title);
        assert_eq!(summary.excerpt, excerpt);
    }

    #[test]
    fn una_linea_enorme_no_se_copia_entera() {
        let text = format!("Antes\n{}\n", "x".repeat(1_000_000));
        let excerpt = Summary::of(&text).excerpt;
        assert_eq!(excerpt.chars().count(), EXCERPT_CHARS);
        assert!(excerpt.starts_with("Antes xxx"));
    }

    #[test]
    fn muchos_headings_antes_de_la_prosa() {
        let text = format!("{}Al fin.\n", "## h\n".repeat(20_000));
        assert_eq!(Summary::of(&text).excerpt, "Al fin.");
    }

    #[test]
    fn el_extracto_se_corta_en_un_caracter_entero() {
        let text = "ñandú ".repeat(200);
        let excerpt = Summary::of(&text).excerpt;
        assert_eq!(excerpt.chars().count(), EXCERPT_CHARS);
        assert!(text.starts_with(&excerpt));
    }
}
