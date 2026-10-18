//! Los adjuntos desde los links: el embed de uno recién guardado y los que
//! ninguna nota usa (06 §6: nada los borra solo, así que se acumulan).

use nota_syntax::path_keys;

use super::complete::shortest_target;
use super::{LinkIndex, LinkTarget};
use crate::index::EntryKind;
use crate::path::VaultPath;

impl LinkIndex {
    /// El `![[…]]` con que `source` embebe `file`, que acaba de crearse y
    /// puede no haber llegado al índice. `None` si un wikilink no puede
    /// nombrarlo (un `#`, `|` o `]]` en la ruta).
    pub(crate) fn embed_for_new(&self, source: &VaultPath, file: &VaultPath) -> Option<String> {
        let names = self.names.with_file(file);
        shortest_target(source, file, |candidate| names.resolve(source, candidate))
            .map(|target| format!("![[{target}]]"))
    }

    /// Los adjuntos de `folder` (y sus subcarpetas; la raíz es todo el vault)
    /// que ningún link usa, por ruta. Un link sin `!` también cuenta: borrar
    /// un PDF enlazado rompe el link igual que un embed. Una nota que no se
    /// pudo leer no tiene links, así que no protege lo que use.
    #[must_use]
    pub fn orphan_attachments(&self, folder: &VaultPath) -> Vec<VaultPath> {
        self.names
            .within(folder)
            .into_iter()
            .filter(|file| EntryKind::of_file(file) == EntryKind::Attachment)
            .filter(|file| !self.is_used(file))
            .collect()
    }

    /// Si algún link resuelve a `file` o lo tiene entre sus empatadas: ante
    /// la duda, se conserva.
    fn is_used(&self, file: &VaultPath) -> bool {
        self.incoming.contains_key(file)
            || path_keys(file.as_str())
                .filter_map(|key| self.by_key.get(&key))
                .flatten()
                .flat_map(|note| self.outgoing(note))
                .any(|link| matches!(&link.target, LinkTarget::Ambiguous(tied) if tied.contains(file)))
    }
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;

    use super::*;

    fn path(path: &str) -> VaultPath {
        VaultPath::new(path).unwrap()
    }

    fn index(files: &[(&str, &str)]) -> LinkIndex {
        LinkIndex::build(
            files.iter().map(|(file, _)| {
                let file = path(file);
                let kind = EntryKind::of_file(&file);
                (file, kind)
            }),
            |note| {
                files
                    .iter()
                    .find(|(file, _)| *file == note.as_str())
                    .map(|(_, text)| (*text).to_owned())
            },
        )
    }

    fn embed(index: &LinkIndex, source: &str, file: &str) -> Option<String> {
        index.embed_for_new(&path(source), &path(file))
    }

    #[test]
    fn el_embed_mas_corto_que_no_empata() {
        let index = index(&[("adjuntos/a.png", ""), ("otra/a.png", ""), ("a.md", "")]);
        assert_eq!(
            embed(&index, "nota.md", "adjuntos/b.png").as_deref(),
            Some("![[b.png]]")
        );
        assert_eq!(
            embed(&index, "nota.md", "adjuntos/a.png").as_deref(),
            Some("![[adjuntos/a.png]]")
        );
        assert_eq!(
            embed(&index, "adjuntos/nota.md", "adjuntos/a.png").as_deref(),
            Some("![[a.png]]"),
            "desde su carpeta, el nombre desempata"
        );
        assert_eq!(
            embed(&index, "nota.md", "a.png").as_deref(),
            Some("![[a.png]]"),
            "el menos profundo gana"
        );
        assert_eq!(
            embed(&index, "nota.md", "a#b/a.png"),
            None,
            "empata y la carpeta no se puede escribir"
        );
    }

    #[test]
    fn huerfanos_de_una_carpeta() {
        let index = index(&[
            ("adjuntos/usado.png", ""),
            ("adjuntos/sub/con-ruta.png", ""),
            ("adjuntos/informe.pdf", ""),
            ("adjuntos/en-codigo.png", ""),
            ("adjuntos/ambiguo.png", ""),
            ("otra/ambiguo.png", ""),
            ("adjuntos/relativo.png", ""),
            ("adjuntos/suelto.png", ""),
            ("adjuntos/nota.md", "![[./relativo.png]] ![[./punto.png/.]]"),
            ("adjuntos/punto.png", ""),
            ("fuera/suelto.png", ""),
            (
                "a.md",
                "![[usado.png]] ![[sub/con-ruta.png|300]] `![[en-codigo.png]]` ![[ambiguo.png]]",
            ),
            ("otra/b.md", "[informe](../adjuntos/informe.pdf)"),
        ]);
        assert_eq!(
            index.orphan_attachments(&path("adjuntos")),
            [path("adjuntos/en-codigo.png"), path("adjuntos/suelto.png")]
        );
        assert_eq!(
            index.orphan_attachments(&VaultPath::root()),
            [
                path("adjuntos/en-codigo.png"),
                path("adjuntos/suelto.png"),
                path("fuera/suelto.png"),
            ]
        );
        assert_eq!(
            index.orphan_attachments(&path("no-existe")),
            Vec::<VaultPath>::new()
        );
    }
}
