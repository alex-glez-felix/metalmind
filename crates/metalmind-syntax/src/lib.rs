//! La sintaxis de Metalmind sobre el texto de una nota: wikilinks, embeds, tags,
//! links markdown a notas locales, headings, `^bloques`, fórmulas `$…$` y
//! frontmatter YAML. Todo rango es en bytes sobre el texto escaneado.
//!
//! Es un scanner a mano, sin árbol de markdown: solo sabe lo necesario para
//! no ver links ni tags dentro de código, comentarios o fórmulas. El mismo scanner sirve
//! al editor y al índice, para que nunca discrepen sobre qué es un link.

mod anchor;
mod code;
mod frontmatter;
mod link;
mod markdown_link;
mod math;
pub mod note_name;
mod resolve;
mod scanner;
mod sink;
mod style;
mod tag;

use std::ops::Range;

pub use anchor::{BlockId, Heading};
pub use frontmatter::{Entry, Frontmatter, Property, PropertyValue, Value};
pub use link::{Link, LinkSyntax, Subpath};
pub use resolve::{
    Resolution, TargetKey, path_keys, path_parent_key, resolve, resolve_from, target_key,
};
pub use style::{
    Conceal, Decorations, FencePart, FormulaRows, LineKind, LineStyle, Lines, LinkSpot, MathPlace,
    MathSpot, Role, Rows, Span, Style, Styles, TextLines, Token, TypedWikilink, is_thematic_break,
    quote_prefix_len,
};
pub use tag::{MAX_TAG_NAME_BYTES, Tag, is_valid_tag_name};

/// Lo que el scanner encontró en una nota, en orden de aparición.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Scan {
    pub frontmatter: Option<Frontmatter>,
    pub links: Vec<Link>,
    pub tags: Vec<Tag>,
    pub headings: Vec<Heading>,
    pub blocks: Vec<BlockId>,
    /// Errores recuperables: el resto del scan sigue siendo válido.
    pub problems: Vec<Problem>,
}

/// Algo mal escrito en la nota que no impide escanear el resto.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum Problem {
    #[error("el frontmatter no es YAML válido: {message}")]
    InvalidYaml {
        span: Range<usize>,
        /// Dónde se dio cuenta el parser, en bytes sobre la nota.
        at: usize,
        message: String,
    },
    #[error("el frontmatter no es un mapa de claves")]
    FrontmatterNotMapping { span: Range<usize> },
    #[error("el campo `{field}` del frontmatter no es ni lista ni texto")]
    InvalidField {
        field: &'static str,
        span: Range<usize>,
    },
    #[error("el campo `{field}` del frontmatter no es un texto")]
    NotText {
        field: &'static str,
        span: Range<usize>,
    },
    #[error("`{name}` en `tags` del frontmatter no es un tag válido")]
    InvalidTag { name: String, span: Range<usize> },
}

/// Un tag bajo un byte de la nota.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TagSpot {
    /// El nombre, sin `#` ni comillas. Si el YAML lo escribe con escapes, el
    /// escalar entero.
    pub name: Range<usize>,
    /// Lo que reemplaza el autocompletado: en el cuerpo, el nombre; en el
    /// YAML, el escalar entero con sus comillas, para escribirlo con las
    /// que haga falta.
    pub replace: Range<usize>,
    pub place: TagPlace,
}

/// Dónde está escrito un tag. `aliases` y `parents` solo nombran tags en
/// una página de tag, y eso lo sabe quien conoce la ruta de la nota.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TagPlace {
    /// Un `#tag` del cuerpo.
    Inline,
    Tags,
    Aliases,
    Parents,
}

impl Scan {
    /// El tag sobre el que está `at`, contando el `#` o las comillas y el
    /// byte justo después del nombre: ahí queda el cursor al escribirlo, que
    /// es cuando se completa (05 §5), y donde se abre su menú (05 §3).
    #[must_use]
    pub fn tag_at(&self, at: usize) -> Option<TagSpot> {
        let covers = |span: &Range<usize>| span.start <= at && at <= span.end;
        if let Some(frontmatter) = self.frontmatter.as_ref().filter(|f| covers(&f.yaml)) {
            let fields = [
                (TagPlace::Tags, &frontmatter.tags),
                (TagPlace::Aliases, &frontmatter.aliases),
                (TagPlace::Parents, &frontmatter.parents),
            ];
            return fields.into_iter().find_map(|(place, values)| {
                let value = values.iter().find(|value| covers(&value.span))?.as_tag()?;
                Some(TagSpot {
                    name: value.literal.unwrap_or_else(|| value.span.clone()),
                    replace: value.span,
                    place,
                })
            });
        }
        let index = self.tags.partition_point(|tag| tag.span.end < at);
        let tag = self.tags.get(index).filter(|tag| covers(&tag.span))?;
        Some(TagSpot {
            name: tag.name.clone(),
            replace: tag.name.clone(),
            place: TagPlace::Inline,
        })
    }
}

/// Escanea una nota completa.
#[must_use]
pub fn scan(text: &str) -> Scan {
    scanner::scan(text)
}

/// El link que contiene el byte `offset`, si lo hay: lo que sigue `gd`.
#[must_use]
pub fn link_at(text: &str, offset: usize) -> Option<Link> {
    scan(text)
        .links
        .into_iter()
        .find(|link| link.span.contains(&offset))
}
