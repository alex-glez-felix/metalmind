//! Cambiar una clave de un YAML del usuario sin reescribir el resto: los
//! comentarios, el orden y cómo están escritas las demás claves quedan
//! igual, como con el frontmatter. Solo se editan mapas en bloque. Cada
//! edición se comprueba releyendo el resultado: si no dice lo mismo que el
//! original con ese único cambio, es [`EditError::Unsupported`] y no se
//! aplica nada.
//!
//! Es texto a texto: quien llama lee el archivo y lo guarda de forma atómica.

mod block;
mod tree;

use std::fmt::Write as _;
use std::ops::Range;

use self::block::{Entry, Line, Map, NotBlockMap, lines};
use self::tree::Tree;
use crate::diagnostic::Diagnostic;

#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum EditError {
    /// El archivo no es YAML válido: se arregla a mano antes de cambiarlo.
    #[error("{0}")]
    Unreadable(Diagnostic),
    /// La clave está escrita de una forma que no se edita sin tocar otras
    /// (`{a: 1}`, una lista, un ancla): se cambia a mano.
    #[error("`{0}` está escrito de una forma que Metalmind no edita; cámbialo a mano")]
    Unsupported(String),
    #[error("ya existe `{0}`")]
    Exists(String),
    #[error("no existe `{0}`")]
    Missing(String),
}

/// Le da a la clave `path` el valor `value`, que ya es YAML (`true`, `15`,
/// o un texto de [`string`]). Las claves que faltan se agregan al final de
/// su mapa.
///
/// # Errors
/// Si el texto no es YAML o el camino no está escrito en bloque.
pub fn set(text: &str, path: &[&str], value: &str) -> Result<String, EditError> {
    let value_tree = read_tree(value)?;
    edit(
        text,
        path,
        |doc| doc.set(path, value),
        |tree| {
            tree.set(path, value_tree);
        },
    )
}

/// Quita la clave `path`; un mapa que se queda vacío se va con ella. Si no
/// está, no cambia nada.
///
/// # Errors
/// Como [`set`].
pub fn remove(text: &str, path: &[&str]) -> Result<String, EditError> {
    edit(text, path, |doc| doc.remove(path), |tree| tree.remove(path))
}

/// Cambia el nombre de la clave `path` por `to`, en su lugar y con su valor.
///
/// # Errors
/// Como [`set`]; además, si no existe o ya hay una clave `to` junto a ella.
pub fn rename(text: &str, path: &[&str], to: &str) -> Result<String, EditError> {
    edit(
        text,
        path,
        |doc| doc.rename(path, to),
        |tree| {
            tree.rename(path, to);
        },
    )
}

/// `text` como texto de YAML, entre comillas dobles: así se lee igual
/// empiece con `#`, `-` o un número.
#[must_use]
pub fn string(text: &str) -> String {
    let mut quoted = String::with_capacity(text.len() + 2);
    quoted.push('"');
    for c in text.chars() {
        match c {
            '"' => quoted.push_str("\\\""),
            '\\' => quoted.push_str("\\\\"),
            '\n' => quoted.push_str("\\n"),
            '\t' => quoted.push_str("\\t"),
            c if c.is_control() => {
                let _ = write!(quoted, "\\u{:04x}", u32::from(c));
            }
            c => quoted.push(c),
        }
    }
    quoted.push('"');
    quoted
}

/// Una clave como se escribe: sin comillas si se lee igual sin ellas.
fn key(name: &str) -> String {
    let plain = name.chars().next().is_some_and(char::is_alphabetic)
        && name
            .chars()
            .all(|c| c.is_alphanumeric() || " _-./()".contains(c))
        && !name.ends_with(' ')
        && !["true", "false", "null", "yes", "no", "on", "off", "y", "n"]
            .iter()
            .any(|word| name.eq_ignore_ascii_case(word));
    if plain { name.to_owned() } else { string(name) }
}

fn edit(
    text: &str,
    path: &[&str],
    change: impl FnOnce(&Document<'_>) -> Result<Option<Splice>, EditError>,
    expect: impl FnOnce(&mut Tree),
) -> Result<String, EditError> {
    let mut expected = read_tree(text)?;
    let document = Document::new(text);
    let Some(splice) = change(&document)? else {
        return Ok(text.to_owned());
    };
    let mut edited = text.to_owned();
    edited.replace_range(splice.range, &splice.text);
    expect(&mut expected);
    match read_tree(&edited) {
        Ok(tree) if tree == expected => Ok(edited),
        _ => Err(EditError::Unsupported(path.join("."))),
    }
}

fn read_tree(text: &str) -> Result<Tree, EditError> {
    if !lines(text).iter().any(Line::is_content) {
        return Ok(Tree::Map(Vec::new()));
    }
    serde_saphyr::from_str_with_options::<Tree>(text, crate::parse::options())
        .map_err(|error| EditError::Unreadable(crate::parse::to_diagnostic(&error)))
}

/// Cambiar `range` por `text`.
struct Splice {
    range: Range<usize>,
    text: String,
}

struct Document<'t> {
    text: &'t str,
    lines: Vec<Line>,
    eol: &'static str,
}

