//! Las vistas guardadas que nombran un tag renombrado o fusionado: su
//! consulta cambia en la config del vault como un archivo más del plan, así
//! sale en la vista previa y `:vaultundo` la devuelve (06 §10).

use std::path::Path;

use metalmind_config::VaultConfig;
use metalmind_vault::tags::TagRewrite;
use metalmind_vault::{FileEdits, TextEdit, VaultPath};

/// La config del vault como está en disco.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum VaultConfigText {
    Missing,
    Text(String),
    /// No se pudo leer: el porqué.
    Unreadable(String),
}

impl VaultConfigText {
    #[must_use]
    pub fn read(root: &Path) -> Self {
        match metalmind_vault::read(&VaultConfig::file(root)) {
            Ok(Some(text)) => Self::Text(text),
            Ok(None) => Self::Missing,
            Err(error) => Self::Unreadable(error.to_string()),
        }
    }
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
