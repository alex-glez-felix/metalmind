//! El último aviso, el que vim muestra en la línea de comandos.

use crate::document::{Document, FileStats};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Message {
    Info(String),
    Error(String),
}

impl Message {
    /// Como vim al abrir: `"a.md" 3L, 20B`, o `[Nuevo]` si todavía no existe.
    #[must_use]
    pub fn opened(document: &Document) -> Self {
        let path = document.path().display();
        if document.exists_on_disk() {
            Self::Info(format!("\"{path}\" {}", document.stats()))
        } else {
            Self::Info(format!("\"{path}\" [Nuevo]"))
        }
    }

    #[must_use]
    pub fn written(document: &Document, stats: FileStats) -> Self {
        let path = document.path().display();
        Self::Info(format!("\"{path}\" {stats} escritos"))
    }

    #[must_use]
    pub fn unsaved(hint: &str) -> Self {
        Self::Error(format!("E37: hay cambios sin guardar ({hint})"))
    }

    #[must_use]
    pub fn unsaved_in(document: &Document) -> Self {
        let path = document.path().display();
        Self::Error(format!("E162: hay cambios sin guardar en \"{path}\""))
    }

    #[must_use]
    pub fn others_unsaved() -> Self {
        Self::Error("E445: otros paneles tienen cambios sin guardar".to_owned())
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::*;

    #[test]
    fn al_abrir_dice_si_es_nuevo() {
        let document = Document::new(Path::new("a.md"), "");
        assert_eq!(
            Message::opened(&document),
            Message::Info("\"a.md\" [Nuevo]".to_owned())
        );
    }
}
