use std::ops::Range;

use yaml_rust2::{Yaml, YamlLoader};

use crate::Problem;
use crate::code::line_end;
use crate::tag::is_valid_tag_name;

/// El bloque `---` … `---` al principio de la nota. Solo se leen los campos
/// que Obsidian entiende; el YAML del usuario no se toca.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Frontmatter {
    /// Desde el primer `---` hasta el fin de la línea del `---` de cierre.
    pub span: Range<usize>,
    /// El YAML entre los dos `---`.
    pub yaml: Range<usize>,
    /// Sin `#`, como los escribe Obsidian.
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
        yaml,
        ..Frontmatter::default()
    };
    match YamlLoader::load_from_str(&text[frontmatter.yaml.clone()]) {
        Err(error) => {
            let yaml = frontmatter.yaml.clone();
            let at = (yaml.start + error.marker().index()).min(yaml.end);
            problems.push(Problem::InvalidYaml {
                at: text.floor_char_boundary(at),
                span: yaml,
                message: error.info().to_owned(),
            });
        }
        Ok(documents) => read_fields(&mut frontmatter, documents.first(), problems),
    }
    (Some(frontmatter), body)
}

fn read_fields(
    frontmatter: &mut Frontmatter,
    document: Option<&Yaml>,
    problems: &mut Vec<Problem>,
) {
    let span = frontmatter.yaml.clone();
    let fields = match document {
        None | Some(Yaml::Null) => return,
        Some(Yaml::Hash(fields)) => fields,
        Some(_) => {
            problems.push(Problem::FrontmatterNotMapping { span });
            return;
        }
    };
    // Obsidian sigue leyendo los nombres en singular de antes de la 1.4.
    let mut field = |names: [&'static str; 2], split: fn(&str) -> Vec<String>| {
        names
            .iter()
            .filter_map(|&name| {
                fields
                    .get(&Yaml::String(name.to_owned()))
                    .map(|v| (name, v))
            })
            .flat_map(|(name, value)| {
                values(value, split).unwrap_or_else(|| {
                    problems.push(Problem::InvalidField {
                        field: name,
                        span: span.clone(),
                    });
                    Vec::new()
                })
            })
            .collect::<Vec<_>>()
    };
    let tags = field(["tags", "tag"], split_words);
    let aliases = field(["aliases", "alias"], split_commas);
    let cssclasses = field(["cssclasses", "cssclass"], split_words);
    frontmatter.tags = tags
        .into_iter()
        .map(|tag| tag.trim_start_matches('#').to_owned())
        .filter(|tag| is_valid_tag_name(tag))
        .collect();
    frontmatter.aliases = aliases;
    frontmatter.cssclasses = cssclasses;
}

fn scalar(value: &Yaml) -> Option<String> {
    match value {
        Yaml::String(s) | Yaml::Real(s) => Some(s.clone()),
        Yaml::Integer(n) => Some(n.to_string()),
        Yaml::Boolean(b) => Some(b.to_string()),
        _ => None,
    }
}

/// Una lista de valores o un texto que se parte con `split`. `None` si el
/// campo es de otro tipo; un campo vacío no tiene valores.
fn values(value: &Yaml, split: fn(&str) -> Vec<String>) -> Option<Vec<String>> {
    match value {
        Yaml::Null => Some(Vec::new()),
        Yaml::Array(items) => Some(
            items
                .iter()
                .filter_map(scalar)
                .map(|item| item.trim().to_owned())
                .filter(|item| !item.is_empty())
                .collect(),
        ),
        other => scalar(other).map(|text| split(&text)),
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
