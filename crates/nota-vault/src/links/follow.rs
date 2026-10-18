//! Seguir el link bajo el cursor (`gd`, `enter`).

use std::ops::Range;

use nota_syntax::link_at;

use super::note::subpath;
use super::{LinkIndex, LinkSubpath, LinkTarget};
use crate::path::VaultPath;

/// A dónde lleva un link.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Destination {
    pub target: LinkTarget,
    /// El destino como lo nombra el link, sin escapes: el nombre con que
    /// crear la nota si no existe.
    pub target_text: String,
    pub subpath: Option<LinkSubpath>,
}

impl LinkIndex {
    /// El destino del link que contiene el byte `offset` de `text`, el
    /// contenido actual de la nota `source` (puede tener cambios sin guardar).
    /// Se resuelve contra los archivos del índice.
    #[must_use]
    pub fn follow(&self, source: &VaultPath, text: &str, offset: usize) -> Option<Destination> {
        let link = link_at(text, offset)?;
        let target_text = link.target_text(text).into_owned();
        Some(Destination {
            target: self.names.resolve(source, &target_text),
            target_text,
            subpath: subpath(text, &link),
        })
    }
}

impl LinkSubpath {
    /// Dónde está el heading o el bloque en `text`, el contenido de la nota
    /// destino: la línea del heading o el `^id`. `None` si no está.
    #[must_use]
    pub fn locate(&self, text: &str) -> Option<Range<usize>> {
        let scan = nota_syntax::scan(text);
        match self {
            Self::Heading(path) => scan.heading(text, path).map(|heading| heading.span.clone()),
            Self::Block(id) => scan.block(text, id).map(|block| block.span.clone()),
        }
    }
}
