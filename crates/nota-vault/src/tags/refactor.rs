//! Lo que produce renombrar o fusionar tags: ediciones de archivo con rangos
//! exactos, sin escribir nada. La UI las muestra como vista previa y las
//! aplica con deshacer (05 §5).

use std::collections::BTreeMap;

use crate::changes::Changes;
use crate::edit::{FileEdits, TextEdit};
use crate::path::VaultPath;
use crate::tags::key::TagKey;

/// Lo que la vista previa tiene que decir.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlanNotice {
    /// El nombre nuevo ya es un tag: los dos se fusionan.
    Merges { into: TagKey },
    /// Se renombra un alias de `of`, no un tag.
    RenamesAlias { of: TagKey },
    /// El alias nuevo ya se escribe en notas: pasan a contar como `of`.
    AliasAbsorbs { tag: TagKey, of: TagKey },
    /// Bloqueo: los dos tags tienen página. Juntarlas es decisión del usuario.
    BothHavePages { from: VaultPath, into: VaultPath },
    /// Bloqueo: el alias nuevo es el nombre de un tag con página o un alias
    /// de otro tag.
    AliasTaken { alias: TagKey, by: TagKey },
    /// La página nueva tiene un nombre que Windows reserva (`con`, `nul`…):
    /// se crea igual, pero no se sincroniza con Windows (05 §1).
    NotPortable { path: VaultPath },
}

impl PlanNotice {
    /// Si impide aplicar el plan.
    #[must_use]
    pub fn blocks(&self) -> bool {
        matches!(self, Self::BothHavePages { .. } | Self::AliasTaken { .. })
    }
}

/// Un cambio en los tags listo para mostrar como vista previa y aplicar con
/// [`crate::Vault::apply`].
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TagPlan {
    pub changes: Changes,
    pub notices: Vec<PlanNotice>,
}

impl TagPlan {
    #[must_use]
    pub fn is_blocked(&self) -> bool {
        self.notices.iter().any(PlanNotice::blocks)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum PlanError {
    #[error("`{0}` no es un nombre de tag válido")]
    InvalidName(String),
    #[error("el tag `{0}` no existe en el vault")]
    UnknownTag(String),
    #[error("`{0}` no es un alias")]
    NotAnAlias(String),
    #[error("`{0}` no es un color ni un ícono válido")]
    InvalidValue(String),
    #[error("un tag no puede ser su propio padre (`{0}`)")]
    OwnParent(String),
    #[error("el frontmatter de {0} no se puede editar sin reescribirlo: hay que arreglarlo a mano")]
    UneditablePage(VaultPath),
}

/// Junta las ediciones por archivo mientras se arma un plan.
#[derive(Debug, Default)]
pub(crate) struct Edits {
    files: BTreeMap<VaultPath, Vec<TextEdit>>,
}

impl Edits {
    pub fn push(&mut self, path: &VaultPath, edit: TextEdit) {
        self.files.entry(path.clone()).or_default().push(edit);
    }

    pub fn into_files(self) -> Vec<FileEdits> {
        self.files
            .into_iter()
            .map(|(path, mut edits)| {
                edits.sort_by_key(|edit| edit.range.start);
                FileEdits { path, edits }
            })
            .collect()
    }
}
