//! La forma de un destino: cómo escribir la ruta nueva igual que estaba
//! escrita la vieja.

use std::fmt::Write as _;

use nota_syntax::LinkSyntax;

use crate::index::EntryKind;
use crate::links::IndexedLink;
use crate::links::note::subpath;
use crate::path::VaultPath;

/// Destinos para `target`, escrito desde `source`, en la forma de `original`
/// y de más corto a más largo. El último, la ruta completa desde la raíz,
/// es el que menos puede empatar.
pub(in crate::links) fn candidates(
    original: &str,
    source: &VaultPath,
    target: &VaultPath,
) -> Vec<String> {
    let original = original.trim();
    let full = target.as_str();
    let without_md = EntryKind::of_file(target) == EntryKind::Note
        && !last_component(original).to_lowercase().ends_with(".md");
    let mut paths = vec![full];
    if without_md {
        paths.insert(0, &full[..full.len() - ".md".len()]);
    }
    let mut forms = Vec::new();
    for path in paths {
        if original.starts_with("./") || original.starts_with("../") {
            forms.push(relative(source, path));
        } else if original.starts_with('/') {
            forms.push(format!("/{path}"));
        } else {
            let parts: Vec<&str> = path.split('/').collect();
            let written = original.split('/').count().clamp(1, parts.len());
            forms.extend((written..=parts.len()).map(|n| parts[parts.len() - n..].join("/")));
        }
    }
    forms.push(format!("/{full}"));
    forms
}

fn last_component(path: &str) -> &str {
    path.rsplit('/').next().unwrap_or_default()
}

/// `path` desde la carpeta de `source`, siempre con `./` o `../` delante.
fn relative(source: &VaultPath, path: &str) -> String {
    let folder = source.parent().unwrap_or_default();
    let from: Vec<&str> = folder.components().collect();
    let to: Vec<&str> = path.split('/').collect();
    let common = from
        .iter()
        .zip(&to[..to.len() - 1])
        .take_while(|(a, b)| a == b)
        .count();
    let rest = to[common..].join("/");
    match from.len() - common {
        0 => format!("./{rest}"),
        ups => format!("{}{rest}", "../".repeat(ups)),
    }
}

/// `candidate` escrito en la sintaxis de `link`, si al volver a escanearlo
/// da un solo link entero con ese destino y la misma subruta.
pub(super) fn write(link: &IndexedLink, candidate: &str) -> Option<String> {
    let written = match (link.syntax, link.bracketed) {
        (LinkSyntax::Wiki, _) => candidate.to_owned(),
        (LinkSyntax::Markdown, true) => escape_bracketed(candidate),
        (LinkSyntax::Markdown, false) => escape_plain(candidate),
    };
    let context = &link.context.text;
    let span = link.in_context(&link.span);
    let target = link.in_context(&link.target_span);
    let snippet = format!(
        "{}{written}{}",
        &context[span.start..target.start],
        &context[target.end..span.end]
    );
    let rescanned = nota_syntax::scan(&snippet);
    let [found] = rescanned.links.as_slice() else {
        return None;
    };
    let same = found.span == (0..snippet.len())
        && found.target_text(&snippet) == candidate
        && subpath(&snippet, found) == link.subpath;
    same.then_some(written)
}

/// Dentro de `<…>` valen los espacios, no los saltos de línea ni otros
/// controles, que van como `%0A`; `%` se escapa porque el destino se
/// decodifica igual.
fn escape_bracketed(path: &str) -> String {
    let mut out = String::with_capacity(path.len());
    for c in path.chars() {
        match c {
            '%' => out.push_str("%25"),
            '\\' | '<' | '>' | '#' => {
                out.push('\\');
                out.push(c);
            }
            _ if c.is_control() => percent_encode(&mut out, c),
            _ => out.push(c),
        }
    }
    out
}

/// Sin `<…>`, los espacios y los controles van como `%20`, y lo que cortaría
/// el destino, con `\`.
fn escape_plain(path: &str) -> String {
    let mut out = String::with_capacity(path.len());
    for c in path.chars() {
        match c {
            '%' => out.push_str("%25"),
            '\\' | '(' | ')' | '<' | '>' | '#' => {
                out.push('\\');
                out.push(c);
            }
            _ if c.is_whitespace() || c.is_control() => percent_encode(&mut out, c),
            _ => out.push(c),
        }
    }
    out
}

fn percent_encode(out: &mut String, c: char) {
    let mut bytes = [0; 4];
    for byte in c.encode_utf8(&mut bytes).bytes() {
        let _ = write!(out, "%{byte:02X}");
    }
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;
    use rstest::rstest;

    use super::*;

    fn path(path: &str) -> VaultPath {
        VaultPath::new(path).unwrap()
    }

    #[rstest]
    #[case("nota", "a/x.md", "b/nueva.md", &["nueva", "b/nueva", "nueva.md", "b/nueva.md", "/b/nueva.md"])]
    #[case("a/nota.md", "x.md", "b/c/nueva.md", &["c/nueva.md", "b/c/nueva.md", "/b/c/nueva.md"])]
    #[case("/nota", "x.md", "b/nueva.md", &["/b/nueva", "/b/nueva.md", "/b/nueva.md"])]
    #[case("./nota", "a/x.md", "a/nueva.md", &["./nueva", "./nueva.md", "/a/nueva.md"])]
    #[case("../nota", "a/b/x.md", "nueva.md", &["../../nueva", "../../nueva.md", "/nueva.md"])]
    #[case("img.png", "x.md", "a/foto.png", &["foto.png", "a/foto.png", "/a/foto.png"])]
    fn forms_keep_the_shape(
        #[case] original: &str,
        #[case] source: &str,
        #[case] target: &str,
        #[case] expected: &[&str],
    ) {
        assert_eq!(candidates(original, &path(source), &path(target)), expected);
    }

    #[test]
    fn relative_paths_climb_only_what_they_need() {
        assert_eq!(relative(&path("a/b/x.md"), "a/c/y.md"), "../c/y.md");
        assert_eq!(relative(&path("x.md"), "a/y.md"), "./a/y.md");
        assert_eq!(relative(&path("a/x.md"), "a"), "../a");
    }

    #[rstest]
    #[case("Mi Nota.md", "Mi%20Nota.md")]
    #[case("50% (final).md", "50%25%20\\(final\\).md")]
    #[case("canción#1.md", "canción\\#1.md")]
    fn plain_markdown_escapes(#[case] path: &str, #[case] written: &str) {
        assert_eq!(escape_plain(path), written);
    }

    #[test]
    fn bracketed_destinations_encode_line_breaks() {
        assert_eq!(escape_bracketed("Mi nota\nnueva.md"), "Mi nota%0Anueva.md");
    }
}
