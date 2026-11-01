//! `metalmind tags`: los tags del vault como los cuenta el panel (05 §2–§3),
//! desde el mismo índice.

use std::borrow::Cow;
use std::io::{self, Write};

use metalmind_vault::VaultOptions;
use metalmind_vault::tags::{Scope, TagKey, TagNode, TagTree};
use serde::Serialize;

use super::{CliError, Config, print, root_text};
use crate::cli::TagsArgs;

#[derive(Serialize)]
struct Listed<'a> {
    vault: Cow<'a, str>,
    tags: Vec<Listing<'a>>,
}

#[derive(Serialize)]
struct Listing<'a> {
    name: &'a str,
    count: usize,
    parents: Vec<&'a str>,
}

/// Imprime cada tag con su forma establecida, cuántas notas distintas trae
/// (con sus alias y descendientes) y sus padres, en el orden del panel.
///
/// # Errors
/// Si no hay vault o no se puede escribir la salida.
pub fn tags(args: &TagsArgs) -> Result<(), CliError> {
    let vault = Config::load().vault_root(&args.common)?;
    let tags = vault.tags(&vault.files(), VaultOptions::default().tags_folder);
    let tree = tags.tree(&Scope::default().filter(&tags));
    let listed = Listed {
        vault: root_text(vault.path()),
        tags: tree
            .flatten()
            .into_iter()
            .map(|node| listing(&tree, node, tags.graph().parents(&node.key)))
            .collect(),
    };
    print(args.common.json, &listed, |out| {
        write_text(out, &listed.tags)
    })
}

fn listing<'t>(tree: &'t TagTree, node: &'t TagNode, parents: &'t [TagKey]) -> Listing<'t> {
    Listing {
        name: &node.name,
        count: node.count,
        parents: parents
            .iter()
            .map(|key| tree.node(key).map_or(key.as_str(), |parent| &parent.name))
            .collect(),
    }
}

/// `  3  gpui  ← Rust, ui`: el conteo alineado a la derecha y los padres al final.
fn write_text(out: &mut dyn Write, tags: &[Listing<'_>]) -> io::Result<()> {
    let width = tags
        .iter()
        .map(|tag| tag.count.to_string().len())
        .max()
        .unwrap_or_default();
    for tag in tags {
        write!(out, "{:>width$}  {}", tag.count, tag.name)?;
        if !tag.parents.is_empty() {
            write!(out, "  ← {}", tag.parents.join(", "))?;
        }
        writeln!(out)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn el_texto_alinea_los_conteos_y_pone_los_padres_al_final() {
        let tags = [
            Listing {
                name: "lenguajes",
                count: 12,
                parents: vec![],
            },
            Listing {
                name: "gpui",
                count: 3,
                parents: vec!["Rust", "ui"],
            },
        ];
        let mut out = Vec::new();
        write_text(&mut out, &tags).unwrap();
        assert_eq!(
            String::from_utf8(out).unwrap(),
            "12  lenguajes\n 3  gpui  ← Rust, ui\n"
        );
    }
}
