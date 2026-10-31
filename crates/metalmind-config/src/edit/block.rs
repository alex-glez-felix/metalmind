//! Los mapas en bloque de un YAML, línea por línea: dónde empieza y termina
//! cada clave y su valor, para cambiar una sin tocar las demás.

use std::ops::Range;

/// Una línea del texto, sin su salto.
#[derive(Debug)]
pub(super) struct Line {
    pub start: usize,
    /// Después del salto; el fin del texto en la última línea.
    pub next: usize,
    pub indent: usize,
    pub kind: Kind,
}

#[derive(Debug)]
pub(super) enum Kind {
    /// Vacía, en blanco o solo un comentario: no es de ninguna clave.
    Filler,
    Key(KeyLine),
    /// Algo que no es una clave de un mapa en bloque: un ítem de lista, un
    /// `---`, la continuación de un valor.
    Other,
}

#[derive(Clone, Debug)]
pub(super) struct KeyLine {
    pub key: String,
    /// Dónde termina la clave como está escrita, comillas incluidas.
    pub key_end: usize,
    /// El `:` que la separa de su valor.
    pub colon: usize,
    /// El valor en la misma línea, sin espacios ni comentario; `None` si no
    /// hay.
    pub value: Option<Range<usize>>,
}

impl Line {
    pub(super) fn is_content(&self) -> bool {
        !matches!(self.kind, Kind::Filler)
    }
}

pub(super) fn lines(text: &str) -> Vec<Line> {
    let mut lines = Vec::new();
    let mut start = 0;
    while start < text.len() {
        let (end, next) = match text[start..].find('\n') {
            Some(at) => (start + at, start + at + 1),
            None => (text.len(), text.len()),
        };
        let end = if text[start..end].ends_with('\r') {
            end - 1
        } else {
            end
        };
        lines.push(line(text, start, end, next));
        start = next;
    }
    lines
}

fn line(text: &str, start: usize, end: usize, next: usize) -> Line {
    let raw = &text[start..end];
    let content = raw.trim_start_matches(' ');
    let indent = raw.len() - content.len();
    let kind = if content.trim().is_empty() || content.starts_with('#') {
        Kind::Filler
    } else {
        key_line(text, start + indent, end).map_or(Kind::Other, Kind::Key)
    };
    Line {
        start,
        next,
        indent,
        kind,
    }
}

/// `clave: valor`, con la clave simple o entre comillas.
fn key_line(text: &str, at: usize, end: usize) -> Option<KeyLine> {
    let content = &text[at..end];
    let (key, written) = match content.chars().next()? {
        '"' => double_quoted(content)?,
        '\'' => single_quoted(content)?,
        _ => plain_key(content)?,
    };
    let after = content[written..].trim_start_matches([' ', '\t']);
    let rest = after.strip_prefix(':')?;
    if !(rest.is_empty() || rest.starts_with([' ', '\t'])) {
        return None;
    }
    let colon = end - after.len();
    Some(KeyLine {
        key,
        key_end: at + written,
        colon,
        value: inline_value(text, colon + 1, end),
    })
}

/// Una clave sin comillas: hasta el primer `:` seguido de espacio o del fin.
fn plain_key(content: &str) -> Option<(String, usize)> {
    let first = content.chars().next()?;
    if "-?:,[]{}#&*!|>%@`".contains(first) || content.starts_with("--- ") || content == "---" {
        return None;
    }
    let colon = content
        .char_indices()
        .find(|&(at, c)| {
            c == ':'
                && content[at + 1..]
                    .chars()
                    .next()
                    .is_none_or(|next| next == ' ' || next == '\t')
        })
        .map(|(at, _)| at)?;
    let key = content[..colon].trim_end();
    if key.contains(" #") || key.contains("\t#") {
        return None;
    }
    Some((key.to_owned(), key.len()))
}

