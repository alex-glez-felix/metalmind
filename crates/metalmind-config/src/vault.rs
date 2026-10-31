//! La config del vault, `<vault>/.metalmind/config.yaml` (06 §10): viaja
//! con el vault y por ahora guarda sus vistas guardadas, cada una un nombre
//! con su consulta de tags (`gs: "#gs -#archivado"`).

use std::fmt;
use std::path::{Path, PathBuf};

use serde::Deserialize;
use serde::de::{self, Deserializer, MapAccess, Visitor};

use crate::diagnostic::Loaded;
use crate::edit::{self, EditError};
use crate::file::{YamlFile, read_yaml};
use crate::parse::parse;

/// La clave de las vistas en el archivo.
const VIEWS: &str = "views";

#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub struct VaultConfig {
    /// En el orden del archivo.
    views: Views,
}

/// Una consulta de tags (05 §4) con nombre.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SavedView {
    pub name: String,
    /// Como se escribe en `space f`; la parsea `TagQuery` al usarla.
    pub query: String,
}

impl VaultConfig {
    /// Dónde está la del vault de `root`.
    #[must_use]
    pub fn file(root: &Path) -> PathBuf {
        root.join(".metalmind").join("config.yaml")
    }

    /// Sin archivo, vacía; con uno inválido también, y el error vuelve en
    /// los diagnósticos.
    #[must_use]
    pub fn load(root: &Path) -> Loaded<Self> {
        match read_yaml(&Self::file(root)) {
            Ok(Some(YamlFile { path, text })) => {
                let loaded = Self::parse(&text);
                Loaded {
                    value: loaded.value,
                    diagnostics: loaded
                        .diagnostics
                        .into_iter()
                        .map(|diagnostic| diagnostic.in_file(&path))
                        .collect(),
                }
            }
            Ok(None) => Loaded {
                value: Self::default(),
                diagnostics: Vec::new(),
            },
            Err(error) => Loaded {
                value: Self::default(),
                diagnostics: vec![error],
            },
        }
    }

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

    #[must_use]
    pub fn views(&self) -> &[SavedView] {
        &self.views.0
    }

    /// La vista `name`, escrita exactamente así.
    #[must_use]
    pub fn view(&self, name: &str) -> Option<&SavedView> {
        self.views().iter().find(|view| view.name == name)
    }

    /// `text`, el archivo, con la vista `name` guardada con `query`: nueva al
    /// final, o con su consulta cambiada si ya existe.
    ///
    /// # Errors
    /// Los de [`edit::set`].
    pub fn save_view(text: &str, name: &str, query: &str) -> Result<String, EditError> {
        edit::set(text, &[VIEWS, name], &edit::string(query))
    }

    /// # Errors
    /// Los de [`edit::rename`].
    pub fn rename_view(text: &str, from: &str, to: &str) -> Result<String, EditError> {
        edit::rename(text, &[VIEWS, from], to)
    }

    /// # Errors
    /// Los de [`edit::remove`].
    pub fn delete_view(text: &str, name: &str) -> Result<String, EditError> {
        edit::remove(text, &[VIEWS, name])
    }
}

/// Un mapa de nombre a consulta, leído en orden.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct Views(Vec<SavedView>);

impl<'de> Deserialize<'de> for Views {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_option(ViewsVisitor)
    }
}

struct ViewsVisitor;

impl<'de> Visitor<'de> for ViewsVisitor {
    type Value = Views;

    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("un mapa de nombre a consulta de tags")
    }

    fn visit_none<E: de::Error>(self) -> Result<Views, E> {
        Ok(Views::default())
    }

    fn visit_unit<E: de::Error>(self) -> Result<Views, E> {
        Ok(Views::default())
    }

    fn visit_some<D: Deserializer<'de>>(self, deserializer: D) -> Result<Views, D::Error> {
        deserializer.deserialize_map(self)
    }

    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Views, A::Error> {
        let mut views = Vec::new();
        while let Some((name, query)) = map.next_entry::<String, String>()? {
            views.push(SavedView { name, query });
        }
        Ok(Views(views))
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use pretty_assertions::assert_eq;

    use super::*;
    use crate::diagnostic::Severity;

    fn names(config: &VaultConfig) -> Vec<(&str, &str)> {
        config
            .views()
            .iter()
            .map(|view| (view.name.as_str(), view.query.as_str()))
            .collect()
    }

    #[test]
    fn las_vistas_se_leen_en_el_orden_del_archivo() {
        let loaded = VaultConfig::parse("views:\n  zeta: \"#z\"\n  gs: \"#gs -#archivado\"\n");
        assert_eq!(loaded.diagnostics, []);
        assert_eq!(
            names(&loaded.value),
            [("zeta", "#z"), ("gs", "#gs -#archivado")]
        );
        assert_eq!(loaded.value.view("gs").unwrap().query, "#gs -#archivado");
        assert_eq!(loaded.value.view("GS"), None);
    }

    #[test]
    fn sin_vistas_no_hay_ninguna() {
        for text in ["", "views:\n", "# nada\n"] {
            let loaded = VaultConfig::parse(text);
            assert_eq!(loaded.diagnostics, [], "{text:?}");
            assert_eq!(loaded.value, VaultConfig::default());
        }
    }

    #[test]
    fn una_consulta_que_no_es_texto_es_error() {
        let loaded = VaultConfig::parse("views:\n  gs: [a]\n");
        assert_eq!(loaded.value, VaultConfig::default());
        assert_eq!(loaded.diagnostics[0].severity, Severity::Error);
    }

    #[test]
    fn una_clave_desconocida_avisa() {
        let loaded = VaultConfig::parse("viewz: 1\nviews:\n  a: \"#a\"\n");
        assert_eq!(names(&loaded.value), [("a", "#a")]);
        assert_eq!(loaded.diagnostics[0].severity, Severity::Warning);
    }

    #[test]
    fn lo_guardado_se_lee_igual() {
        let text = VaultConfig::save_view("", "gs", "#gs -#archivado").unwrap();
        let text = VaultConfig::save_view(&text, "Para hoy: \"ya\"", "-#").unwrap();
        let text = VaultConfig::rename_view(&text, "gs", "estudio").unwrap();
        assert_eq!(
            names(&VaultConfig::parse(&text).value),
            [("estudio", "#gs -#archivado"), ("Para hoy: \"ya\"", "-#")]
        );
        let text = VaultConfig::delete_view(&text, "estudio").unwrap();
        let text = VaultConfig::delete_view(&text, "Para hoy: \"ya\"").unwrap();
        assert_eq!(text, "");
    }

    #[test]
    fn se_lee_del_vault_con_su_ruta_en_los_errores() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(VaultConfig::load(dir.path()).value, VaultConfig::default());
        let file = VaultConfig::file(dir.path());
        fs::create_dir_all(file.parent().unwrap()).unwrap();
        fs::write(&file, "views: [\n").unwrap();
        let loaded = VaultConfig::load(dir.path());
        assert_eq!(loaded.diagnostics[0].file.as_deref(), Some(file.as_path()));
    }
}
