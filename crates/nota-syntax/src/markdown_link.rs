use std::collections::HashMap;
use std::ops::Range;

use crate::code::line_end;
use crate::link::{Link, LinkSyntax, split_subpath, trim};

/// Cuántos paréntesis anidados admite un destino, como cmark: acota lo que
/// recorre cada intento en una línea llena de `](`.
const MAX_PAREN_DEPTH: usize = 32;

fn is_escape(bytes: &[u8], i: usize) -> bool {
    bytes[i] == b'\\' && bytes.get(i + 1).is_some_and(u8::is_ascii_punctuation)
}

/// Los pares `[` `]` balanceados de la línea en curso, calculados una vez por
/// línea: buscar el cierre desde cada `[` sería cuadrático en una línea llena
/// de corchetes sin cerrar.
#[derive(Default)]
pub(crate) struct Brackets {
    line: Range<usize>,
    closes: HashMap<usize, usize>,
}

impl Brackets {
    fn close(&mut self, text: &str, open: usize) -> Option<usize> {
        if !self.line.contains(&open) {
            self.match_line(text, open);
        }
        self.closes.get(&open).copied()
    }

    fn match_line(&mut self, text: &str, at: usize) {
        let start = text[..at].rfind('\n').map_or(0, |i| i + 1);
        self.line = start..line_end(text, at);
        self.closes.clear();
        let bytes = text.as_bytes();
        let mut opens = Vec::new();
        let mut i = start;
        while i < self.line.end {
            match bytes[i] {
                _ if is_escape(bytes, i) => i += 1,
                b'[' => opens.push(i),
                b']' => {
                    if let Some(open) = opens.pop() {
                        self.closes.insert(open, i);
                    }
                }
                _ => {}
            }
            i += 1;
        }
    }
}

/// Un link markdown reconocido. `link` es `None` si apunta fuera del vault:
/// igual se salta entero para no ver tags en su URL.
pub(crate) struct MarkdownLink {
    pub end: usize,
    pub link: Option<Link>,
}

/// El destino dentro de `(…)`: `<con espacios>` o sin espacios y con
/// paréntesis balanceados. Devuelve su rango y dónde sigue el scan.
fn destination(text: &str, from: usize) -> Option<(Range<usize>, usize)> {
    let bytes = text.as_bytes();
    if bytes.get(from) == Some(&b'<') {
        let start = from + 1;
        let len = text[start..].find(['>', '<', '\n'])?;
        let end = start + len;
        return (bytes[end] == b'>').then_some((start..end, end + 1));
    }
    let mut depth = 0usize;
    let mut i = from;
    while i < bytes.len() {
        match bytes[i] {
            _ if is_escape(bytes, i) => i += 1,
            b'(' if depth == MAX_PAREN_DEPTH => return None,
            b'(' => depth += 1,
            b')' if depth == 0 => break,
            b')' => depth -= 1,
            b if b.is_ascii_whitespace() => break,
            _ => {}
        }
        i += 1;
    }
    let end = i.min(bytes.len());
    Some((from..end, end))
}

/// Tras el destino: un título opcional entre comillas y el `)` de cierre.
fn closing_paren(text: &str, from: usize) -> Option<usize> {
    let bytes = text.as_bytes();
    let mut at = skip_spaces(bytes, from);
    if let Some(&quote @ (b'"' | b'\'')) = bytes.get(at) {
        let title = at + 1;
        let len = text[title..].find([char::from(quote), '\n'])?;
        if bytes[title + len] != quote {
            return None;
        }
        at = skip_spaces(bytes, title + len + 1);
    }
    (bytes.get(at) == Some(&b')')).then_some(at + 1)
}

fn skip_spaces(bytes: &[u8], from: usize) -> usize {
    from + bytes[from..]
        .iter()
        .take_while(|&&b| b == b' ' || b == b'\t')
        .count()
}

/// `scheme:` o `//host`: apunta fuera del vault.
fn is_external(destination: &str) -> bool {
    if destination.starts_with("//") {
        return true;
    }
    destination.split_once(':').is_some_and(|(scheme, _)| {
        let mut chars = scheme.chars();
        chars.next().is_some_and(|c| c.is_ascii_alphabetic())
            && chars.all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.'))
    })
}

/// El link markdown cuyo `[` está en `open`; `start` es el `!` de un embed.
pub(crate) fn markdown_link_at(
    text: &str,
    brackets: &mut Brackets,
    start: usize,
    open: usize,
) -> Option<MarkdownLink> {
    let label_close = brackets.close(text, open)?;
    let paren = label_close + 1;
    if text.as_bytes().get(paren) != Some(&b'(') {
        return None;
    }
    let (dest, dest_end) = destination(text, skip_spaces(text.as_bytes(), paren + 1))?;
    let end = closing_paren(text, dest_end)?;
    if is_external(&text[dest.clone()]) {
        return Some(MarkdownLink { end, link: None });
    }
    let (target, subpath) = split_subpath(text, dest);
    if target.is_empty() && subpath.is_none() {
        return Some(MarkdownLink { end, link: None });
    }
    let alias = trim(text, open + 1..label_close);
    Some(MarkdownLink {
        end,
        link: Some(Link {
            syntax: LinkSyntax::Markdown,
            embed: start != open,
            span: start..end,
            target,
            subpath,
            alias: (!alias.is_empty()).then_some(alias),
        }),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    #[rstest]
    #[case("https://example.com", true)]
    #[case("mailto:a@b.c", true)]
    #[case("obsidian://open?vault=x", true)]
    #[case("//cdn.example.com/a.png", true)]
    #[case("nota.md", false)]
    #[case("carpeta/nota.md", false)]
    #[case("nota.md#a:b", false)]
    fn external_destinations(#[case] destination: &str, #[case] external: bool) {
        assert_eq!(is_external(destination), external);
    }
}
