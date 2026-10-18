use std::ops::Range;

use crate::Problem;
use crate::code::line_end;

mod value;
mod yaml;

use value::Place;
pub use value::Value;
use yaml::{Field, Fields, Item, ReadError};

/// El bloque `---` … `---` al principio de la nota. Solo se leen los campos
/// que nota entiende, cada uno con su rango para reescribirlo sin tocar el
/// resto del YAML del usuario.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Frontmatter {
    /// Desde el primer `---` hasta el fin de la línea del `---` de cierre.
    pub span: Range<usize>,
    /// El YAML entre los dos `---`.
    pub yaml: Range<usize>,
    /// Ya validados y sin `#`: `#tag` y `tag` son el mismo tag.
    pub tags: Vec<Value>,
    /// Este y los que siguen, tal cual: solo una página de tag les da
    /// sentido (05 §7), y ahí se validan.
    pub aliases: Vec<Value>,
    pub parents: Vec<Value>,
    pub cssclasses: Vec<Value>,
    pub color: Option<Value>,
    pub icon: Option<Value>,
}

/// El BOM no es parte de la primera línea: sin saltarlo, ni una cerca ni un
/// tag al principio de la nota se reconocerían.
fn bom_len(text: &str) -> usize {
    if text.starts_with('\u{FEFF}') {
        '\u{FEFF}'.len_utf8()
    } else {
        0
    }
}

fn is_delimiter(line: &str) -> bool {
    line.trim_end() == "---"
}

/// El rango del bloque y el de su YAML. Sin `---` de cierre no hay
/// frontmatter: el `---` del principio es una regla horizontal.
fn delimit(text: &str) -> Option<(Range<usize>, Range<usize>)> {
    let start = bom_len(text);
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
        return (None, bom_len(text));
    };
    let body = (span.end + 1).min(text.len());
    let mut frontmatter = Frontmatter {
        span,
        yaml: yaml.clone(),
        ..Frontmatter::default()
    };
    match yaml::read(&text[yaml.clone()]) {
        Ok(fields) => frontmatter.read_fields(text, fields, problems),
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
    fn read_fields(&mut self, text: &str, fields: Fields, problems: &mut Vec<Problem>) {
        let mut reader = FieldReader {
            text,
            offset: self.yaml.start,
            span: self.yaml.clone(),
            problems,
        };
        for tag in reader.list("tags", fields.tags) {
            match tag.as_tag() {
                Some(tag) => self.tags.push(tag),
                None => reader.problems.push(Problem::InvalidTag {
                    name: tag.text,
                    span: tag.span,
                }),
            }
        }
        self.aliases = reader.list("aliases", fields.aliases);
        self.parents = reader.list("parents", fields.parents);
        self.cssclasses = reader.list("cssclasses", fields.cssclasses);
        self.color = reader.single("color", fields.color);
        self.icon = reader.single("icon", fields.icon);
    }
}

/// Pasa los campos del YAML a valores con rangos sobre la nota.
struct FieldReader<'a> {
    text: &'a str,
    /// Dónde empieza el YAML en la nota.
    offset: usize,
    /// El YAML entero, para los problemas de un campo.
    span: Range<usize>,
    problems: &'a mut Vec<Problem>,
}

impl FieldReader<'_> {
    /// Una lista o un solo texto. Un texto es un solo valor, con o sin
    /// comas: partirlo adivinaría la intención, y renombrar un tag tendría
    /// que reescribir dentro de él.
    fn list(&mut self, field: &'static str, value: Option<Field>) -> Vec<Value> {
        match value {
            None => Vec::new(),
            Some(Field::List(items)) => items
                .iter()
                .filter_map(|item| self.value(item, Place::List))
                .collect(),
            Some(Field::Text(item)) => self.value(&item, Place::Text).into_iter().collect(),
            Some(Field::Invalid) => {
                self.problems.push(Problem::InvalidField {
                    field,
                    span: self.span.clone(),
                });
                Vec::new()
            }
        }
    }

    /// Un solo texto; vacío o `null` es no tenerlo.
    fn single(&mut self, field: &'static str, value: Option<Field>) -> Option<Value> {
        match value? {
            Field::Text(item) => self.value(&item, Place::Text),
            Field::List(items) if items.is_empty() => None,
            Field::List(_) | Field::Invalid => {
                self.problems.push(Problem::NotText {
                    field,
                    span: self.span.clone(),
                });
                None
            }
        }
    }

    fn value(&self, item: &Item, place: Place) -> Option<Value> {
        let span = item.span.start + self.offset..item.span.end + self.offset;
        Value::new(self.text, &item.text, span, place)
    }
}