/// Solo los escapes `\"` y `\\`: con otros, la clave se cambia a mano.
fn double_quoted(content: &str) -> Option<(String, usize)> {
    let mut key = String::new();
    let mut chars = content.char_indices().skip(1);
    while let Some((at, c)) = chars.next() {
        match c {
            '"' => return Some((key, at + 1)),
            '\\' => match chars.next()?.1 {
                escaped @ ('"' | '\\') => key.push(escaped),
                _ => return None,
            },
            c => key.push(c),
        }
    }
    None
}

fn single_quoted(content: &str) -> Option<(String, usize)> {
    let mut key = String::new();
    let mut chars = content.char_indices().skip(1).peekable();
    while let Some((at, c)) = chars.next() {
        if c != '\'' {
            key.push(c);
        } else if chars.peek().is_some_and(|&(_, next)| next == '\'') {
            key.push('\'');
            chars.next();
        } else {
            return Some((key, at + 1));
        }
    }
    None
}

/// El valor que sigue al `:`, sin el comentario que lo cierre.
fn inline_value(text: &str, from: usize, end: usize) -> Option<Range<usize>> {
    let rest = &text[from..end];
    let trimmed = rest.trim_start_matches([' ', '\t']);
    if trimmed.is_empty() || trimmed.starts_with('#') {
        return None;
    }
    let start = end - trimmed.len();
    let len = match trimmed.chars().next()? {
        '"' => double_quoted(trimmed).map_or(trimmed.len(), |(_, len)| len),
        '\'' => single_quoted(trimmed).map_or(trimmed.len(), |(_, len)| len),
        '[' | '{' => trimmed.trim_end().len(),
        _ => [" #", "\t#"]
            .iter()
            .filter_map(|comment| trimmed.find(comment))
            .min()
            .unwrap_or(trimmed.len()),
    };
    let value = trimmed[..len].trim_end();
    Some(start..start + value.len())
}

/// Una clave de un mapa en bloque, con las líneas de su valor.
#[derive(Debug)]
pub(super) struct Entry {
    /// El índice de su línea.
    pub line: usize,
    pub key: KeyLine,
    /// Las líneas de su valor que siguen a la de la clave, hasta la última
    /// con contenido: los comentarios y blancos de después no son suyos.
    pub body: Range<usize>,
}

/// Un mapa en bloque: sus claves en orden, todas con la misma sangría.
#[derive(Debug)]
pub(super) struct Map {
    pub indent: usize,
    pub entries: Vec<Entry>,
}

/// Lo que no es un mapa en bloque: un mapa de flujo, una lista, un texto.
#[derive(Debug, PartialEq, Eq)]
pub(super) struct NotBlockMap;

impl Map {
    /// El mapa en las líneas `region`; vacío si no tienen contenido.
    pub(super) fn read(lines: &[Line], region: Range<usize>) -> Result<Self, NotBlockMap> {
        let mut content = region.clone().filter(|&at| lines[at].is_content());
        let Some(first) = content.next() else {
            return Ok(Self {
                indent: 0,
                entries: Vec::new(),
            });
        };
        let indent = lines[first].indent;
        let mut entries: Vec<Entry> = Vec::new();
        for at in std::iter::once(first).chain(content) {
            let line = &lines[at];
            match (&line.kind, line.indent.cmp(&indent)) {
                (Kind::Key(key), std::cmp::Ordering::Equal) => entries.push(Entry {
                    line: at,
                    key: key.clone(),
                    body: at + 1..at + 1,
                }),
                (_, std::cmp::Ordering::Greater) => {
                    let entry = entries.last_mut().ok_or(NotBlockMap)?;
                    entry.body.end = at + 1;
                }
                _ => return Err(NotBlockMap),
            }
        }
        Ok(Self { indent, entries })
    }

    pub(super) fn position(&self, key: &str) -> Option<usize> {
        self.entries.iter().position(|entry| entry.key.key == key)
    }
}

impl Entry {
    pub(super) fn has_body(&self) -> bool {
        !self.body.is_empty()
    }

    /// Desde el principio de su línea hasta después del salto de la última
    /// de su valor.
    pub(super) fn span(&self, lines: &[Line]) -> Range<usize> {
        let last = if self.has_body() {
            self.body.end - 1
        } else {
            self.line
        };
        lines[self.line].start..lines[last].next
    }
}