/// Un mapa del camino y, si está, la posición de su clave en él.
struct Step {
    map: Map,
    entry: Option<usize>,
}

impl Step {
    fn found(&self) -> Option<&Entry> {
        self.entry.map(|at| &self.map.entries[at])
    }
}

impl<'t> Document<'t> {
    fn new(text: &'t str) -> Self {
        let crlf = text
            .find('\n')
            .is_some_and(|end| text[..end].ends_with('\r'));
        Self {
            text,
            lines: lines(text),
            eol: if crlf { "\r\n" } else { "\n" },
        }
    }

    /// Los mapas del camino, del documento hasta la primera clave que
    /// falta o la última del camino.
    fn walk(&self, path: &[&str]) -> Result<Vec<Step>, EditError> {
        let mut steps: Vec<Step> = Vec::new();
        let mut region = 0..self.lines.len();
        for (depth, segment) in path.iter().enumerate() {
            let map = Map::read(&self.lines, region)
                .map_err(|NotBlockMap| EditError::Unsupported(path[..depth].join(".")))?;
            let entry = map.position(segment);
            let step = Step { map, entry };
            region = match step.found() {
                Some(found) if found.key.value.is_some() && depth + 1 < path.len() => {
                    return Err(EditError::Unsupported(path[..=depth].join(".")));
                }
                Some(found) => found.body.clone(),
                None => 0..0,
            };
            let missing = step.entry.is_none();
            steps.push(step);
            if missing {
                break;
            }
        }
        Ok(steps)
    }

    fn set(&self, path: &[&str], value: &str) -> Result<Option<Splice>, EditError> {
        let steps = self.walk(path)?;
        let Some((last, before)) = steps.split_last() else {
            return Ok(None);
        };
        let Some(entry) = last.found() else {
            let parent = before.last().and_then(Step::found);
            return Ok(Some(self.insert(
                last,
                parent,
                &path[before.len()..],
                value,
            )));
        };
        let line = &self.lines[entry.line];
        if entry.has_body() {
            let written = &self.text[line.start..entry.key.key_end];
            return Ok(Some(Splice {
                range: entry.span(&self.lines),
                text: format!("{written}: {value}{}", self.eol),
            }));
        }
        Ok(match &entry.key.value {
            Some(range) if &self.text[range.clone()] == value => None,
            Some(range) => Some(Splice {
                range: range.clone(),
                text: value.to_owned(),
            }),
            None => Some(Splice {
                range: entry.key.colon + 1..entry.key.colon + 1,
                text: format!(" {value}"),
            }),
        })
    }

    /// Las claves `rest`, anidadas, al final de `map`; si está vacío, bajo
    /// la clave `parent` o al final del documento.
    fn insert(&self, last: &Step, parent: Option<&Entry>, rest: &[&str], value: &str) -> Splice {
        let (indent, at) = match (last.map.entries.last(), parent) {
            (Some(entry), _) => (last.map.indent, entry.span(&self.lines).end),
            (None, Some(parent)) => {
                let line = &self.lines[parent.line];
                (line.indent + 2, line.next)
            }
            (None, None) => (0, self.text.len()),
        };
        let mut text = String::new();
        if at > 0 && !self.text[..at].ends_with('\n') {
            text.push_str(self.eol);
        }
        for (depth, segment) in rest.iter().enumerate() {
            text.push_str(&" ".repeat(indent + 2 * depth));
            text.push_str(&key(segment));
            text.push(':');
            if depth + 1 == rest.len() {
                text.push(' ');
                text.push_str(value);
            }
            text.push_str(self.eol);
        }
        Splice {
            range: at..at,
            text,
        }
    }

    fn remove(&self, path: &[&str]) -> Result<Option<Splice>, EditError> {
        let steps = self.walk(path)?;
        if steps.len() < path.len() {
            return Ok(None);
        }
        // La única clave de su mapa se lleva a su padre, que quedaría vacío.
        let mut depth = steps.len();
        while depth > 1 && steps[depth - 1].map.entries.len() == 1 {
            depth -= 1;
        }
        Ok(steps[depth - 1].found().map(|entry| Splice {
            range: entry.span(&self.lines),
            text: String::new(),
        }))
    }

    fn rename(&self, path: &[&str], to: &str) -> Result<Option<Splice>, EditError> {
        let steps = self.walk(path)?;
        let entry = steps
            .last()
            .filter(|_| steps.len() == path.len())
            .and_then(|last| last.found().map(|entry| (last, entry)));
        let Some((last, entry)) = entry else {
            return Err(EditError::Missing(path.join(".")));
        };
        if entry.key.key == to {
            return Ok(None);
        }
        if last.map.position(to).is_some() {
            let mut taken = path[..path.len() - 1].to_vec();
            taken.push(to);
            return Err(EditError::Exists(taken.join(".")));
        }
        let line = &self.lines[entry.line];
        Ok(Some(Splice {
            range: line.start + line.indent..entry.key.key_end,
            text: key(to),
        }))
    }
}

#[cfg(test)]
mod tests;
