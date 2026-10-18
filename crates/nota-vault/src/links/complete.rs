//! Cómo nombrar un archivo desde una nota, para el autocompletado de `[[`.

use super::rename::form;
use super::{LinkIndex, LinkTarget};
use crate::index::EntryKind;
use crate::path::VaultPath;

impl LinkIndex {
    /// El destino más corto con que `[[…]]`, escrito en `source`, llega a
    /// `target` sin empatar: el nombre si alcanza, si no la ruta con las
    /// carpetas que hagan falta. Las notas van sin `.md`. `None` si un
    /// wikilink no puede nombrarlo (un `#`, `|` o `]]` en la ruta) o no es
    /// un archivo del vault.
    #[must_use]
    pub fn wikilink_target(&self, source: &VaultPath, target: &VaultPath) -> Option<String> {
        if !self.names.contains(target) {
            return None;
        }
        let resolved = LinkTarget::Resolved(target.clone());
        let name = target.name();
        let shape = match EntryKind::of_file(target) {
            EntryKind::Note => &name[..name.len() - ".md".len()],
            _ => name,
        };
        form::candidates(shape, source, target)
            .into_iter()
            .find(|candidate| {
                self.names.resolve(source, candidate) == resolved && writable(candidate)
            })
    }
}

/// `[[candidate]]` es un solo link entero hacia `candidate`, sin subruta.
fn writable(candidate: &str) -> bool {
    let link = format!("[[{candidate}]]");
    let scan = nota_syntax::scan(&link);
    matches!(
        scan.links.as_slice(),
        [found] if found.span == (0..link.len())
            && found.subpath.is_none()
            && found.target_text(&link) == candidate
    )
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;

    use super::*;

    fn path(path: &str) -> VaultPath {
        VaultPath::new(path).unwrap()
    }

    fn index(files: &[&str]) -> LinkIndex {
        LinkIndex::build(
            files.iter().map(|file| {
                let file = path(file);
                let kind = EntryKind::of_file(&file);
                (file, kind)
            }),
            |_| Some(String::new()),
        )
    }

    #[test]
    fn the_shortest_name_that_does_not_tie() {
        let index = index(&[
            "a/plan.md",
            "b/plan.md",
            "c/único.md",
            "a/img.png",
            "x#y.md",
        ]);
        let from = |source: &str, target: &str| index.wikilink_target(&path(source), &path(target));
        assert_eq!(from("inicio.md", "c/único.md").as_deref(), Some("único"));
        assert_eq!(from("inicio.md", "a/plan.md").as_deref(), Some("a/plan"));
        assert_eq!(from("a/otra.md", "a/plan.md").as_deref(), Some("plan"));
        assert_eq!(from("inicio.md", "a/img.png").as_deref(), Some("img.png"));
        assert_eq!(from("inicio.md", "x#y.md"), None);
        assert_eq!(from("inicio.md", "no/existe.md"), None);
    }
}
