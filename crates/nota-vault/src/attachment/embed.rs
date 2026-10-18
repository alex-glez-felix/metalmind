//! El `![[...]]` de un adjunto: el destino más corto que resuelve a él, como
//! escribe Obsidian, para que la nota siga siendo legible.

use nota_syntax::{Resolution, resolve};

use crate::index::VaultIndex;
use crate::path::VaultPath;

/// El embed de `path` con el final de ruta más corto que lo resuelve sin
/// ambigüedad entre los archivos de `index` (más `path`, que puede no haber
/// llegado al índice todavía); si ninguno basta, la ruta completa con `/`.
pub(crate) fn embed_text(path: &VaultPath, index: &VaultIndex) -> String {
    let others: Vec<&str> = index
        .files()
        .map(|entry| entry.path.as_str())
        .filter(|other| *other != path.as_str())
        .collect();
    let paths = || others.iter().copied().chain([path.as_str()]);
    let components: Vec<&str> = path.components().collect();
    let target = (0..components.len())
        .rev()
        .map(|start| components[start..].join("/"))
        .find(|suffix| resolve(suffix, paths()) == Resolution::Resolved(path.as_str()))
        .unwrap_or_else(|| format!("/{path}"));
    format!("![[{target}]]")
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;

    use super::*;
    use crate::index::EntryKind;

    fn path(path: &str) -> VaultPath {
        VaultPath::new(path).unwrap()
    }

    fn index(files: &[&str]) -> VaultIndex {
        let mut entries = vec![
            (path("adjuntos"), EntryKind::Folder),
            (path("otra"), EntryKind::Folder),
        ];
        entries.extend(files.iter().map(|file| (path(file), EntryKind::Attachment)));
        VaultIndex::from_entries(entries)
    }

    #[test]
    fn el_nombre_basta_si_es_unico() {
        let embed = embed_text(&path("adjuntos/a.png"), &index(&["otra/b.png"]));
        assert_eq!(embed, "![[a.png]]");
    }

    #[test]
    fn con_otro_igual_mas_arriba_lleva_la_carpeta() {
        let embed = embed_text(&path("adjuntos/a.png"), &index(&["a.png"]));
        assert_eq!(embed, "![[adjuntos/a.png]]");
    }

    #[test]
    fn con_otro_igual_a_la_misma_altura_lleva_la_carpeta() {
        let embed = embed_text(&path("adjuntos/a.png"), &index(&["otra/a.png"]));
        assert_eq!(embed, "![[adjuntos/a.png]]");
    }

    #[test]
    fn en_la_raiz_con_el_mismo_nombre_en_otra_carpeta() {
        let embed = embed_text(&path("a.png"), &index(&["otra/a.png"]));
        assert_eq!(embed, "![[a.png]]");
    }
}
