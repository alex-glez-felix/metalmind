use std::ops::Range;

use crate::Problem;
use crate::code::line_end;
use crate::tag::is_valid_tag_name;

mod yaml;

use yaml::{Field, Fields, ReadError};

/// El bloque `---` … `---` al principio de la nota. Solo se leen los campos
/// que nota entiende; el YAML del usuario no se toca.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Frontmatter {
    /// Desde el primer `---` hasta el fin de la línea del `---` de cierre.
    pub span: Range<usize>,
    /// El YAML entre los dos `---`.
    pub yaml: Range<usize>,
    /// Sin `#`: `#tag` y `tag` son el mismo tag.
    pub tags: Vec<String>,
    pub aliases: Vec<String>,
    pub cssclasses: Vec<String>,
}

fn is_delimiter(line: &str) -> bool {
    line.trim_end() == "---"
}

/// El rango del bloque y el de su YAML. Sin `---` de cierre no hay
/// frontmatter: el `---` del principio es una regla horizontal.
fn delimit(text: &str) -> Option<(Range<usize>, Range<usize>)> {
    let start = if text.starts_with('\u{FEFF}') { 3 } else { 0 };
    let first_end = line_end(text, start);
    if !is_delimiter(&text[start..first_end]) || first_end == text.len() {
        return None;
    }
    let yaml_start = first_end + 1;
    let mut line = yaml_start;
    loop {
        let end = line_end(text, line);
        if is_delimiter(&text[line..end]) {
            return Some((start..end, yaml_start..line));
        }
        if end == text.len() {
            return None;
        }
        line = end + 1;
    }
}

/// El frontmatter de la nota y dónde empieza el cuerpo, más lo que haya
/// salido mal al leerlo.
pub(crate) fn parse(text: &str, problems: &mut Vec<Problem>) -> (Option<Frontmatter>, usize) {
    let Some((span, yaml)) = delimit(text) else {
        return (None, 0);
    };
    let body = (span.end + 1).min(text.len());
    let mut frontmatter = Frontmatter {
        span,
        yaml: yaml.clone(),
        ..Frontmatter::default()
    };
    match yaml::read(&text[yaml.clone()]) {
        Ok(fields) => frontmatter.read_fields(fields, problems),
        Err(ReadError::Syntax { at, message }) => problems.push(Problem::InvalidYaml {
            at: yaml.start + at,
            span: yaml,
            message,
        }),
        Err(ReadError::NotMapping) => problems.push(Problem::FrontmatterNotMapping { span: yaml }),
    }
    (Some(frontmatter), body)
}

impl Frontmatter {
    fn read_fields(&mut self, fields: Fields, problems: &mut Vec<Problem>) {
        let mut values = |name: &'static str,
                          field: Option<Field>,
                          split: fn(&str) -> Vec<String>| {
            match field {
                None => Vec::new(),
                Some(Field::List(items)) => items
                    .iter()
                    .map(|item| item.trim())
                    .filter(|item| !item.is_empty())
                    .map(str::to_owned)
                    .collect(),
                Some(Field::Text(text)) => split(&text),
                Some(Field::Invalid) => {
                    problems.push(Problem::InvalidField {
                        field: name,
                        span: self.yaml.clone(),
                    });
                    Vec::new()
                }
            }
        };
        let tags = values("tags", fields.tags, split_words);
        let aliases = values("aliases", fields.aliases, split_commas);
        let cssclasses = values("cssclasses", fields.cssclasses, split_words);
        self.tags = tags
            .into_iter()
            .map(|tag| tag.trim_start_matches('#').to_owned())
            .filter(|tag| is_valid_tag_name(tag))
            .collect();
        self.aliases = aliases;
        self.cssclasses = cssclasses;
    }
}

/// `tags: a, b c`: los tags no llevan espacios, así que separan las comas y
/// los espacios.
fn split_words(text: &str) -> Vec<String> {
    text.split(|c: char| c == ',' || c.is_whitespace())
        .filter(|word| !word.is_empty())
        .map(str::to_owned)
        .collect()
}

/// `aliases: a, b`: un alias puede llevar espacios, solo separan las comas.
fn split_commas(text: &str) -> Vec<String> {
    text.split(',')
        .map(str::trim)
        .filter(|alias| !alias.is_empty())
        .map(str::to_owned)
        .collect()
}
