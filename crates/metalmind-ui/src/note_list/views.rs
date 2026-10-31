//! Las vistas guardadas del vault en disco (06 §10): leerlas y cambiarlas en
//! `<vault>/.metalmind/config.yaml` tocando solo su clave, con guardado
//! atómico.

use std::fs;
use std::path::Path;

use metalmind_config::edit::EditError;
use metalmind_config::{Diagnostic, Loaded, SavedView, Severity, VaultConfig};
use metalmind_vault::FileError;
use metalmind_vault::tags::{QueryWord, TagQuery};

use super::source::ListSource;

#[derive(Debug, thiserror::Error)]
pub enum ViewsError {
    #[error("{0}")]
    Edit(#[from] EditError),
    #[error(transparent)]
    File(#[from] FileError),
    /// El archivo tiene un error: no se escribe hasta arreglarlo.
    #[error("{0}")]
    Config(Diagnostic),
    #[error("no se pudo crear {path}: {error}")]
    Folder { path: String, error: std::io::Error },
    #[error("{0} es un enlace simbólico; las vistas no se escriben a través de él")]
    Symlink(String),
    #[error("ya hay una vista «{0}»")]
    Exists(String),
    #[error("ya no existe la vista «{0}»")]
    Missing(String),
}

/// Las vistas del vault de `root`; las de un archivo roto, ninguna, con el
/// error.
#[must_use]
pub fn load(root: &Path) -> (Vec<SavedView>, Vec<Diagnostic>) {
    let Loaded { value, diagnostics } = VaultConfig::load(root);
    (value.views().to_vec(), diagnostics)
}

/// Aplica `change` al texto del archivo, con las vistas que dice ahora, y lo
/// guarda; devuelve las vistas que quedaron.
///
/// # Errors
/// Si el archivo no se puede leer, tiene un error, `change` falla o no se
/// puede escribir dentro del vault.
pub fn change(
    root: &Path,
    change: impl FnOnce(&str, &VaultConfig) -> Result<String, ViewsError>,
) -> Result<Vec<SavedView>, ViewsError> {
    let file = VaultConfig::file(root);
    let folder = file.parent().unwrap_or(root);
    // Escribir a través de un enlace podría tocar algo fuera del vault.
    for path in [folder, file.as_path()] {
        if fs::symlink_metadata(path).is_ok_and(|metadata| metadata.file_type().is_symlink()) {
            return Err(ViewsError::Symlink(path.display().to_string()));
        }
    }
    let text = metalmind_vault::read(&file)?.unwrap_or_default();
    let current = VaultConfig::parse(&text);
    if let Some(error) = current
        .diagnostics
        .into_iter()
        .find(|diagnostic| diagnostic.severity == Severity::Error)
    {
        return Err(ViewsError::Config(error.in_file(&file)));
    }
    let edited = change(&text, &current.value)?;
    if edited != text {
        fs::create_dir_all(folder).map_err(|error| ViewsError::Folder {
            path: folder.display().to_string(),
            error,
        })?;
        metalmind_vault::replace_file(&file, &edited)?;
    }
    Ok(VaultConfig::parse(&edited).value.views().to_vec())
}

/// La vista de `views` que se llama `name` sin distinguir mayúsculas.
#[must_use]
pub fn named<'v>(views: &'v [SavedView], name: &str) -> Option<&'v SavedView> {
    let name = name.to_lowercase();
    views.iter().find(|view| view.name.to_lowercase() == name)
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
        let source_query = source.query().unwrap_or_default();
        let mut draft = Self {
            query: String::new(),
            dropped: Vec::new(),
        };
        let mut words: Vec<&str> = Vec::new();
        for text in [source_query.as_str(), filter] {
            for (range, word) in TagQuery::words(text) {
                let written = &text[range];
                match word {
                    QueryWord::Invalid => return Err(DraftError::Invalid(written.to_owned())),
                    QueryWord::Text => draft.dropped.push(written.to_owned()),
                    _ if words.contains(&written) => {}
                    _ => words.push(written),
                }
            }
        }
        if words.is_empty() {
            return Err(DraftError::Empty);
        }
        draft.query = words.join(" ");
        Ok(draft)
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
    fn de_una_vista_con_texto_o_un_tag_invalido_no_sale_otra_igual() {
        let with_text = ListSource::View {
            name: "x".into(),
            query: "#rust idea".into(),
        };
        let draft = ViewDraft::new(&with_text, "-#archivado").unwrap();
        assert_eq!(draft.query, "#rust -#archivado");
        assert_eq!(draft.dropped, ["idea"]);
        let invalid = ListSource::View {
            name: "x".into(),
            query: "#2024/10".into(),
        };
        assert_eq!(
            ViewDraft::new(&invalid, "#rust"),
            Err(DraftError::Invalid("#2024/10".into()))
        );
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
        let views = change(dir.path(), |text, _| {
            Ok(VaultConfig::save_view(text, "gs", "#gs -#archivado")?)
        })
        .unwrap();
        assert_eq!(views.len(), 1);
        let file = VaultConfig::file(dir.path());
        fs::write(
            &file,
            format!("# mías\n{}", fs::read_to_string(&file).unwrap()),
        )
        .unwrap();
        let views = change(dir.path(), |text, _| {
            Ok(VaultConfig::rename_view(text, "gs", "estudio")?)
        })
        .unwrap();
        assert_eq!(views[0].name, "estudio");
        assert_eq!(
            fs::read_to_string(&file).unwrap(),
            "# mías\nviews:\n  estudio: \"#gs -#archivado\"\n"
        );
        assert_eq!(load(dir.path()).0, views);
    }

