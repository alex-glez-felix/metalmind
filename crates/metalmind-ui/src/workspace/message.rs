//! El último aviso, el que vim muestra en la línea de comandos.

use metalmind_vim::{Substitution, Wrap};

use super::DocumentId;
use crate::document::{Document, FileStats};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Message {
    Info(String),
    Error(String),
}

impl Message {
    #[must_use]
    pub fn wrapped(wrap: Wrap) -> Self {
        Self::Info(
            match wrap {
                Wrap::ToTop => "la búsqueda llegó al final y siguió desde el principio",
                Wrap::ToBottom => "la búsqueda llegó al principio y siguió desde el final",
            }
            .to_owned(),
        )
    }

    /// `None` si fueron pocos para avisar.
    #[must_use]
    pub fn substituted(substitution: Substitution) -> Option<Self> {
        substitution.report().map(Self::Info)
    }

    /// Como vim al abrir: `"a.md" 3L, 20B`, `"a.md" [noeol] 1L, 3B` si le
    /// falta el `\n` final, o `[Nuevo]` si todavía no existe.
    #[must_use]
    pub fn opened(document: &Document) -> Self {
        let path = document.display_name();
        if document.exists_on_disk() {
            let noeol = if document.noeol() { "[noeol] " } else { "" };
            Self::Info(format!("\"{path}\" {noeol}{}", document.opened_stats()))
        } else {
            Self::Info(format!("\"{path}\" [Nuevo]"))
        }
    }

    #[must_use]
    pub fn written(name: &str, stats: FileStats) -> Self {
        Self::Info(format!("\"{name}\" {stats} escritos"))
    }

    /// `:e` sin ruta, que releería el archivo y perdería cambios.
    #[must_use]
    pub fn unsaved() -> Self {
        Self::Error("E37: No write since last change (add ! to override)".to_owned())
    }

    /// `:w ruta` con `ruta` abierta, sin guardar todavía, en otro buffer.
    #[must_use]
    pub fn loaded_elsewhere() -> Self {
        Self::Error("E139: File is loaded in another buffer".to_owned())
    }

    /// Cerrar la ventana con cambios en `document`. nvim muestra antes
    /// `E37: No write since last change`; la barra tiene una sola línea y
    /// se queda con la que dice cuál es.
    #[must_use]
    pub fn unsaved_in(document: &Document) -> Self {
        let path = document.display_name();
        Self::Error(format!(
            "E162: No write since last change for buffer \"{path}\""
        ))
    }

    /// `:wa` con un buffer sin archivo.
    #[must_use]
    pub fn no_file_name_for(id: DocumentId) -> Self {
        Self::Error(format!("E141: No file name for buffer {id}"))
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::*;

    #[test]
    fn s_avisa_desde_tres_reemplazos_como_vim() {
        let report = |replacements, lines| {
            Message::substituted(Substitution {
                replacements,
                lines,
            })
        };
        assert_eq!(report(2, 2), None);
        assert_eq!(
            report(3, 1),
            Some(Message::Info("3 substitutions on 1 line".to_owned()))
        );
        assert_eq!(
            report(4, 2),
            Some(Message::Info("4 substitutions on 2 lines".to_owned()))
        );
    }

    #[test]
    fn la_busqueda_avisa_hacia_donde_dio_la_vuelta() {
        assert_eq!(
            Message::wrapped(Wrap::ToTop),
            Message::Info("la búsqueda llegó al final y siguió desde el principio".to_owned())
        );
        assert_eq!(
            Message::wrapped(Wrap::ToBottom),
            Message::Info("la búsqueda llegó al principio y siguió desde el final".to_owned())
        );
    }

    #[test]
    fn al_abrir_dice_si_es_nuevo() {
        let document = Document::new(Path::new("a.md"), "");
        assert_eq!(
            Message::opened(&document),
            Message::Info("\"a.md\" [Nuevo]".to_owned())
        );
    }
}
