//! Dónde está la clave `tags` en el YAML de un frontmatter.

use std::ops::Range;

/// La clave `tags` de primer nivel, en bytes sobre el YAML.
pub(super) struct Field {
    /// Sus líneas, con el salto final. Los comentarios que siguen al valor
    /// no son suyos.
    pub whole: Range<usize>,
    /// Lo que va tras los `:` en su línea, sin el comentario ni el salto.
    pub value: Range<usize>,
    /// El comentario de su línea con los espacios que lo preceden, o vacío.
    pub comment: Range<usize>,
    /// Si el valor sigue en las líneas de abajo.
    pub continues: bool,
    /// Las de esas líneas que empiezan con `-`.
    pub items: Vec<Range<usize>>,
}

pub(super) fn tags_field(yaml: &str) -> Option<Field> {
    let mut lines = lines(yaml);
    let (key, colon) = lines.find_map(|line| Some((line.clone(), tags_colon(&yaml[line])?)))?;
    let content_end = key.start + yaml[key.clone()].trim_end_matches(['\n', '\r']).len();
    let after = key.start + colon + 1..content_end;
    let comment_start = after.start + comment_start(&yaml[after.clone()]);
    let mut field = Field {
        whole: key.clone(),
        value: after.start..comment_start,
        comment: comment_start..content_end,
        continues: false,
        items: Vec::new(),
    };
    for line in lines {
        let text = &yaml[line.clone()];
        if continues_value(text) {
            if text.trim_start_matches([' ', '\t']).starts_with('-') {
                field.items.push(line.clone());
            }
            field.whole.end = line.end;
            field.continues = true;
        } else if !is_blank_or_comment(text) {
            break;
        }
    }
    Some(field)
}

/// Dónde empieza el comentario de un valor en línea, con sus espacios
/// previos: un `#` tras un espacio y fuera de comillas.
fn comment_start(value: &str) -> usize {
    let mut quote = None;
    let mut previous = ' ';
    for (at, c) in value.char_indices() {
        match (quote, c) {
            (None, '#') if previous.is_whitespace() => {
                return value[..at].trim_end_matches([' ', '\t']).len();
            }
            (None, '"' | '\'') => quote = Some(c),
            (Some(open), _) if c == open => quote = None,
            _ => {}
        }
        previous = c;
    }
    value.len()
}

/// Cada línea con su salto.
fn lines(text: &str) -> impl Iterator<Item = Range<usize>> + '_ {
    let mut start = 0;
    std::iter::from_fn(move || {
        if start == text.len() {
            return None;
        }
        let end = text[start..]
            .find('\n')
            .map_or(text.len(), |at| start + at + 1);
        let line = start..end;
        start = end;
        Some(line)
    })
}

/// Dónde están los `:` si la línea es la clave `tags` de primer nivel.
fn tags_colon(line: &str) -> Option<usize> {
    ["tags", "\"tags\"", "'tags'"].iter().find_map(|key| {
        let rest = line.strip_prefix(key)?;
        let spaces = rest.len() - rest.trim_start_matches([' ', '\t']).len();
        let after = rest[spaces..].strip_prefix(':')?;
        (after.is_empty() || after.starts_with(char::is_whitespace)).then_some(key.len() + spaces)
    })
}

fn continues_value(line: &str) -> bool {
    !is_blank_or_comment(line) && (line.starts_with([' ', '\t']) || line.starts_with('-'))
}

fn is_blank_or_comment(line: &str) -> bool {
    let trimmed = line.trim_start();
    trimmed.is_empty() || trimmed.starts_with('#')
}
