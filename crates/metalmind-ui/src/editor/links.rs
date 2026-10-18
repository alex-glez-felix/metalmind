//! Si un link del texto lleva a algo: lo pregunta al índice de links del
//! vault (`metalmind-vault`), que no recorre nada para responder.

use std::sync::Arc;

use metalmind_syntax::LinkSpot;
use metalmind_vault::{LinkIndex, LinkTarget, VaultPath};

/// El índice y la nota desde donde se resuelven sus links. Sin vault o con
/// una nota fuera de él, todo link cuenta como resuelto: no hay con qué
/// saber que no lo está.
#[derive(Clone, Debug, Default)]
pub struct Links {
    index: Option<Arc<LinkIndex>>,
    source: Option<VaultPath>,
}

impl Links {
    pub fn new(index: Arc<LinkIndex>, source: Option<VaultPath>) -> Self {
        Self {
            index: Some(index),
            source,
        }
    }

    pub fn resolves(&self, spot: &LinkSpot) -> bool {
        match (&self.index, &self.source) {
            (Some(index), Some(source)) => {
                index.resolve(source, &spot.target) != LinkTarget::Unresolved
            }
            _ => true,
        }
    }
}
