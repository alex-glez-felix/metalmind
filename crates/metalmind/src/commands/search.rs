//! `metalmind search`: fuzzy sobre las rutas del vault y, como en `space f`,
//! la consulta de tags (05 §4). El texto completo llega con M6.

use std::borrow::Cow;
use std::collections::HashSet;

use metalmind_search::{FuzzyMatcher, Query};
use metalmind_vault::tags::TagQuery;
use metalmind_vault::{Entry, EntryKind, VaultIndex, VaultOptions, VaultPath, VaultRoot};
use serde::Serialize;

use super::{CliError, Config, print, root_text};
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

/// Imprime las rutas que coinciden, de la mejor a la peor. Con tags en la
/// consulta, solo las notas que la cumplen; el resto de las palabras va al
/// fuzzy.
///
/// # Errors
/// Si la consulta tiene un tag inválido o no hay vault.
pub fn search(args: &SearchArgs) -> Result<(), CliError> {
    let parsed = TagQuery::parse(&args.query);
    if let Some(invalid) = parsed.invalid.first() {
        return Err(CliError::Usage(format!("`{invalid}` no es un tag válido")));
    }
    let vault = Config::load().vault_root(&args.common)?;
    let index = vault.files();
    let mut files: Vec<Entry<'_>> = index.files().collect();
    let fuzzy = if parsed.query.is_empty() {
        Cow::Borrowed(args.query.as_str())
    } else {
        let tagged = tagged(&vault, &index, &parsed.query);
        files.retain(|entry| tagged.contains(entry.path));
        Cow::Owned(parsed.words.join(" "))
    };
    // El índice no tiene orden; a igual puntaje gana el orden de entrada.
    files.sort_unstable_by(|a, b| a.path.cmp(b.path));
    let candidates: Vec<&str> = files.iter().map(|entry| entry.path.as_str()).collect();
    let mut matches = FuzzyMatcher::paths().rank(&Query::new(&fuzzy), &candidates);
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

/// Las notas que cumplen `query`. Solo aquí se leen las notas: una consulta
/// sin tags no las abre.
fn tagged(vault: &VaultRoot, index: &VaultIndex, query: &TagQuery) -> HashSet<VaultPath> {
    let tags = vault.tags(index, VaultOptions::default().tags_folder);
    query.notes(&tags).into_iter().cloned().collect()
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn los_caracteres_de_control_se_escapan() {
        assert_eq!(printable("a/b ñ.md"), "a/b ñ.md");
        assert_eq!(printable("a\nb.md"), "a\\nb.md");
        assert_eq!(printable("\u{1b}[31mx.md"), "\\u{1b}[31mx.md");
    }
}
