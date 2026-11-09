//! El índice de links: los links de cada nota y de cada pizarra ya resueltos,
//! quién enlaza a cada archivo y los links sin resolver o ambiguos. Se arma con
//! lo escaneado de cada nota al abrir el vault y se mantiene al día aplicando cada
//! [`crate::VaultChange`].

mod attachments;
mod complete;
mod follow;
mod names;
pub(crate) mod note;
mod rename;
mod update;

use std::ops::Range;
use std::sync::Arc;

use imbl::{HashMap, OrdSet};

pub use follow::Destination;
pub use rename::{RenameConflict, RenamePlan, RenamePlanError};

use crate::path::VaultPath;
use names::Names;

/// A dónde apunta un link.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum LinkTarget {
    /// `[[#Heading]]`: la nota donde está el link.
    SameNote,
    Resolved(VaultPath),
    /// Las rutas que empatan, en orden.
    Ambiguous(Vec<VaultPath>),
    #[default]
    Unresolved,
}

/// Lo que va tras el `#` del destino, ya decodificado.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum LinkSubpath {
    Heading(String),
    /// Sin el `^`.
    Block(String),
}

/// La línea de un link, recortada alrededor de él.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct LinkContext {
    pub text: String,
    /// Dónde está el link dentro de `text`.
    pub link: Range<usize>,
}

/// Cómo está escrito el destino de un link: renombrar lo reescribe igual.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
enum Written {
    Wiki,
    Markdown,
    /// Un destino markdown entre `<…>`, que admite espacios.
    BracketedMarkdown,
    /// El `file` de una tarjeta de pizarra: la ruta desde la raíz, sin `/`.
    Card,
}

/// Un link de una nota o de una pizarra, con sus rangos en bytes sobre el
/// texto que se indexa de ella ([`crate::FileEdits`]): el archivo tal como
/// se leyó o, en una pizarra, el texto de sus nodos.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct IndexedLink {
    pub embed: bool,
    pub span: Range<usize>,
    /// El destino como está escrito.
    pub target_span: Range<usize>,
    /// El destino sin escapes ni `%20`; vacío en `[[#Heading]]`. El de una
    /// tarjeta de pizarra es su ruta con `/` delante: desde la raíz.
    pub target_text: String,
    pub subpath: Option<LinkSubpath>,
    /// Desde cero.
    pub line: usize,
    /// Dónde empieza el link en su línea, en bytes.
    pub column: usize,
    pub context: LinkContext,
    /// Depende del resto del vault: la caché no la guarda.
    #[serde(skip)]
    pub target: LinkTarget,
    written: Written,
}

impl IndexedLink {
    /// El destino tal como está escrito en el archivo.
    #[must_use]
    pub fn written_target(&self) -> &str {
        &self.context.text[self.in_context(&self.target_span)]
    }

    fn in_context(&self, range: &Range<usize>) -> Range<usize> {
        let shift = self.span.start - self.context.link.start;
        range.start - shift..range.end - shift
    }
}

/// Un link y la nota donde está.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NoteLink<'a> {
    pub source: &'a VaultPath,
    pub link: &'a IndexedLink,
}

/// El link número `ordinal` de la nota `source`.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct LinkId {
    source: VaultPath,
    ordinal: usize,
}

/// Los links del vault. Es un valor, como el [`crate::VaultIndex`]: un clon
/// comparte todo con el original y cada cambio copia solo lo que toca.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LinkIndex {
    names: Names,
    /// Los links de cada nota.
    notes: HashMap<VaultPath, Arc<Vec<IndexedLink>>>,
    /// Los links que resuelven a cada archivo.
    incoming: HashMap<VaultPath, OrdSet<LinkId>>,
    /// Las notas con algún link cuyo destino tiene esa clave: las que hay que
    /// volver a resolver cuando aparece o desaparece un archivo con ella.
    by_key: HashMap<String, OrdSet<VaultPath>>,
}

impl LinkIndex {
    /// Los links de `note`, en orden de aparición. Vacío si no es una nota
    /// ni una pizarra.
    #[must_use]
    pub fn outgoing(&self, note: &VaultPath) -> &[IndexedLink] {
        self.notes.get(note).map_or(&[], |links| links.as_slice())
    }

    /// A dónde lleva un link a `target` (el destino ya decodificado, como
    /// `metalmind_syntax::Link::target_text`) escrito en `source`. No
    /// recorre el vault: el editor lo pregunta por cada link que pinta.
    #[must_use]
    pub fn resolve(&self, source: &VaultPath, target: &str) -> LinkTarget {
        self.names.resolve(source, target)
    }

    /// Los links que resuelven a `path` desde otras notas, desde las pizarras
    /// (sus tarjetas y el texto de sus cajas) o desde ella misma por su
    /// nombre, ordenados por archivo y posición. No incluye `[[#Heading]]`.
    pub fn backlinks(&self, path: &VaultPath) -> impl Iterator<Item = NoteLink<'_>> {
        self.incoming
            .get(path)
            .into_iter()
            .flatten()
            .filter_map(|id| self.link(id))
    }

    /// Los links que no resuelven a ningún archivo, ordenados por nota.
    #[must_use]
    pub fn unresolved(&self) -> Vec<NoteLink<'_>> {
        self.matching(|target| *target == LinkTarget::Unresolved)
    }

    /// Los links con varias rutas empatadas, ordenados por nota.
    #[must_use]
    pub fn ambiguous(&self) -> Vec<NoteLink<'_>> {
        self.matching(|target| matches!(target, LinkTarget::Ambiguous(_)))
    }

    fn matching(&self, keep: impl Fn(&LinkTarget) -> bool) -> Vec<NoteLink<'_>> {
        let mut found: Vec<NoteLink<'_>> = self
            .notes
            .iter()
            .flat_map(|(source, links)| {
                links
                    .iter()
                    .filter(|link| keep(&link.target))
                    .map(move |link| NoteLink { source, link })
            })
            .collect();
        found.sort_by(|a, b| (a.source, a.link.span.start).cmp(&(b.source, b.link.span.start)));
        found
    }

    fn link(&self, id: &LinkId) -> Option<NoteLink<'_>> {
        let (source, links) = self.notes.get_key_value(&id.source)?;
        Some(NoteLink {
            source,
            link: links.get(id.ordinal)?,
        })
    }
}
