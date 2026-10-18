//! `config.toml`: lo que el usuario ajusta. Todo tiene default, así que un
//! archivo ausente o vacío es una config válida.

use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::diagnostic::Loaded;
use crate::parse::parse;
use crate::theme::DEFAULT_THEME;

#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(default)]
pub struct Settings {
    /// Nombre del tema: uno de `<config>/themes/*.toml` o uno embebido.
    pub theme: String,
    /// Vault que se abre si no se pasa ninguno; `~/` se expande.
    vault: Option<PathBuf>,
    pub font: Font,
    pub editor: Editor,
    pub search: Search,
}

#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(default)]
pub struct Font {
    /// `None`: la primera monoespaciada instalada de una lista conocida.
    pub family: Option<String>,
    pub size: FontSize,
}

#[derive(Clone, Copy, Debug, PartialEq, Deserialize)]
#[serde(try_from = "f32")]
pub struct FontSize(f32);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub struct Editor {
    /// Líneas que se dejan visibles por encima y por debajo del cursor.
    pub scrolloff: u32,
    pub tabstop: TabStop,
    /// `Tab` inserta espacios.
    pub expandtab: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[serde(try_from = "u8")]
pub struct TabStop(u8);

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub struct Search {
    pub ignorecase: bool,
    /// Con `ignorecase`, una mayúscula en el patrón vuelve la búsqueda exacta.
    pub smartcase: bool,
}

impl Settings {
    /// Lee `config.toml`. Nunca falla: con un error se usan los defaults y el
    /// error va en los diagnósticos. Es también la recarga en caliente.
    #[must_use]
    pub fn parse(text: &str) -> Loaded<Self> {
        match parse(text) {
            Ok((value, diagnostics)) => Loaded { value, diagnostics },
            Err(error) => Loaded {
                value: Self::default(),
                diagnostics: vec![error],
            },
        }
    }

    /// El vault por defecto, con `~/` resuelto contra `home`.
    #[must_use]
    pub fn vault(&self, home: &Path) -> Option<PathBuf> {
        let vault = self.vault.as_deref()?;
        Some(match vault.strip_prefix("~") {
            Ok(rest) => home.join(rest),
            Err(_) => vault.to_owned(),
        })
    }
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            theme: DEFAULT_THEME.to_owned(),
            vault: None,
            font: Font::default(),
            editor: Editor::default(),
            search: Search::default(),
        }
    }
}

impl Default for Font {
    fn default() -> Self {
        Self {
            family: None,
            size: FontSize(15.),
        }
    }
}

impl Default for Editor {
    fn default() -> Self {
        Self {
            scrolloff: 0,
            tabstop: TabStop(4),
            expandtab: true,
        }
    }
}

impl FontSize {
    const MIN: f32 = 4.;
    const MAX: f32 = 200.;

    #[must_use]
    pub fn get(self) -> f32 {
        self.0
    }
}

impl TryFrom<f32> for FontSize {
    type Error = String;

    fn try_from(size: f32) -> Result<Self, String> {
        if (Self::MIN..=Self::MAX).contains(&size) {
            Ok(Self(size))
        } else {
            Err(format!(
                "el tamaño de fuente debe estar entre {} y {}, no {size}",
                Self::MIN,
                Self::MAX
            ))
        }
    }
}

impl TabStop {
    #[must_use]
    pub fn get(self) -> u8 {
        self.0
    }
}

impl TryFrom<u8> for TabStop {
    type Error = String;

    fn try_from(width: u8) -> Result<Self, String> {
        if width == 0 {
            Err("tabstop debe ser al menos 1".to_owned())
        } else {
            Ok(Self(width))
        }
    }
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;

    use super::*;
    use crate::diagnostic::{Location, Severity};

    #[test]
    fn sin_archivo_todo_por_defecto() {
        let loaded = Settings::parse("");
        assert_eq!(loaded.value, Settings::default());
        assert_eq!(loaded.diagnostics, []);
        assert_eq!(loaded.value.theme, "nota-oscuro");
        assert_eq!(loaded.value.font.size, FontSize(15.));
        assert_eq!(loaded.value.editor.tabstop.get(), 4);
    }

    #[test]
    fn lee_cada_ajuste() {
        let text = r#"
theme = "nota-claro"
vault = "/notas"

[font]
family = "Iosevka"
size = 13.5

[editor]
scrolloff = 5
tabstop = 2
expandtab = false

[search]
ignorecase = true
smartcase = true
"#;
        let loaded = Settings::parse(text);
        assert_eq!(loaded.diagnostics, []);
        assert_eq!(
            loaded.value,
            Settings {
                theme: "nota-claro".to_owned(),
                vault: Some("/notas".into()),
                font: Font {
                    family: Some("Iosevka".to_owned()),
                    size: FontSize(13.5),
                },
                editor: Editor {
                    scrolloff: 5,
                    tabstop: TabStop(2),
                    expandtab: false,
                },
                search: Search {
                    ignorecase: true,
                    smartcase: true,
                },
            }
        );
    }

    #[test]
    fn un_tamano_entero_tambien_vale() {
        assert_eq!(
            Settings::parse(
                "[font]
size = 18"
            )
            .value
            .font
            .size,
            FontSize(18.)
        );
    }

    #[test]
    fn toml_roto_usa_defaults_y_reporta_linea() {
        let loaded = Settings::parse("theme = \"nota-claro\"\n[font\nsize = 12\n");
        assert_eq!(loaded.value, Settings::default());
        let [error] = loaded.diagnostics.as_slice() else {
            panic!("{:?}", loaded.diagnostics)
        };
        assert_eq!(error.severity, Severity::Error);
        assert_eq!(error.location.map(|at| at.line), Some(2));
    }

    #[test]
    fn tipo_equivocado_usa_defaults_y_reporta_posicion() {
        let loaded = Settings::parse("[editor]\ntabstop = \"cuatro\"\n");
        assert_eq!(loaded.value, Settings::default());
        assert_eq!(loaded.diagnostics.len(), 1);
        assert_eq!(
            loaded.diagnostics[0].location,
            Some(Location {
                line: 2,
                column: 11
            })
        );
    }

    #[test]
    fn valor_fuera_de_rango_es_error() {
        for text in [
            "[font]\nsize = 0",
            "[font]\nsize = 900",
            "[editor]\ntabstop = 0",
        ] {
            let loaded = Settings::parse(text);
            assert_eq!(loaded.value, Settings::default(), "{text}");
            assert_eq!(loaded.diagnostics[0].location.map(|at| at.line), Some(2));
        }
    }

    #[test]
    fn clave_desconocida_avisa_y_aplica_el_resto() {
        let loaded = Settings::parse("[editor]\nscrolloff = 3\nscroloff = 4\n");
        assert_eq!(loaded.value.editor.scrolloff, 3);
        let [warning] = loaded.diagnostics.as_slice() else {
            panic!("{:?}", loaded.diagnostics)
        };
        assert_eq!(warning.severity, Severity::Warning);
        assert_eq!(warning.location, Some(Location { line: 3, column: 1 }));
        assert!(warning.message.contains("editor.scroloff"));
    }

    #[test]
    fn el_vault_expande_la_tilde() {
        let home = Path::new("/home/usuario");
        let vault = |text: &str| Settings::parse(text).value.vault(home);
        assert_eq!(vault(""), None);
        assert_eq!(
            vault("vault = \"~/Notas\""),
            Some("/home/usuario/Notas".into())
        );
        assert_eq!(vault("vault = \"/srv/notas\""), Some("/srv/notas".into()));
    }
}
