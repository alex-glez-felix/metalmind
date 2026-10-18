//! Lo que una refactorización hace en el vault, como datos: editar rangos de
//! archivos, crear archivos nuevos y renombrar, en ese orden. Se arma sin
//! tocar el disco para mostrarlo como vista previa, y [`crate::Vault::apply`]
//! lo aplica con deshacer (05 §5).

use std::hash::{DefaultHasher, Hash as _, Hasher as _};
use std::ops::Range;

use crate::path::VaultPath;

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

/// La huella con la que un plan comprueba que el archivo sigue como se escaneó.
#[must_use]
pub fn content_hash(text: &str) -> u64 {
    let mut hasher = DefaultHasher::new();
    text.hash(&mut hasher);
    hasher.finish()
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

/// Un archivo que no existe y se crea con `text`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewFile {
    pub path: VaultPath,
    pub text: String,
}

/// Mover `from` a `to`, sin pisar nada.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rename {
    pub from: VaultPath,
    pub to: VaultPath,
}

/// Un cambio en varios archivos. Se aplica en orden: primero las ediciones
/// (con las rutas de antes de renombrar), luego los archivos nuevos y al
/// final los renombres.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Changes {
    /// En orden de ruta.
    pub edits: Vec<FileEdit>,
    pub creates: Vec<NewFile>,
    pub renames: Vec<Rename>,
}

impl Changes {
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.edits.is_empty() && self.creates.is_empty() && self.renames.is_empty()
    }

    /// Cuántas ediciones de texto hay, para el resumen de la vista previa.
    #[must_use]
    pub fn edit_count(&self) -> usize {
        self.edits.iter().map(|file| file.edits.len()).sum()
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
