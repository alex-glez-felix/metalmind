//! La sintaxis de nota sobre el texto de una nota: wikilinks, embeds, tags,
//! links markdown a notas locales, headings, `^bloques` y frontmatter YAML. Todo
//! rango es en bytes sobre el texto escaneado.
//!
//! Es un scanner a mano, sin árbol de markdown: solo sabe lo necesario para
//! no ver links ni tags dentro de código o comentarios. El mismo scanner sirve
//! al editor y al índice, para que nunca discrepen sobre qué es un link.

mod anchor;
mod code;
mod frontmatter;
mod link;
mod markdown_link;
mod resolve;
mod scanner;
mod tag;

use std::ops::Range;

pub use anchor::{BlockId, Heading};
pub use frontmatter::Frontmatter;
pub use link::{Link, LinkSyntax, Subpath};
pub use resolve::{
    Resolution, TargetKey, path_keys, path_parent_key, resolve, resolve_from, target_key,
};
pub use tag::{Tag, is_valid_tag_name};

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
    #[error("`{name}` en `tags` del frontmatter no es un tag válido")]
    InvalidTag { name: String, span: Range<usize> },
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
