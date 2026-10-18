//! Lo que una nota aporta a los tags: sus ocurrencias y, si es una página de
//! tag, su metadata.

use std::ops::Range;

use nota_syntax::{Frontmatter, Value};

use crate::changes::content_hash;
use crate::path::VaultPath;
use crate::tags::key::{TagKey, tag_name};
use crate::tags::page::TagPage;

/// De dónde salió una ocurrencia.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TagSource {
    /// Un `#tag` del cuerpo.
    Inline,
    /// Un valor de `tags:` (o de `aliases`/`parents` en una página).
    Frontmatter(FrontmatterSpot),
}

/// Cómo reescribir o quitar un valor del frontmatter sin tocar el resto.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FrontmatterSpot {
    /// Si es `false`, el YAML escribe el valor con escapes o en varias
    /// líneas: el rango cubre el escalar entero y se reescribe entre comillas.
    pub literal: bool,
    /// Lo que se borra para sacar el valor de su lista.
    pub removal: Range<usize>,
}

/// Un tag escrito en una nota.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TagOccurrence {
    pub key: TagKey,
    /// Como lo escribió el usuario, sin `#`: solo para mostrar.
    pub spelling: String,
    /// El nombre en la nota, sin `#` ni comillas: lo que reescribe renombrar.
    pub range: Range<usize>,
    pub source: TagSource,
}

impl TagOccurrence {
    /// Un valor del frontmatter ya validado como tag.
    pub(crate) fn from_value(value: &Value) -> Self {
        Self {
            key: TagKey::new(&value.text),
            spelling: value.text.clone(),
            range: value.literal.clone().unwrap_or_else(|| value.span.clone()),
            source: TagSource::Frontmatter(FrontmatterSpot {
                literal: value.literal.is_some(),
                removal: value.removal.clone(),
            }),
        }
    }
}

/// Las ocurrencias de una nota, en orden de aparición, y la huella de su
/// contenido para saber si cambió desde que se escaneó.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct NoteTags {
    pub occurrences: Vec<TagOccurrence>,
    pub hash: u64,
}

/// Las ocurrencias de una nota y su frontmatter, del que una página de tag
/// saca su metadata.
pub(crate) fn scan_note(text: &str) -> (NoteTags, Option<Frontmatter>) {
    let scan = nota_syntax::scan(text);
    let mut occurrences: Vec<TagOccurrence> = scan
        .frontmatter
        .iter()
        .flat_map(|frontmatter| &frontmatter.tags)
        .map(TagOccurrence::from_value)
        .collect();
    occurrences.extend(scan.tags.iter().map(|tag| {
        let spelling = tag.name(text).to_owned();
        TagOccurrence {
            key: TagKey::new(&spelling),
            spelling,
            range: tag.name.clone(),
            source: TagSource::Inline,
        }
    }));
    let tags = NoteTags {
        occurrences,
        hash: content_hash(text),
    };
    (tags, scan.frontmatter)
}

/// Qué es un archivo de la carpeta de tags.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum PageFile {
    Page(Box<TagPage>),
    /// Su nombre no es un tag válido: es una nota normal.
    InvalidName,
}

/// Si `path` es una nota directamente dentro de `folder`, el nombre del tag
/// de su página, o `Err` si el nombre no es un tag válido.
pub(crate) fn page_name<'p>(
    folder: &VaultPath,
    path: &'p VaultPath,
) -> Option<Result<&'p str, ()>> {
    if path.parent().as_ref() != Some(folder) {
        return None;
    }
    let (stem, ext) = path.name().rsplit_once('.')?;
    if !ext.eq_ignore_ascii_case("md") || stem.is_empty() {
        return None;
    }
    Some(
        tag_name(stem)
            .filter(|name| name.len() == stem.len())
            .ok_or(()),
    )
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
    #[case("tags/rust.md", Some(Ok("rust")))]
    #[case("tags/Rust.MD", Some(Ok("Rust")))]
    #[case("tags/mi tag.md", Some(Err(())))]
    #[case("tags/a.b.md", Some(Err(())))]
    #[case("tags/#a.md", Some(Err(())))]
    #[case("tags/viejo/x.md", None)]
    #[case("tags/img.png", None)]
    #[case("rust.md", None)]
    fn pages_live_directly_in_the_folder(
        #[case] file: &str,
        #[case] name: Option<Result<&str, ()>>,
    ) {
        assert_eq!(page_name(&path("tags"), &path(file)), name);
    }

    #[test]
    fn occurrences_from_frontmatter_and_body() {
        let text = "---\ntags: [Rust, \"#nota\"]\n---\nHoy #gpui y #rust.";
        let (tags, _) = scan_note(text);
        let found: Vec<(&str, &str, &str)> = tags
            .occurrences
            .iter()
            .map(|o| (o.key.as_str(), o.spelling.as_str(), &text[o.range.clone()]))
            .collect();
        assert_eq!(
            found,
            [
                ("rust", "Rust", "Rust"),
                ("nota", "nota", "nota"),
                ("gpui", "gpui", "gpui"),
                ("rust", "rust", "rust"),
            ]
        );
    }

    #[test]
    fn an_escaped_value_is_rewritten_whole() {
        let text = "---\ntags: [\"r\\u0075st\"]\n---\n";
        let (tags, _) = scan_note(text);
        let [occurrence] = tags.occurrences.as_slice() else {
            panic!("{:?}", tags.occurrences);
        };
        assert_eq!(occurrence.key.as_str(), "rust");
        assert_eq!(&text[occurrence.range.clone()], "\"r\\u0075st\"");
        assert!(matches!(
            &occurrence.source,
            TagSource::Frontmatter(FrontmatterSpot { literal: false, .. })
        ));
    }
}
