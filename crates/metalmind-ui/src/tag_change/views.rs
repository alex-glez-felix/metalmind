//! Las vistas guardadas que nombran un tag renombrado o fusionado: su
//! consulta cambia en la config del vault como un archivo más del plan, así
//! sale en la vista previa y `:vaultundo` la devuelve (06 §10).

use std::path::Path;

use metalmind_config::VaultConfig;
use metalmind_vault::tags::TagRewrite;
use metalmind_vault::{FileEdits, TextEdit, VaultPath};

use crate::note_list::views_symlink;

/// La config del vault como está en disco.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum VaultConfigText {
    Missing,
    Text(String),
    /// No se pudo leer: el porqué.
    Unreadable(String),
}

impl VaultConfigText {
    /// Una a la que se llega por un enlace simbólico no se reescribe:
    /// [`metalmind_vault::Vault::apply`] no escribe a través de enlaces.
    #[must_use]
    pub fn read(root: &Path) -> Self {
        if let Some(link) = views_symlink(root) {
            return Self::Unreadable(format!("{} es un enlace simbólico", link.display()));
        }
        match metalmind_vault::read(&VaultConfig::file(root)) {
            Ok(Some(text)) => Self::Text(text),
            Ok(None) => Self::Missing,
            Err(error) => Self::Unreadable(error.to_string()),
        }
    }
}

/// La ruta en el vault de `file` si es la config del vault de `root`: un
/// buffer suyo sin guardar también impide escribirla.
#[must_use]
pub fn config_path(root: &Path, file: &Path) -> Option<VaultPath> {
    (file == VaultConfig::file(root))
        .then(|| VaultPath::internal(VaultConfig::PATH).ok())
        .flatten()
}

/// Las ediciones de las vistas que nombran `rewrite.from`; `None` si
/// ninguna lo nombra.
///
/// # Errors
/// El porqué, si la config no se puede leer o reescribir.
pub(super) fn view_edits(
    rewrite: &TagRewrite,
    config: &VaultConfigText,
) -> Result<Option<FileEdits>, String> {
    let text = match config {
        VaultConfigText::Missing => return Ok(None),
        VaultConfigText::Text(text) => text,
        VaultConfigText::Unreadable(error) => return Err(error.clone()),
    };
    let edits = VaultConfig::requery_views(text, |query| rewrite.query(query))
        .map_err(|error| error.to_string())?;
    if edits.is_empty() {
        return Ok(None);
    }
    let path = VaultPath::internal(VaultConfig::PATH).map_err(|error| error.to_string())?;
    Ok(Some(FileEdits {
        path,
        edits: edits
            .into_iter()
            .map(|edit| TextEdit {
                range: edit.range,
                old: edit.old,
                new: edit.new,
                line: edit.line,
            })
            .collect(),
        base_len: Some(text.len()),
    }))
}

#[cfg(test)]
mod tests {
    use std::fs;

    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn una_config_por_un_enlace_no_se_lee_para_reescribirla() {
        let dir = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        fs::write(outside.path().join("config.yaml"), "views:\n  a: \"#a\"\n").unwrap();
        assert_eq!(VaultConfigText::read(dir.path()), VaultConfigText::Missing);
        std::os::unix::fs::symlink(outside.path(), dir.path().join(".metalmind")).unwrap();
        assert!(matches!(
            VaultConfigText::read(dir.path()),
            VaultConfigText::Unreadable(error) if error.ends_with("es un enlace simbólico")
        ));
    }

    #[test]
    fn el_buffer_de_la_config_cuenta_como_archivo_del_vault() {
        let root = Path::new("/notas");
        assert_eq!(
            config_path(root, &root.join(".metalmind/config.yaml"))
                .map(|path| path.as_str().to_owned()),
            Some(".metalmind/config.yaml".to_owned())
        );
        assert_eq!(config_path(root, &root.join("a.md")), None);
    }
}
