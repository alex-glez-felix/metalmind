//! Lo que produce renombrar o fusionar tags: ediciones de archivo con rangos
//! exactos, sin escribir nada. La UI las muestra como vista previa y las
//! aplica con deshacer (05 §5).

use std::collections::BTreeMap;

use crate::changes::Changes;
use crate::edit::{FileEdits, TextEdit};
use crate::path::VaultPath;
use crate::tags::key::TagKey;
use crate::tags::query::{QueryWord, TagQuery};

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
    /// El tag que deja de escribirse así, si el plan lo renombra o fusiona:
    /// las consultas que lo nombran, como las vistas guardadas, lo siguen.
    pub rewrite: Option<TagRewrite>,
}

/// Lo que era `#from` pasa a escribirse `#to`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TagRewrite {
    pub from: TagKey,
    pub to: String,
}

impl TagRewrite {
    /// `query` (05 §4) con `#from` y `-#from` escritos con el nombre nuevo;
    /// `None` si no lo nombra. Una palabra que queda repetida se quita, y
    /// las demás quedan separadas por un espacio.
    #[must_use]
    pub fn query(&self, query: &str) -> Option<String> {
        let to = TagKey::new(&self.to);
        let mut renamed = false;
        let mut kept: Vec<QueryWord> = Vec::new();
        let mut words: Vec<String> = Vec::new();
        for (range, word) in TagQuery::words(query) {
            let original = &query[range];
            let (word, written) = match word {
                QueryWord::Include(key) if key == self.from => {
                    (QueryWord::Include(to.clone()), format!("#{}", self.to))
                }
                QueryWord::Exclude(key) if key == self.from => {
                    (QueryWord::Exclude(to.clone()), format!("-#{}", self.to))
                }
                word => (word, original.to_owned()),
            };
            renamed |= written != original;
            let tag = !matches!(word, QueryWord::Text | QueryWord::Invalid);
            if tag && kept.contains(&word) {
                continue;
            }
            kept.push(word);
            words.push(written);
        }
        renamed.then(|| words.join(" "))
    }
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
                FileEdits {
                    path,
                    edits,
                    base_len: None,
                }
            })
            .collect()
    }
}