    fn save_a(text: &str, _: &VaultConfig) -> Result<String, ViewsError> {
        Ok(VaultConfig::save_view(text, "a", "#a")?)
    }

    #[test]
    fn un_archivo_roto_no_se_toca() {
        let dir = tempfile::tempdir().unwrap();
        let file = VaultConfig::file(dir.path());
        fs::create_dir_all(file.parent().unwrap()).unwrap();
        for broken in ["views: [\n", "views:\n  gs: [a]\n"] {
            fs::write(&file, broken).unwrap();
            let error = change(dir.path(), save_a).unwrap_err();
            assert!(matches!(error, ViewsError::Config(_)), "{error:?}");
            assert_eq!(fs::read_to_string(&file).unwrap(), broken);
        }
    }

    #[test]
    fn el_cambio_ve_las_vistas_del_archivo_de_ahora() {
        let dir = tempfile::tempdir().unwrap();
        let file = VaultConfig::file(dir.path());
        fs::create_dir_all(file.parent().unwrap()).unwrap();
        // Otro programa guardó «GS» mientras la pregunta estaba abierta.
        fs::write(&file, "views:\n  GS: \"#otra\"\n").unwrap();
        let error = change(dir.path(), |text, current| {
            if let Some(taken) = named(current.views(), "gs") {
                return Err(ViewsError::Exists(taken.name.clone()));
            }
            Ok(VaultConfig::save_view(text, "gs", "#gs")?)
        })
        .unwrap_err();
        assert!(
            matches!(&error, ViewsError::Exists(name) if name == "GS"),
            "{error:?}"
        );
        assert_eq!(
            fs::read_to_string(&file).unwrap(),
            "views:\n  GS: \"#otra\"\n"
        );
    }

    #[test]
    fn no_escribe_a_traves_de_un_enlace() {
        let dir = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        let target = outside.path().join("otro.yaml");
        fs::write(&target, "a: 1\n").unwrap();
        let file = VaultConfig::file(dir.path());
        let folder = file.parent().unwrap();
        fs::create_dir_all(folder).unwrap();
        std::os::unix::fs::symlink(&target, &file).unwrap();
        let error = change(dir.path(), save_a).unwrap_err();
        assert!(matches!(error, ViewsError::Symlink(_)), "{error:?}");
        assert_eq!(fs::read_to_string(&target).unwrap(), "a: 1\n");
        fs::remove_file(&file).unwrap();
        fs::remove_dir(folder).unwrap();
        std::os::unix::fs::symlink(outside.path(), folder).unwrap();
        let error = change(dir.path(), save_a).unwrap_err();
        assert!(matches!(error, ViewsError::Symlink(_)), "{error:?}");
        assert!(!outside.path().join("config.yaml").exists());
    }
}
