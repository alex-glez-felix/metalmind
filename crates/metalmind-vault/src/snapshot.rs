//! Lo que el vault publica en cada lote: sus índices, juntos.

use std::sync::Arc;

use crate::index::VaultIndex;
use crate::links::LinkIndex;
use crate::properties::PropertyIndex;
use crate::refresh::VaultChange;
use crate::summary_index::SummaryIndex;
use crate::tags::{TagKey, Tags};

/// Los índices del vault de un mismo lote (03 §5). Se publican de una vez,
/// así que quien combine dos los toma del mismo snapshot y nunca ve uno de
/// un lote y otro del siguiente.
#[derive(Debug, Clone)]
pub struct VaultSnapshot {
    pub files: Arc<VaultIndex>,
    pub links: Arc<LinkIndex>,
    pub tags: Arc<Tags>,
    /// El frontmatter de cada nota.
    pub properties: Arc<PropertyIndex>,
    /// El título y el extracto de cada nota.
    pub summaries: Arc<SummaryIndex>,
}

/// Lo que cambió en el vault, con el snapshot que ya lo incluye. `changes`
/// puede venir vacío si solo cambiaron los links, los tags, las propiedades o
/// los resúmenes (un chmod que vuelve legible una nota) o si se dejó de
/// vigilar el vault ([`crate::Vault::unwatched`]).
#[derive(Debug, Clone)]
pub struct VaultUpdate {
    pub changes: Vec<VaultChange>,
    pub snapshot: Arc<VaultSnapshot>,
    /// Los tags cuyas notas, formas escritas o página pueden haber cambiado.
    pub tag_changes: Vec<TagKey>,
}
