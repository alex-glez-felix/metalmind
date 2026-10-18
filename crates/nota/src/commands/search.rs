//! `nota search`: por ahora, solo fuzzy sobre las rutas del vault. Los tags
//! llegan con `TagQuery` (05 §4) y el texto completo con M6.

use std::borrow::Cow;

use nota_search::{FuzzyMatcher, Query};
use nota_vault::{EntryKind, VaultPath};
use serde::Serialize;

use super::{CliError, print, root_text, vault_root};
use crate::cli::SearchArgs;

#[derive(Serialize)]
struct Found<'a> {
    vault: Cow<'a, str>,
    query: &'a str,
    results: Vec<Hit<'a>>,
}

#[derive(Serialize)]
struct Hit<'a> {
    path: &'a VaultPath,
    kind: EntryKind,
}

/// Imprime las rutas que coinciden, de la mejor a la peor.
///
/// # Errors
/// Si la consulta tiene tags, que todavía no se buscan, o no hay vault.
pub fn search(args: &SearchArgs) -> Result<(), CliError> {
    if let Some(tag) = args.query.split_whitespace().find(|word| is_tag_term(word)) {
        return Err(CliError::Usage(format!(
            "`{tag}`: la búsqueda por tags todavía no existe; por ahora solo se buscan rutas"
        )));
    }
    let vault = vault_root(&args.common)?;
    let index = vault.files();
    let mut files: Vec<_> = index.files().collect();
    // El índice no tiene orden; a igual puntaje gana el orden de entrada.
    files.sort_unstable_by(|a, b| a.path.cmp(b.path));
    let candidates: Vec<&str> = files.iter().map(|entry| entry.path.as_str()).collect();
    let mut matches = FuzzyMatcher::paths().rank(&Query::new(&args.query), &candidates);
    matches.truncate(args.limit);
    let results: Vec<Hit<'_>> = matches
        .iter()
        .map(|found| Hit {
            path: files[found.index].path,
            kind: files[found.index].kind,
        })
        .collect();
    let found = Found {
        vault: root_text(vault.path()),
        query: &args.query,
        results,
    };
    print(args.common.json, &found, |out| {
        found
            .results
            .iter()
            .try_for_each(|hit| writeln!(out, "{}", printable(hit.path.as_str())))
    })
}

/// Un nombre con saltos de línea o secuencias de escape no puede partir la
/// lista ni llegar crudo a la terminal: esos caracteres salen como `\n`,
/// `\u{1b}`, … (`--json` siempre los da exactos).
fn printable(path: &str) -> Cow<'_, str> {
    if path.chars().any(char::is_control) {
        let escaped = path.chars().map(|c| {
            if c.is_control() {
                c.escape_default().to_string()
            } else {
                c.to_string()
            }
        });
        Cow::Owned(escaped.collect())
    } else {
        Cow::Borrowed(path)
    }
}

/// `#tag` y `-#tag` son de la consulta de tags, que no se puede adivinar con fuzzy.
fn is_tag_term(word: &str) -> bool {
    word.strip_prefix('-').unwrap_or(word).starts_with('#')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn los_caracteres_de_control_se_escapan() {
        assert_eq!(printable("a/b ñ.md"), "a/b ñ.md");
        assert_eq!(printable("a\nb.md"), "a\\nb.md");
        assert_eq!(printable("\u{1b}[31mx.md"), "\\u{1b}[31mx.md");
    }

    #[test]
    fn reconoce_los_terminos_de_tags() {
        assert!(is_tag_term("#rust"));
        assert!(is_tag_term("-#archivado"));
        assert!(!is_tag_term("rust"));
        assert!(!is_tag_term("c#"));
        assert!(!is_tag_term("-x"));
    }
}
