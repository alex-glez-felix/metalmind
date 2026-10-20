//! El último aviso, el que vim muestra en la línea de comandos.

use metalmind_vault::Unwatched;
use metalmind_vim::{Substitution, Wrap};

use super::DocumentId;
use crate::document::{Document, FileStats, SaveError};

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

    /// Un guardado que falló: la causa primero, para que un nombre largo no
    /// la tape al recortar la barra. `name` es el archivo, si no es el que
    /// ya muestra la barra, y va al final; los errores de vim van con su
    /// texto original.
    #[must_use]
    pub fn save_failed(name: Option<&str>, error: &SaveError) -> Self {
        let text = if error.is_vim() {
            error.to_string()
        } else {
            format!("no se pudo guardar: {}", error.cause())
        };
        Self::Error(match name {
            Some(name) => format!("{text} ({name})"),
            None => text,
        })
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

    /// El archivo de `document`, que tiene cambios, cambió fuera de Metalmind.
    #[must_use]
    pub fn changed_on_disk(document: &Document) -> Self {
        let path = document.display_name();
        Self::Error(format!(
            "W12: Warning: File \"{path}\" has changed and the buffer was changed in Vim as well"
        ))
    }

    /// El vault dejó de ver los cambios de fuera, y cómo arreglarlo; el log
    /// dice además cómo dejarlo así al reiniciar.
    #[must_use]
    pub fn unwatched(reason: Unwatched) -> Self {
        let text = match reason {
            Unwatched::WatchLimit => {
                "no se ven los cambios de fuera: se acabaron los watches de inotify; \
                 súbelos con sudo sysctl fs.inotify.max_user_watches=524288 y reabre"
            }
        };
        Self::Error(text.to_owned())
    }

    /// `:wa` con un buffer sin archivo.
    #[must_use]
    pub fn no_file_name_for(id: DocumentId) -> Self {
        Self::Error(format!("E141: No file name for buffer {id}"))
    }
}

#[cfg(test)]
mod tests {
    use std::io;
    use std::path::{Path, PathBuf};

    use metalmind_vault::FileError;

    use super::*;

    #[test]
    fn sin_watches_dice_como_subir_el_limite() {
        let Message::Error(text) = Message::unwatched(Unwatched::WatchLimit) else {
            panic!("es un error");
        };
        assert!(
            text.contains("sudo sysctl fs.inotify.max_user_watches="),
            "{text}"
        );
    }

    #[test]
    fn un_guardado_que_falla_dice_la_causa_primero() {
        let denied = SaveError::File(FileError::Write {
            path: PathBuf::from("/una/ruta/larguisima/nota.md"),
            source: io::Error::from(io::ErrorKind::PermissionDenied),
        });
        assert_eq!(
            Message::save_failed(None, &denied),
            Message::Error("no se pudo guardar: permiso denegado".to_owned())
        );
        assert_eq!(
            Message::save_failed(Some("otra.md"), &denied),
            Message::Error("no se pudo guardar: permiso denegado (otra.md)".to_owned())
        );
        assert_eq!(
            Message::save_failed(None, &SaveError::Exists),
            Message::Error("E13: File exists (add ! to override)".to_owned()),
            "los de vim, con su texto"
        );
        assert_eq!(
            Message::save_failed(Some("oculta.md"), &SaveError::ChangedOnDisk),
            Message::Error(
                "WARNING: The file has been changed since reading it!!! (oculta.md)".to_owned()
            ),
            "y con el nombre si no es la nota que se ve, como al fallar :wa"
        );
    }

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
