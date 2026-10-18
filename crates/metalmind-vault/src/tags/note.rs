//! Lo que una nota aporta a los tags: sus ocurrencias y, si es una página de
//! tag, su metadata.

use std::ops::Range;

use metalmind_syntax::{Frontmatter, Value};

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
    /// El escalar como está escrito, si el YAML no escribe el nombre tal
    /// cual (escapes, varias líneas): el rango lo cubre entero y se
    /// reescribe entre comillas.
    pub escaped: Option<String>,
    /// Lo que se borra para sacar el valor de su lista.
    pub removal: Snippet,
    /// Lo que se borra si también se quitan todos los de antes en su lista.
    pub removal_first: Snippet,
}

/// Un tramo de la nota y lo que dice, para que una edición compruebe que
/// sigue ahí.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Snippet {
    pub range: Range<usize>,
    pub text: String,
}

impl Snippet {
    fn new(text: &str, range: &Range<usize>) -> Self {
        Self {
            range: range.clone(),
            text: text[range.clone()].to_owned(),
        }
    }
}

/// Un tag escrito en una nota.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TagOccurrence {
    pub key: TagKey,
    /// Como lo escribió el usuario, sin `#`: solo para mostrar.
    pub spelling: String,
    /// El nombre en la nota, sin `#` ni comillas: lo que reescribe renombrar.
    pub range: Range<usize>,
    /// La línea, desde cero, para la vista previa.
    pub line: usize,
    pub source: TagSource,
}

impl TagOccurrence {
    /// Un valor del frontmatter de `text` ya validado como tag.
    pub(crate) fn from_value(text: &str, value: &Value, lines: &mut Lines<'_>) -> Self {
        let range = value.literal.clone().unwrap_or_else(|| value.span.clone());
        Self {
            key: TagKey::new(&value.text),
            spelling: value.text.clone(),
            line: lines.line_of(range.start),
            source: TagSource::Frontmatter(FrontmatterSpot {
                escaped: value
                    .literal
                    .is_none()
                    .then(|| text[range.clone()].to_owned()),
                removal: Snippet::new(text, &value.removal),
                removal_first: Snippet::new(text, &value.removal_first),
            }),
            range,
        }
    }

    /// Lo que dice la nota en [`TagOccurrence::range`].
    #[must_use]
    pub fn written(&self) -> &str {
        match &self.source {
            TagSource::Frontmatter(FrontmatterSpot {
                escaped: Some(escaped),
                ..
            }) => escaped,
            _ => &self.spelling,
        }
    }
}

/// Cuenta líneas hacia adelante: preguntar en orden recorre el texto una vez.
pub(crate) struct Lines<'t> {
    text: &'t str,
    at: usize,
    line: usize,
}

impl<'t> Lines<'t> {
    pub(crate) fn new(text: &'t str) -> Self {
        Self {
            text,
            at: 0,
            line: 0,
        }
    }

    /// La línea de `offset`, desde cero.
    pub(crate) fn line_of(&mut self, offset: usize) -> usize {
        if offset < self.at {
            *self = Self::new(self.text);
        }
        self.line += self.text[self.at..offset].matches('\n').count();
        self.at = offset;
        self.line
    }
}

/// Las ocurrencias de una nota, en orden de aparición.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct NoteTags {
    pub occurrences: Vec<TagOccurrence>,
}

/// Las ocurrencias de una nota y su frontmatter, del que una página de tag
/// saca su metadata.
pub(crate) fn scan_note(text: &str) -> (NoteTags, Option<Frontmatter>) {
    let scan = metalmind_syntax::scan(text);
    let mut lines = Lines::new(text);
    let mut occurrences: Vec<TagOccurrence> = scan
        .frontmatter
        .iter()
        .flat_map(|frontmatter| &frontmatter.tags)
        .map(|value| TagOccurrence::from_value(text, value, &mut lines))
        .collect();
    occurrences.extend(scan.tags.iter().map(|tag| {
        let spelling = tag.name(text).to_owned();
        TagOccurrence {
            key: TagKey::new(&spelling),
            spelling,
            range: tag.name.clone(),
            line: lines.line_of(tag.name.start),
            source: TagSource::Inline,
        }
    }));
    (NoteTags { occurrences }, scan.frontmatter)
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
            TagSource::Frontmatter(FrontmatterSpot {
                escaped: Some(_),
                ..
            })
        ));
    }
}
