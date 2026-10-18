//! El frontmatter: los `---`, las claves del primer nivel y los tags de
//! `tags:` como tags.

use super::flatten::{Layer, Layers};
use super::inline::line_ranges;
use super::{LineKind, LineStyle, Role};
use crate::frontmatter;

/// `text` son las líneas del frontmatter, del `---` que abre al que cierra,
/// sin el salto final. `None` si no es un frontmatter.
pub(crate) fn style(text: &str) -> Option<Vec<LineStyle>> {
    let (Some(frontmatter), _) = frontmatter::parse(text, &mut Vec::new()) else {
        return None;
    };
    let lines = line_ranges(text);
    let mut layers = Layers::default();
    let last = lines.len() - 1;
    for (index, line) in lines.iter().enumerate() {
        let content = &text[line.clone()];
        if index == 0 || index == last {
            let start = line.start + (content.len() - content.trim_start_matches('\u{FEFF}').len());
            layers.push(start..line.end, Layer::Role(Role::Markup));
            continue;
        }
        layers.push(line.clone(), Layer::Role(Role::Frontmatter));
        if let Some(colon) = top_level_key(content) {
            layers.push(
                line.start..line.start + colon,
                Layer::Role(Role::FrontmatterKey),
            );
        }
    }
    for value in &frontmatter.tags {
        layers.push(value.span.clone(), Layer::Role(Role::Tag));
    }
    Some(
        layers
            .flatten(&lines)
            .into_iter()
            .map(|spans| LineStyle {
                kind: LineKind::Frontmatter,
                spans,
                ..LineStyle::default()
            })
            .collect(),
    )
}

/// Dónde está el `:` de una clave del primer nivel (`tags: …`).
fn top_level_key(line: &str) -> Option<usize> {
    let first = *line.as_bytes().first()?;
    if first.is_ascii_whitespace() || matches!(first, b'#' | b'-') {
        return None;
    }
    line.find(':')
}
