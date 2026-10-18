//! Lo que produce renombrar o fusionar tags: ediciones de archivo con rangos
//! exactos, sin escribir nada. La UI las muestra como vista previa y las
//! aplica con deshacer (05 §5).

use std::collections::BTreeMap;
use std::ops::Range;

use crate::path::VaultPath;
use crate::tags::key::TagKey;
use crate::tags::note::content_hash;

/// Reemplazar `range` (bytes del contenido escaneado) por `text`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextEdit {
    pub range: Range<usize>,
    pub text: String,
}

/// Las ediciones de un archivo, en orden y sin solaparse.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileEdit {
    pub path: VaultPath,
    /// La huella del contenido que se escaneó ([`content_hash`]).
    pub hash: u64,
    pub edits: Vec<TextEdit>,
}

/// El archivo ya no es el que se escaneó: hay que reescanearlo y avisar en
/// vez de escribir a ciegas.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{path} cambió desde que se escaneó")]
pub struct StaleFile {
    pub path: VaultPath,
}

impl FileEdit {
    /// El contenido nuevo, si `text` sigue siendo el que se escaneó.
    ///
    /// # Errors
    /// Si `text` no es el contenido escaneado.
    pub fn apply(&self, text: &str) -> Result<String, StaleFile> {
        let stale = || StaleFile {
            path: self.path.clone(),
        };
        if content_hash(text) != self.hash {
            return Err(stale());
        }
        let mut out = String::with_capacity(text.len());
        let mut at = 0;
        for edit in &self.edits {
            out.push_str(text.get(at..edit.range.start).ok_or_else(stale)?);
            out.push_str(&edit.text);
            at = edit.range.end;
        }
        out.push_str(text.get(at..).ok_or_else(stale)?);
        Ok(out)
    }
}

/// Renombrar una página de tag, después de aplicar sus ediciones (que se
/// refieren a `from`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PageRename {
    pub from: VaultPath,
    pub to: VaultPath,
}

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
}

impl PlanNotice {
    /// Si impide aplicar el plan.
    #[must_use]
    pub fn blocks(&self) -> bool {
        matches!(self, Self::BothHavePages { .. } | Self::AliasTaken { .. })
    }
}

/// Un renombre o una fusión listos para mostrar y aplicar.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TagPlan {
    /// En orden de ruta.
    pub files: Vec<FileEdit>,
    pub page_rename: Option<PageRename>,
    pub notices: Vec<PlanNotice>,
}

impl TagPlan {
    #[must_use]
    pub fn is_blocked(&self) -> bool {
        self.notices.iter().any(PlanNotice::blocks)
    }

    /// Cuántas ediciones hay en total, para el resumen de la vista previa.
    #[must_use]
    pub fn edit_count(&self) -> usize {
        self.files.iter().map(|file| file.edits.len()).sum()
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
}

/// Junta las ediciones por archivo mientras se arma un plan.
#[derive(Debug, Default)]
pub(crate) struct Edits {
    files: BTreeMap<VaultPath, (u64, Vec<TextEdit>)>,
}

impl Edits {
    pub fn push(&mut self, path: &VaultPath, hash: u64, edit: TextEdit) {
        self.files
            .entry(path.clone())
            .or_insert_with(|| (hash, Vec::new()))
            .1
            .push(edit);
    }

    pub fn into_files(self) -> Vec<FileEdit> {
        self.files
            .into_iter()
            .map(|(path, (hash, mut edits))| {
                edits.sort_by_key(|edit| edit.range.start);
                FileEdit { path, hash, edits }
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn edits_apply_in_order() {
        let text = "tags: [a, b]\n#a y #a";
        let edit = FileEdit {
            path: VaultPath::new("n.md").unwrap(),
            hash: content_hash(text),
            edits: vec![
                TextEdit {
                    range: 7..8,
                    text: "x".into(),
                },
                TextEdit {
                    range: 14..15,
                    text: "xyz".into(),
                },
                TextEdit {
                    range: 19..20,
                    text: String::new(),
                },
            ],
        };
        assert_eq!(edit.apply(text).unwrap(), "tags: [x, b]\n#xyz y #");
    }

    #[test]
    fn a_changed_file_is_not_edited() {
        let edit = FileEdit {
            path: VaultPath::new("n.md").unwrap(),
            hash: content_hash("#a"),
            edits: vec![TextEdit {
                range: 1..2,
                text: "b".into(),
            }],
        };
        assert_eq!(
            edit.apply("#a "),
            Err(StaleFile {
                path: VaultPath::new("n.md").unwrap()
            })
        );
    }
}
