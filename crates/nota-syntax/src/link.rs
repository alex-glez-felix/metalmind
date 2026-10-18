use std::borrow::Cow;
use std::ops::Range;

/// Cómo está escrito el link.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LinkSyntax {
    /// `[[nota]]`
    Wiki,
    /// `[texto](nota.md)`
    Markdown,
}

/// Lo que va tras el `#` del destino.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Subpath {
    /// `#Heading`; los anidados (`#H1#H2`) quedan juntos.
    Heading(Range<usize>),
    /// `#^bloque`, sin el `^`.
    Block(Range<usize>),
}

/// Un link o embed hacia una nota o archivo del vault.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Link {
    pub syntax: LinkSyntax,
    /// `![[…]]` o `![…](…)`.
    pub embed: bool,
    /// Todo el link, desde el `!` o el primer `[`.
    pub span: Range<usize>,
    /// El nombre o ruta de la nota. Vacío en `[[#Heading]]`: la misma nota.
    pub target: Range<usize>,
    pub subpath: Option<Subpath>,
    /// Lo que se muestra: el alias del wikilink o el texto del link markdown.
    /// En un embed de imagen es el tamaño (`![[img.png|300]]`).
    pub alias: Option<Range<usize>>,
}

impl Link {
    /// El texto de una de las partes del link. En un link markdown se
    /// decodifican los `%20`: un destino markdown no puede llevar espacios.
    #[must_use]
    pub fn text<'t>(&self, text: &'t str, part: Range<usize>) -> Cow<'t, str> {
        let raw = &text[part];
        match self.syntax {
            LinkSyntax::Wiki => Cow::Borrowed(raw),
            LinkSyntax::Markdown => percent_decode(raw),
        }
    }
}

fn percent_decode(raw: &str) -> Cow<'_, str> {
    if !raw.contains('%') {
        return Cow::Borrowed(raw);
    }
    let bytes = raw.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        let escaped = bytes
            .get(i + 1..i + 3)
            .filter(|_| bytes[i] == b'%')
            .and_then(|hex| std::str::from_utf8(hex).ok())
            .and_then(|hex| u8::from_str_radix(hex, 16).ok());
        if let Some(byte) = escaped {
            out.push(byte);
            i += 3;
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    String::from_utf8(out).map_or(Cow::Borrowed(raw), Cow::Owned)
}

pub(crate) fn trim(text: &str, range: Range<usize>) -> Range<usize> {
    let part = &text[range.clone()];
    let start = range.start + (part.len() - part.trim_start().len());
    let end = range.end - (part.len() - part.trim_end().len());
    start..end.max(start)
}

/// Separa `nota#Heading` o `nota#^bloque` en destino y subruta.
pub(crate) fn split_subpath(text: &str, range: Range<usize>) -> (Range<usize>, Option<Subpath>) {
    let Some(hash) = text[range.clone()].find('#').map(|i| range.start + i) else {
        return (trim(text, range), None);
    };
    let target = trim(text, range.start..hash);
    let rest = hash + 1..range.end;
    let subpath = if text[rest.clone()].starts_with('^') {
        Subpath::Block(trim(text, rest.start + 1..rest.end))
    } else {
        Subpath::Heading(trim(text, rest))
    };
    let empty = match &subpath {
        Subpath::Heading(r) | Subpath::Block(r) => r.is_empty(),
    };
    (target, (!empty).then_some(subpath))
}

/// El `]]` del wikilink cuyo contenido empieza en `body`, si llega antes que
/// un salto de línea o que otro `[[`.
fn wikilink_close(text: &str, body: usize) -> Option<usize> {
    let bytes = text.as_bytes();
    for i in body..bytes.len().saturating_sub(1) {
        match (bytes[i], bytes[i + 1]) {
            (b']', b']') => return Some(i),
            (b'[', b'[') | (b'\n', _) => return None,
            _ => {}
        }
    }
    None
}

/// El wikilink cuyo `[[` está en `open`. `start` es `open` o el `!` de un
/// embed justo antes. No cruza saltos de línea ni contiene otro `[[`.
pub(crate) fn wikilink_at(text: &str, start: usize, open: usize) -> Option<Link> {
    let body = open + 2;
    let close = wikilink_close(text, body)?;
    // `\|` también separa el alias: así se escribe dentro de una tabla.
    let (destination, alias) = match text[body..close].find('|').map(|i| body + i) {
        Some(bar) => {
            let end = if text[..bar].ends_with('\\') {
                bar - 1
            } else {
                bar
            };
            let alias = trim(text, bar + 1..close);
            (body..end, (!alias.is_empty()).then_some(alias))
        }
        None => (body..close, None),
    };
    let (target, subpath) = split_subpath(text, destination);
    if target.is_empty() && subpath.is_none() {
        return None;
    }
    Some(Link {
        syntax: LinkSyntax::Wiki,
        embed: start != open,
        span: start..close + 2,
        target,
        subpath,
        alias,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;
    use rstest::rstest;

    #[rstest]
    #[case("Mi%20Nota.md", "Mi Nota.md")]
    #[case("sin-escapes.md", "sin-escapes.md")]
    #[case("canci%C3%B3n", "canción")]
    #[case("roto%2", "roto%2")]
    #[case("%FF", "%FF")]
    fn percent_decoding(#[case] raw: &str, #[case] decoded: &str) {
        assert_eq!(percent_decode(raw), decoded);
    }
}
