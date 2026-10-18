//! Encontrar y leer un archivo YAML del usuario.

use std::io;
use std::path::{Path, PathBuf};

use crate::diagnostic::Diagnostic;

#[derive(Debug)]
pub(crate) struct YamlFile {
    pub(crate) path: PathBuf,
    pub(crate) text: String,
}

/// Lee `preferred` (`algo.yaml`) o, si no existe, `algo.yml`; `None` si no hay
/// ninguno. Avisa en `warnings` si están los dos (gana `.yaml`) o si queda un
/// `algo.toml` del formato anterior, que ya no se lee.
///
/// # Errors
/// Si el archivo existe pero no se puede leer.
pub(crate) fn read_yaml(
    preferred: &Path,
    warnings: &mut Vec<Diagnostic>,
) -> Result<Option<YamlFile>, Diagnostic> {
    let alternative = preferred.with_extension("yml");
    let chosen = if alternative.exists() && !preferred.exists() {
        alternative
    } else {
        if alternative.exists() {
            warnings.push(ignored(&alternative, preferred));
        }
        preferred.to_owned()
    };
    // Los avisos van antes de leer: un archivo ilegible no debe esconderlos.
    let old = preferred.with_extension("toml");
    if old.exists() {
        warnings.push(if chosen.exists() {
            ignored(&old, &chosen)
        } else {
            Diagnostic::warning(format!(
                "el formato cambió a YAML y este archivo ya no se lee; pásalo a `{}`",
                file_name(preferred)
            ))
            .in_file(&old)
        });
    }
    Ok(read(&chosen)?.map(|text| YamlFile { path: chosen, text }))
}

fn ignored(file: &Path, used: &Path) -> Diagnostic {
    Diagnostic::warning(format!(
        "se ignora porque también existe `{}`",
        file_name(used)
    ))
    .in_file(file)
}

/// El texto de `file`, `None` si no existe.
fn read(file: &Path) -> Result<Option<String>, Diagnostic> {
    match std::fs::read_to_string(file) {
        Ok(text) => Ok(Some(text)),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(Diagnostic::error(format!("no se pudo leer: {error}")).in_file(file)),
    }
}

fn file_name(path: &Path) -> String {
    path.file_name()
        .map_or_else(String::new, |name| name.to_string_lossy().into_owned())
}

#[cfg(test)]
mod tests {
    use std::fs;

    use pretty_assertions::assert_eq;

    use super::*;
    use crate::diagnostic::Severity;

    fn read_in(dir: &Path) -> (Result<Option<YamlFile>, Diagnostic>, Vec<Diagnostic>) {
        let mut warnings = Vec::new();
        let found = read_yaml(&dir.join("config.yaml"), &mut warnings);
        (found, warnings)
    }

    #[test]
    fn sin_archivo_no_hay_nada_ni_avisos() {
        let dir = tempfile::tempdir().unwrap();
        let (found, warnings) = read_in(dir.path());
        assert!(found.unwrap().is_none());
        assert_eq!(warnings, []);
    }

    #[test]
    fn acepta_la_extension_yml() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("config.yml"), "a: 1\n").unwrap();
        let (found, warnings) = read_in(dir.path());
        let found = found.unwrap().unwrap();
        assert_eq!(found.path, dir.path().join("config.yml"));
        assert_eq!(found.text, "a: 1\n");
        assert_eq!(warnings, []);
    }

    #[test]
    fn con_yaml_y_yml_gana_yaml_y_se_avisa() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("config.yaml"), "a: 1\n").unwrap();
        fs::write(dir.path().join("config.yml"), "a: 2\n").unwrap();
        let (found, warnings) = read_in(dir.path());
        assert_eq!(found.unwrap().unwrap().text, "a: 1\n");
        let [warning] = warnings.as_slice() else {
            panic!("{warnings:?}")
        };
        assert_eq!(warning.severity, Severity::Warning);
        assert_eq!(warning.file, Some(dir.path().join("config.yml")));
        assert_eq!(
            warning.message,
            "se ignora porque también existe `config.yaml`"
        );
    }

    #[test]
    fn un_toml_del_formato_anterior_solo_se_avisa() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("config.toml"), "theme = \"x\"\n").unwrap();
        let (found, warnings) = read_in(dir.path());
        assert!(found.unwrap().is_none());
        let [warning] = warnings.as_slice() else {
            panic!("{warnings:?}")
        };
        assert_eq!(warning.severity, Severity::Warning);
        assert_eq!(warning.file, Some(dir.path().join("config.toml")));
        assert!(warning.message.contains("YAML"), "{}", warning.message);
        assert!(warning.message.contains("`config.yaml`"));
    }

    #[test]
    fn un_toml_junto_al_yaml_solo_dice_que_se_ignora() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("config.toml"), "theme = \"x\"\n").unwrap();
        fs::write(dir.path().join("config.yml"), "a: 1\n").unwrap();
        let (found, warnings) = read_in(dir.path());
        assert_eq!(found.unwrap().unwrap().text, "a: 1\n");
        let [warning] = warnings.as_slice() else {
            panic!("{warnings:?}")
        };
        assert_eq!(warning.file, Some(dir.path().join("config.toml")));
        assert_eq!(
            warning.message,
            "se ignora porque también existe `config.yml`"
        );
    }

    #[test]
    fn un_yaml_ilegible_no_esconde_los_avisos_de_los_demas() {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir(dir.path().join("config.yaml")).unwrap();
        fs::write(dir.path().join("config.yml"), "a: 1\n").unwrap();
        fs::write(dir.path().join("config.toml"), "a = 1\n").unwrap();
        let (found, warnings) = read_in(dir.path());
        assert_eq!(
            found.unwrap_err().file,
            Some(dir.path().join("config.yaml"))
        );
        let files: Vec<_> = warnings
            .iter()
            .map(|warning| warning.file.clone())
            .collect();
        assert_eq!(
            files,
            [
                Some(dir.path().join("config.yml")),
                Some(dir.path().join("config.toml"))
            ]
        );
        assert!(
            warnings
                .iter()
                .all(|warning| warning.message.contains("`config.yaml`"))
        );
    }

    #[test]
    fn un_archivo_ilegible_es_error_con_su_ruta() {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir(dir.path().join("config.yaml")).unwrap();
        let error = read_in(dir.path()).0.unwrap_err();
        assert_eq!(error.severity, Severity::Error);
        assert_eq!(error.file, Some(dir.path().join("config.yaml")));
        assert!(error.message.starts_with("no se pudo leer"));
    }
}
