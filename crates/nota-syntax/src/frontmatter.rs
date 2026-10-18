use std::ops::Range;

use crate::Problem;
use crate::code::{line_end, line_start};

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
    /// La plantilla de las notas nuevas del tag, en una página de tag.
    pub template: Option<Value>,
    /// Las claves del primer nivel con lo que ocupa cada una, en orden. Es
    /// `None` si el YAML no se pudo leer o no es un mapa en bloque con cada
    /// clave al principio de su línea: solo así se puede cambiar o agregar un
    /// campo sin tocar los demás.
    pub entries: Option<Vec<Entry>>,
}

/// Una clave del primer nivel del frontmatter y su valor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    pub key: String,
    /// Desde el principio de la línea de la clave hasta el fin de la última
    /// línea de su valor, salto incluido. Las líneas en blanco y los
    /// comentarios que la siguen quedan fuera.
    pub span: Range<usize>,
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
    /// El campo `key`, si el frontmatter se puede editar y lo tiene.
    #[must_use]
    pub fn entry(&self, key: &str) -> Option<&Entry> {
        self.entries.as_ref()?.iter().find(|entry| entry.key == key)
    }

    fn read_fields(&mut self, text: &str, fields: Fields, problems: &mut Vec<Problem>) {
        self.entries = fields.keys.and_then(|keys| entries(text, &self.yaml, keys));
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
        self.template = reader.single("template", fields.template);
    }
}

fn entries(text: &str, yaml: &Range<usize>, keys: Vec<Item>) -> Option<Vec<Entry>> {
    let starts: Vec<usize> = keys.iter().map(|key| yaml.start + key.span.start).collect();
    if starts.iter().any(|&start| line_start(text, start) != start) {
        return None;
    }
    let ends = starts.iter().skip(1).copied().chain([yaml.end]);
    Some(
        keys.into_iter()
            .zip(starts.iter().copied().zip(ends))
            .map(|(key, (start, end))| Entry {
                key: key.text,
                span: start..without_trailing_lines(text, start, end),
            })
            .collect(),
    )
}

/// `end` sin las líneas en blanco ni los comentarios de la columna 0 que
/// cierran el tramo: son del campo siguiente o de nadie.
fn without_trailing_lines(text: &str, start: usize, mut end: usize) -> usize {
    while end > start {
        let last = line_start(text, end - 1).max(start);
        let line = &text[last..end];
        if last == start || !(line.trim().is_empty() || line.starts_with('#')) {
            break;
        }
        end = last;
    }
    end
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
            None | Some(Field::Null) => Vec::new(),
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
            Field::Null => None,
            Field::Text(item) => self.value(&item, Place::Text),
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
