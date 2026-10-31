//! Las vistas guardadas del vault en disco (06 §10): leerlas y cambiarlas en
//! `<vault>/.metalmind/config.yaml` tocando solo su clave, con guardado
//! atómico.

use std::path::Path;

use metalmind_config::edit::EditError;
use metalmind_config::{Diagnostic, Loaded, SavedView, VaultConfig};
use metalmind_vault::FileError;
use metalmind_vault::tags::TagQuery;

use super::source::ListSource;

#[derive(Debug, thiserror::Error)]
pub enum ViewsError {
    #[error("{0}")]
    Edit(#[from] EditError),
    #[error(transparent)]
    File(#[from] FileError),
    #[error("no se pudo crear {path}: {error}")]
    Folder { path: String, error: std::io::Error },
}

/// Las vistas del vault de `root`; las de un archivo roto, ninguna, con el
/// error.
#[must_use]
pub fn load(root: &Path) -> (Vec<SavedView>, Vec<Diagnostic>) {
    let Loaded { value, diagnostics } = VaultConfig::load(root);
    (value.views().to_vec(), diagnostics)
}

/// Aplica `change` al texto del archivo y lo guarda; devuelve las vistas
/// que quedaron.
///
/// # Errors
/// Si el archivo no se puede leer, editar o escribir.
pub fn change(
    root: &Path,
    change: impl FnOnce(&str) -> Result<String, EditError>,
) -> Result<Vec<SavedView>, ViewsError> {
    let file = VaultConfig::file(root);
    let text = metalmind_vault::read(&file)?.unwrap_or_default();
    let edited = change(&text)?;
    if edited != text {
        if let Some(folder) = file.parent() {
            std::fs::create_dir_all(folder).map_err(|error| ViewsError::Folder {
                path: folder.display().to_string(),
                error,
            })?;
        }
        metalmind_vault::write_atomic(&file, |out| out.write_all(edited.as_bytes()))?;
    }
    Ok(VaultConfig::parse(&edited).value.views().to_vec())
}

/// Lo que se guarda al pedir "Guardar como vista": la consulta de la fuente
/// con los tags del filtro.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ViewDraft {
    pub query: String,
    /// Lo del filtro que no es de tags y una vista no guarda.
    pub dropped: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum DraftError {
    #[error("una vista es una consulta de tags: una carpeta no se guarda como vista")]
    Folder,
    #[error("`{0}` no es un tag válido")]
    Invalid(String),
    #[error("escribe en el filtro los tags de la vista, como `#rust -#archivado`")]
    Empty,
}

impl ViewDraft {
    /// # Errors
    /// Si la fuente es una carpeta, el filtro tiene un tag inválido o no
    /// queda ningún tag que guardar.
    pub fn new(source: &ListSource, filter: &str) -> Result<Self, DraftError> {
        if matches!(source, ListSource::Folder(_)) {
            return Err(DraftError::Folder);
        }
        let mut words: Vec<String> = source
            .query()
            .map(|query| query.split_whitespace().map(str::to_owned).collect())
            .unwrap_or_default();
        let parsed = TagQuery::parse(filter);
        if let Some(invalid) = parsed.invalid.first() {
            return Err(DraftError::Invalid(invalid.clone()));
        }
        for (range, _) in TagQuery::words(filter) {
            let word = &filter[range];
            if !parsed.words.iter().any(|text| text == word) && !words.iter().any(|w| w == word) {
                words.push(word.to_owned());
            }
        }
        if words.is_empty() {
            return Err(DraftError::Empty);
        }
        Ok(Self {
            query: words.join(" "),
            dropped: parsed.words,
        })
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use pretty_assertions::assert_eq;
    use rstest::rstest;

    use super::*;

    #[rstest]
    #[case::un_tag_y_el_filtro(ListSource::Tag("rust".into()), "#nota -#archivado", "#rust #nota -#archivado")]
    #[case::todas_con_el_filtro(ListSource::All, "#rust", "#rust")]
    #[case::el_buzon_solo(ListSource::Inbox, "", "-#")]
    #[case::una_vista_y_mas(
        ListSource::View { name: "gs".into(), query: "#gs -#archivado".into() },
        "-#archivado #hoy",
        "#gs -#archivado #hoy"
    )]
    fn la_vista_junta_la_fuente_y_los_tags_del_filtro(
        #[case] source: ListSource,
        #[case] filter: &str,
        #[case] query: &str,
    ) {
        assert_eq!(ViewDraft::new(&source, filter).unwrap().query, query);
    }

    #[test]
    fn el_texto_del_filtro_no_se_guarda_y_se_dice() {
        let draft = ViewDraft::new(&ListSource::All, "parser #rust gpui").unwrap();
        assert_eq!(draft.query, "#rust");
        assert_eq!(draft.dropped, ["parser", "gpui"]);
    }

    #[rstest]
    #[case::carpeta(ListSource::Folder("ui".into()), "#rust", DraftError::Folder)]
    #[case::sin_tags(ListSource::All, "parser", DraftError::Empty)]
    #[case::tag_invalido(ListSource::All, "#2024/10", DraftError::Invalid("#2024/10".into()))]
    fn lo_que_no_es_una_vista(
        #[case] source: ListSource,
        #[case] filter: &str,
        #[case] error: DraftError,
    ) {
        assert_eq!(ViewDraft::new(&source, filter), Err(error));
    }

    #[test]
    fn cambiar_las_vistas_crea_el_archivo_y_deja_lo_demas() {
        let dir = tempfile::tempdir().unwrap();
        let views = change(dir.path(), |text| {
            VaultConfig::save_view(text, "gs", "#gs -#archivado")
        })
        .unwrap();
        assert_eq!(views.len(), 1);
        let file = VaultConfig::file(dir.path());
        fs::write(
            &file,
            format!("# mías\n{}", fs::read_to_string(&file).unwrap()),
        )
        .unwrap();
        let views = change(dir.path(), |text| {
            VaultConfig::rename_view(text, "gs", "estudio")
        })
        .unwrap();
        assert_eq!(views[0].name, "estudio");
        assert_eq!(
            fs::read_to_string(&file).unwrap(),
            "# mías\nviews:\n  estudio: \"#gs -#archivado\"\n"
        );
        assert_eq!(load(dir.path()).0, views);
    }

    #[test]
    fn un_archivo_roto_no_se_toca() {
        let dir = tempfile::tempdir().unwrap();
        let file = VaultConfig::file(dir.path());
        fs::create_dir_all(file.parent().unwrap()).unwrap();
        fs::write(&file, "views: [\n").unwrap();
        let error = change(dir.path(), |text| VaultConfig::save_view(text, "a", "#a")).unwrap_err();
        assert!(
            matches!(error, ViewsError::Edit(EditError::Unreadable(_))),
            "{error:?}"
        );
        assert_eq!(fs::read_to_string(&file).unwrap(), "views: [\n");
    }
}
