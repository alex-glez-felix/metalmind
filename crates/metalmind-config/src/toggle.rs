//! Las opciones de sí o no de `config.yaml` que se cambian desde la app y
//! se guardan en el archivo, sin tocar el resto.

use crate::edit::{self, EditError};
use crate::settings::Settings;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Toggle {
    /// `editor.wrap`
    Wrap,
    /// `markdown.continue_lists`
    ContinueLists,
    /// `markdown.indent_list_children`
    IndentListChildren,
    /// `markdown.align_tables_on_leave`
    AlignTablesOnLeave,
    /// `which_key.enabled`
    WhichKey,
}

impl Toggle {
    /// Su clave en el archivo.
    #[must_use]
    pub fn path(self) -> [&'static str; 2] {
        match self {
            Self::Wrap => ["editor", "wrap"],
            Self::ContinueLists => ["markdown", "continue_lists"],
            Self::IndentListChildren => ["markdown", "indent_list_children"],
            Self::AlignTablesOnLeave => ["markdown", "align_tables_on_leave"],
            Self::WhichKey => ["which_key", "enabled"],
        }
    }
}

impl Settings {
    /// `text`, el archivo, con `toggle` en `on`.
    ///
    /// # Errors
    /// Los de [`edit::set`].
    pub fn save_toggle(text: &str, toggle: Toggle, on: bool) -> Result<String, EditError> {
        edit::set(text, &toggle.path(), if on { "true" } else { "false" })
    }
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;

    use super::*;

    fn get(toggle: Toggle, settings: &Settings) -> bool {
        match toggle {
            Toggle::Wrap => settings.editor.wrap,
            Toggle::ContinueLists => settings.markdown.continue_lists,
            Toggle::IndentListChildren => settings.markdown.indent_list_children,
            Toggle::AlignTablesOnLeave => settings.markdown.align_tables_on_leave,
            Toggle::WhichKey => settings.which_key.enabled,
        }
    }

    const ALL: [Toggle; 5] = [
        Toggle::Wrap,
        Toggle::ContinueLists,
        Toggle::IndentListChildren,
        Toggle::AlignTablesOnLeave,
        Toggle::WhichKey,
    ];

    #[test]
    fn lo_guardado_se_lee_con_ese_valor_y_lo_demas_igual() {
        let text = "# mía\ntheme: metalmind-claro  # el claro\neditor:\n  tabstop: 2\n";
        for toggle in ALL {
            for on in [true, false] {
                let saved = Settings::save_toggle(text, toggle, on).unwrap();
                let loaded = Settings::parse(&saved);
                assert_eq!(loaded.diagnostics, [], "{saved}");
                assert_eq!(get(toggle, &loaded.value), on, "{toggle:?}");
                assert_eq!(loaded.value.theme(), "metalmind-claro");
                assert_eq!(loaded.value.editor.tabstop.get(), 2);
                assert!(saved.starts_with("# mía\ntheme: metalmind-claro  # el claro\n"));
            }
        }
    }

    #[test]
    fn cambia_el_valor_en_su_lugar() {
        let text = "markdown:\n  continue_lists: true # listas\n";
        assert_eq!(
            Settings::save_toggle(text, Toggle::ContinueLists, false).unwrap(),
            "markdown:\n  continue_lists: false # listas\n"
        );
        assert_eq!(
            Settings::save_toggle("", Toggle::Wrap, false).unwrap(),
            "editor:\n  wrap: false\n"
        );
    }
}
