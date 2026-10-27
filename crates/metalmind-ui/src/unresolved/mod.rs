//! `space u`, sin GPUI: los links del vault que no llevan a ningún archivo,
//! como filas del picker. Cada uno se busca por cómo está escrito y dice a
//! la derecha dónde está.

use std::sync::Arc;

use metalmind_vault::{LinkIndex, VaultPath};

use crate::picker::PickerItem;
use crate::workspace::LinePosition;

/// Un link sin destino.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UnresolvedLink {
    /// El link entero como está escrito: `[[Nota]]`, `![[foto.png]]`,
    /// `[x](falta.md)`.
    pub written: String,
    pub source: VaultPath,
    pub at: LinePosition,
    /// `ruta:línea`, desde uno.
    detail: String,
}

impl AsRef<str> for UnresolvedLink {
    fn as_ref(&self) -> &str {
        &self.written
    }
}

impl PickerItem for UnresolvedLink {
    fn detail(&self) -> Option<&str> {
        Some(&self.detail)
    }
}

/// Los de `index`, por nota y por posición.
#[must_use]
pub fn unresolved_links(index: &LinkIndex) -> Arc<[UnresolvedLink]> {
    index
        .unresolved()
        .into_iter()
        .map(|found| {
            let link = found.link;
            let context = &link.context;
            UnresolvedLink {
                written: context.text[context.link.clone()].to_owned(),
                source: found.source.clone(),
                at: LinePosition {
                    line: link.line,
                    column: link.column,
                },
                detail: format!("{}:{}", found.source.as_str(), link.line + 1),
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use metalmind_vault::EntryKind;
    use pretty_assertions::assert_eq;

    use super::*;

    fn path(path: &str) -> VaultPath {
        VaultPath::new(path).unwrap()
    }

    #[test]
    fn cada_link_sin_destino_con_donde_esta() {
        let notes: HashMap<VaultPath, &str> = [
            (
                "a.md",
                "ver [[b]] y [[falta]]\n\n![[foto.png]] y [x](otra/falta.md)\n",
            ),
            ("b.md", "nada que falte: [[a]]\n"),
            ("c/d.md", "uno\n[[falta#Meta|el alias]]\n"),
        ]
        .into_iter()
        .map(|(name, text)| (path(name), text))
        .collect();
        let index = LinkIndex::build(
            notes.keys().map(|name| (name.clone(), EntryKind::Note)),
            |name| notes.get(name).map(|text| (*text).to_owned()),
        );
        let links = unresolved_links(&index);
        let shown: Vec<(&str, &str, Option<&str>, usize)> = links
            .iter()
            .map(|link| {
                (
                    link.as_ref(),
                    link.source.as_str(),
                    link.detail(),
                    link.at.column,
                )
            })
            .collect();
        assert_eq!(
            shown,
            [
                ("[[falta]]", "a.md", Some("a.md:1"), 12),
                ("![[foto.png]]", "a.md", Some("a.md:3"), 0),
                ("[x](otra/falta.md)", "a.md", Some("a.md:3"), 16),
                ("[[falta#Meta|el alias]]", "c/d.md", Some("c/d.md:2"), 0),
            ]
        );
    }
}
