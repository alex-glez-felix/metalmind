//! El último aviso, el que vim muestra en la línea de comandos.

use nota_vim::{Substitution, Wrap};

use crate::document::{Document, FileStats};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Message {
    Info(String),
    Error(String),
}

/// Desde cuántos reemplazos `:s` los cuenta, como `report=2` de vim.
const REPORT: usize = 2;

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
    pub fn substituted(
        Substitution {
            replacements,
            lines,
        }: Substitution,
    ) -> Option<Self> {
        let plural =
            |n: usize, one: &str, many: &str| format!("{n} {}", if n == 1 { one } else { many });
        (replacements > REPORT).then(|| {
            Self::Info(format!(
                "{} en {}",
                plural(replacements, "reemplazo", "reemplazos"),
                plural(lines, "línea", "líneas"),
            ))
        })
    }

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
            Some(Message::Info("3 reemplazos en 1 línea".to_owned()))
        );
        assert_eq!(
            report(4, 2),
            Some(Message::Info("4 reemplazos en 2 líneas".to_owned()))
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
