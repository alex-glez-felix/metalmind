//! Dónde empieza a escribir quien acaba de crear una nota desde una
//! plantilla: la primera sección que espera texto.

use metalmind_syntax::{Heading, scan};

/// El byte donde va el cursor en `text`: bajo el primer heading cuya
/// sección está vacía, sin contar uno cuyo contenido son solo subsecciones
/// (el `# Título` sobre `## Resumen`). Sin eso, la primera línea vacía del
/// cuerpo, y si no hay, el final del texto.
#[must_use]
pub fn first_gap(text: &str) -> usize {
    let scan = scan(text);
    let body = scan.frontmatter.as_ref().map_or(0, |frontmatter| {
        line_start_at_or_after(text, frontmatter.span.end)
    });
    let headings: Vec<&Heading> = scan
        .headings
        .iter()
        .filter(|heading| heading.span.start >= body)
        .collect();
    let next_levels = headings
        .iter()
        .skip(1)
        .map(|next| Some(*next))
        .chain([None]);
    headings
        .iter()
        .zip(next_levels)
        .find_map(|(heading, next)| gap_under(text, heading, next))
        .or_else(|| empty_line_from(text, body))
        .unwrap_or_else(|| text.trim_end_matches('\n').len())
}

/// Dónde escribir bajo `heading` si su sección, hasta `next`, está vacía y
/// `next` no es una subsección suya: la línea vacía que lo sigue o, si no
/// hay, el final de su línea.
fn gap_under(text: &str, heading: &Heading, next: Option<&Heading>) -> Option<usize> {
    let start = line_after(text, heading.span.end);
    let end = next.map_or(text.len(), |next| next.span.start);
    let empty = text[start..end].trim().is_empty();
    let container = next.is_some_and(|next| next.level > heading.level);
    if !empty || container {
        return None;
    }
    Some(if text[start..].starts_with('\n') {
        start
    } else {
        heading.span.end
    })
}

fn empty_line_from(text: &str, from: usize) -> Option<usize> {
    if text[from..].starts_with('\n') {
        return Some(from);
    }
    text[from..].find("\n\n").map(|at| from + at + 1)
}

/// El inicio de la línea que sigue a la que termina en `end`.
fn line_after(text: &str, end: usize) -> usize {
    match text[end..].find('\n') {
        Some(at) => end + at + 1,
        None => text.len(),
    }
}

fn line_start_at_or_after(text: &str, at: usize) -> usize {
    if at == 0 || text[..at].ends_with('\n') {
        at
    } else {
        line_after(text, at)
    }
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;
    use rstest::rstest;

    use super::*;

    /// `text` con `|` donde tiene que quedar el cursor.
    #[rstest]
    #[case::empty("|")]
    #[case::under_the_title("# Plan\n|\nAsistentes:\n")]
    #[case::text_under_the_title("# Plan\n\nFecha: hoy\n\n## Resumen\n|\n## Preguntas\n")]
    #[case::heading_right_after_heading("# Plan\n\n## Resumen|\n## Notas\n")]
    #[case::a_list_is_content("## Preguntas\n\n- una\n\n## Notas\n|\n")]
    #[case::title_only_at_its_end("# 2026-10-18|\n")]
    #[case::first_section_not_the_title("# Plan\n\n## Resumen\n|\n## Preguntas\n\n")]
    #[case::same_level_sections("## Resumen\n|\n## Preguntas\n")]
    #[case::section_with_text_is_skipped("# Plan\n\n## Resumen\nya escrito\n## Notas\n|\n")]
    #[case::after_the_frontmatter("---\ntags: [x]\n---\n# Plan\n\n## Notas|\n")]
    #[case::frontmatter_only("---\ntags: [x]\n---|\n")]
    #[case::no_heading_first_empty_line("Asistentes:\n|\nTemas:\n")]
    #[case::no_heading_body_starts_empty("---\na: 1\n---\n|\nTexto\n")]
    #[case::no_gap_at_all("Texto\nmás|\n")]
    #[case::heading_in_code_is_not_a_heading("```\n# no\n\n```\n# Sí\n|\n")]
    #[case::deeper_after_blank_lines("# Plan\n\n\n### Detalle\n|\n")]
    #[case::utf8("# Reunión\n|\n")]
    fn gap(#[case] marked: &str) {
        let at = marked.find('|').unwrap();
        let text = marked.replacen('|', "", 1);
        assert_eq!(first_gap(&text), at, "{marked:?}");
    }
}
