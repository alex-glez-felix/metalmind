//! Los adjuntos desde los links: el embed de uno recién guardado.

use super::LinkIndex;
use super::complete::shortest_target;
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
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;

    use super::*;
    use crate::index::EntryKind;

    fn path(path: &str) -> VaultPath {
        VaultPath::new(path).unwrap()
    }

    fn index(files: &[(&str, &str)]) -> LinkIndex {
        LinkIndex::from_texts(files.iter().map(|&(file, text)| {
            let file = path(file);
            let kind = EntryKind::of_file(&file);
            (file, kind, text)
        }))
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
}
