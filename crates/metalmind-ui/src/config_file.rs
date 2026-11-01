//! Cambiar un archivo de config del usuario en disco: se lee, se comprueba
//! que no tenga errores, se cambia solo lo pedido (con
//! `metalmind_config::edit`) y se guarda de forma atómica. Lo usan las
//! vistas guardadas de la config del vault y las opciones de `config.yaml`.

use std::fs;
use std::path::Path;

use metalmind_config::edit::EditError;
use metalmind_config::{Diagnostic, Loaded, Paths, Settings, Severity, Toggle};
use metalmind_vault::FileError;

#[derive(Debug, thiserror::Error)]
pub enum ConfigFileError {
    #[error(transparent)]
    File(#[from] FileError),
    /// El archivo tiene un error: no se escribe hasta arreglarlo.
    #[error("{0}")]
    Config(Diagnostic),
    #[error("no se pudo crear {path}: {error}")]
    Folder { path: String, error: std::io::Error },
}

/// Aplica `change` al texto de `file`, con lo que `parse` lee de él ahora,
/// y lo guarda si cambió; devuelve lo que se lee del archivo que quedó. Sin
/// archivo, el texto es vacío y se crea con su carpeta.
///
/// # Errors
/// Si el archivo no se puede leer, tiene un error, `change` falla o no se
/// puede escribir.
pub fn change<T, E: From<ConfigFileError>>(
    file: &Path,
    parse: impl Fn(&str) -> Loaded<T>,
    change: impl FnOnce(&str, &T) -> Result<String, E>,
) -> Result<T, E> {
    let text = metalmind_vault::read(file)
        .map_err(ConfigFileError::from)?
        .unwrap_or_default();
    let current = parse(&text);
    if let Some(error) = current
        .diagnostics
        .into_iter()
        .find(|diagnostic| diagnostic.severity == Severity::Error)
    {
        return Err(ConfigFileError::Config(error.in_file(file)).into());
    }
    let edited = change(&text, &current.value)?;
    if edited == text {
        return Ok(current.value);
    }
    if let Some(folder) = file.parent() {
        fs::create_dir_all(folder).map_err(|error| ConfigFileError::Folder {
            path: folder.display().to_string(),
            error,
        })?;
    }
    metalmind_vault::replace_file(file, &edited).map_err(ConfigFileError::from)?;
    Ok(parse(&edited).value)
}

#[derive(Debug, thiserror::Error)]
pub enum SettingsError {
    #[error("{0}")]
    Edit(#[from] EditError),
    #[error(transparent)]
    File(#[from] ConfigFileError),
}

/// Deja `toggle` en `on` en `config.yaml`. Un enlace simbólico se sigue y
/// se reescribe su destino: la config suele vivir en un repo de dotfiles.
///
/// # Errors
/// Los de [`change`], o si la clave está escrita de una forma que no se
/// edita sin tocar otras.
pub fn save_toggle(paths: &Paths, toggle: Toggle, on: bool) -> Result<(), SettingsError> {
    let file = paths.config_file();
    let file = fs::canonicalize(&file).unwrap_or(file);
    change(&file, Settings::parse, |text, _| {
        Ok::<_, SettingsError>(Settings::save_toggle(text, toggle, on)?)
    })?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;

    use super::*;

    fn paths() -> (tempfile::TempDir, Paths) {
        let dir = tempfile::tempdir().unwrap();
        let paths = Paths::under(dir.path());
        (dir, paths)
    }

    #[test]
    fn sin_archivo_lo_crea_con_su_carpeta() {
        let (_dir, paths) = paths();
        save_toggle(&paths, Toggle::ContinueLists, false).unwrap();
        assert_eq!(
            fs::read_to_string(paths.config_file()).unwrap(),
            "markdown:\n  continue_lists: false\n"
        );
    }

    #[test]
    fn cambia_solo_esa_clave_y_deja_los_comentarios() {
        let (_dir, paths) = paths();
        let text = "# mi config\ntheme: metalmind-claro\n\neditor:\n  wrap: true   # partir\n  tabstop: 2\n";
        fs::create_dir_all(paths.config_file().parent().unwrap()).unwrap();
        fs::write(paths.config_file(), text).unwrap();
        save_toggle(&paths, Toggle::Wrap, false).unwrap();
        save_toggle(&paths, Toggle::AlignTablesOnLeave, true).unwrap();
        assert_eq!(
            fs::read_to_string(paths.config_file()).unwrap(),
            format!(
                "{}markdown:\n  align_tables_on_leave: true\n",
                text.replace("wrap: true", "wrap: false")
            )
        );
    }

    #[test]
    fn un_archivo_roto_no_se_toca() {
        let (_dir, paths) = paths();
        fs::create_dir_all(paths.config_file().parent().unwrap()).unwrap();
        for broken in ["editor: [\n", "editor:\n  wrap: on\n"] {
            fs::write(paths.config_file(), broken).unwrap();
            let error = save_toggle(&paths, Toggle::Wrap, false).unwrap_err();
            assert!(
                matches!(error, SettingsError::File(ConfigFileError::Config(_))),
                "{error:?}"
            );
            assert_eq!(fs::read_to_string(paths.config_file()).unwrap(), broken);
        }
        fs::write(paths.config_file(), "editor: {wrap: true}\n").unwrap();
        let error = save_toggle(&paths, Toggle::Wrap, false).unwrap_err();
        assert!(matches!(error, SettingsError::Edit(_)), "{error:?}");
    }

    /// Con la config en un repo de dotfiles, se escribe el destino y el
    /// enlace sigue siendo enlace.
    #[test]
    fn sigue_un_enlace_simbolico() {
        let (dir, paths) = paths();
        let dotfiles = dir.path().join("dotfiles.yaml");
        fs::write(&dotfiles, "editor:\n  wrap: true\n").unwrap();
        fs::create_dir_all(paths.config_file().parent().unwrap()).unwrap();
        std::os::unix::fs::symlink(&dotfiles, paths.config_file()).unwrap();
        save_toggle(&paths, Toggle::Wrap, false).unwrap();
        assert_eq!(
            fs::read_to_string(&dotfiles).unwrap(),
            "editor:\n  wrap: false\n"
        );
        assert!(
            fs::symlink_metadata(paths.config_file())
                .unwrap()
                .file_type()
                .is_symlink()
        );
    }
}
