//! De dónde salen las notas de la lista: el buzón, todas, una vista
//! guardada, un tag con sus descendientes o una carpeta con sus subcarpetas.

use std::collections::BTreeSet;

use metalmind_config::SavedView;
use metalmind_vault::tags::{TagKey, TagQuery, Tags};
use metalmind_vault::{EntryKind, InvalidPath, VaultChange, VaultIndex, VaultPath};
use serde::{Deserialize, Serialize};

/// Por qué `list::show_folder` no tiene carpeta que mostrar.
#[derive(Debug, thiserror::Error)]
pub enum FolderError {
    #[error("la nota no es del vault: no tiene carpeta que mostrar en la lista")]
    NotInVault,
    #[error(transparent)]
    Invalid(#[from] InvalidPath),
    #[error("{0} no es una carpeta del vault")]
    Missing(VaultPath),
}

/// La carpeta que pide `list::show_folder`: la nombrada (con o sin `/` al
/// final) o, sin nombre, la de `note`.
///
/// # Errors
/// Si no se nombra ninguna y `note` no es del vault, o si la nombrada no es
/// una carpeta de `index`.
pub fn folder_to_show(
    named: Option<&str>,
    note: Option<&VaultPath>,
    index: &VaultIndex,
) -> Result<VaultPath, FolderError> {
    let folder = match named {
        Some(named) => VaultPath::new(named.trim().trim_end_matches('/'))?,
        None => note
            .and_then(VaultPath::parent)
            .ok_or(FolderError::NotInVault)?,
    };
    match index.kind(&folder) {
        Some(EntryKind::Folder) => Ok(folder),
        _ => Err(FolderError::Missing(folder)),
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ListSource {
    /// Las notas sin tags (06 §9).
    #[default]
    Inbox,
    All,
    /// Una vista guardada (06 §10), con la consulta que tenía al elegirla.
    View {
        name: String,
        query: String,
    },
    /// El nombre del tag, sin `#`, como se eligió.
    Tag(String),
    /// La ruta de la carpeta en el vault.
    Folder(String),
}

impl ListSource {
    /// La carpeta `folder` con sus subcarpetas; la raíz del vault son todas
    /// las notas.
    #[must_use]
    pub fn folder(folder: &VaultPath) -> Self {
        if folder.is_root() {
            Self::All
        } else {
            Self::Folder(folder.as_str().to_owned())
        }
    }

    /// Cómo se nombra en la cabecera y en el picker de fuentes.
    #[must_use]
    pub fn label(&self) -> String {
        match self {
            Self::Inbox => "Buzón".to_owned(),
            Self::All => "Todas las notas".to_owned(),
            Self::View { name, .. } => name.clone(),
            Self::Tag(tag) => format!("#{tag}"),
            Self::Folder(folder) => format!("{folder}/"),
        }
    }

    /// La consulta de tags que la define, como se escribiría en `space f`.
    #[must_use]
    pub fn query(&self) -> Option<String> {
        match self {
            Self::Inbox => Some("-#".to_owned()),
            Self::Tag(tag) => Some(format!("#{tag}")),
            Self::View { query, .. } => Some(query.clone()),
            Self::All | Self::Folder(_) => None,
        }
    }

    /// Sus notas, en cualquier orden. Un tag que ya no existe, una carpeta
    /// que ya no está o una vista con algo que no es de tags no tienen
    /// ninguna.
    #[must_use]
    pub fn notes(&self, index: &VaultIndex, tags: &Tags) -> Vec<VaultPath> {
        match self {
            Self::Inbox | Self::Tag(_) | Self::View { .. } => {
                let text = self.query().unwrap_or_default();
                let parsed = TagQuery::parse(&text);
                if !parsed.invalid.is_empty() || !parsed.words.is_empty() {
                    return Vec::new();
                }
                parsed.query.notes(tags).into_iter().cloned().collect()
            }
            Self::All => notes_where(index, |_| true),
            Self::Folder(folder) => match VaultPath::new(folder) {
                Ok(folder) => notes_where(index, |path| path.starts_with(&folder)),
                Err(_) => Vec::new(),
            },
        }
    }

    /// Si la fuente es `#from`, pasa a ser `#to`; devuelve si cambió.
    pub fn rename_tag(&mut self, from: &str, to: &str) -> bool {
        let Self::Tag(tag) = self else {
            return false;
        };
        let bare = |name: &str| name.trim_start_matches('#').to_owned();
        if TagKey::new(&bare(tag)) != TagKey::new(&bare(from)) {
            return false;
        }
        *tag = bare(to);
        true
    }

    /// Una carpeta renombrada sigue siendo la fuente con su nombre nuevo.
    pub fn follow(&mut self, changes: &[VaultChange]) {
        let Self::Folder(folder) = self else {
            return;
        };
        let Ok(mut path) = VaultPath::new(folder) else {
            return;
        };
        for change in changes {
            if let VaultChange::Renamed { from, to, .. } = change
                && let Some(moved) = path.rebase(from, to)
            {
                path = moved;
            }
        }
        path.as_str().clone_into(folder);
    }

    /// El nombre de la vista guardada, si lo es.
    #[must_use]
    pub fn view_name(&self) -> Option<&str> {
        match self {
            Self::View { name, .. } => Some(name),
            _ => None,
        }
    }

    /// Una vista sigue a las del archivo: con su consulta de ahora, o el
    /// buzón si ya no existe.
    #[must_use]
    pub fn resolve(self, views: &[SavedView]) -> Self {
        let Self::View { name, .. } = &self else {
            return self;
        };
        views
            .iter()
            .find(|view| &view.name == name)
            .map_or(Self::Inbox, Self::of_view)
    }

    #[must_use]
    pub fn of_view(view: &SavedView) -> Self {
        Self::View {
            name: view.name.clone(),
            query: view.query.clone(),
        }
    }

    /// Lo que se puede elegir como fuente en este vault: el buzón, todas,
    /// cada vista, cada tag y cada carpeta, en ese orden.
    #[must_use]
    pub fn choices(index: &VaultIndex, tags: &Tags, views: &[SavedView]) -> Vec<Self> {
        // También los que solo tienen página o son padres de otros: traen
        // las notas de sus descendientes.
        let keys: BTreeSet<&TagKey> = tags
            .index()
            .keys()
            .chain(tags.graph().pages().map(|page| &page.key))
            .chain(tags.graph().parent_keys())
            .collect();
        let mut tag_names: Vec<String> = keys
            .into_iter()
            .map(|key| tags.display_name(key).unwrap_or(key.as_str()).to_owned())
            .collect();
        tag_names.sort_by_key(|name| name.to_lowercase());
        let mut folders: Vec<String> = index
            .iter()
            .filter(|entry| entry.kind == EntryKind::Folder && !entry.path.is_root())
            .map(|entry| entry.path.as_str().to_owned())
            .collect();
        folders.sort();
        [Self::Inbox, Self::All]
            .into_iter()
            .chain(views.iter().map(Self::of_view))
            .chain(tag_names.into_iter().map(Self::Tag))
            .chain(folders.into_iter().map(Self::Folder))
            .collect()
    }
}

fn notes_where(index: &VaultIndex, keep: impl Fn(&VaultPath) -> bool) -> Vec<VaultPath> {
    index
        .files()
        .filter(|entry| entry.kind == EntryKind::Note && keep(entry.path))
        .map(|entry| entry.path.clone())
        .collect()
}

#[cfg(test)]
mod tests {
    use std::fs;

    use metalmind_vault::Vault;
    use pretty_assertions::assert_eq;
    use rstest::rstest;

    use super::*;

    const FILES: &[(&str, &str)] = &[
        ("parser.md", "#Rust y el parser"),
        ("ui/gpui.md", "---\ntags: [gpui]\n---\nUn parser"),
        ("ui/hondo/viejo.md", "#rust #archivado"),
        ("sin-tags.md", "Nada."),
        ("ui/suelta.md", "Tampoco."),
        ("tags/gpui.md", "---\nparents: [rust]\n---\n"),
        ("img.png", ""),
    ];

    fn vault() -> (tempfile::TempDir, Vault) {
        let dir = tempfile::tempdir().unwrap();
        for (name, text) in FILES {
            let path = dir.path().join(name);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, text).unwrap();
        }
        let (vault, _updates) = Vault::open(dir.path()).unwrap();
        (dir, vault)
    }

    fn sorted(notes: &[VaultPath]) -> Vec<String> {
        let mut names: Vec<String> = notes.iter().map(ToString::to_string).collect();
        names.sort();
        names
    }

    #[rstest]
    #[case::buzon(ListSource::Inbox, &["sin-tags.md", "ui/suelta.md"])]
    #[case::un_tag_con_sus_descendientes(
        ListSource::Tag("Rust".into()),
        &["parser.md", "ui/gpui.md", "ui/hondo/viejo.md"]
    )]
    #[case::un_tag_que_no_existe(ListSource::Tag("nada".into()), &[])]
    #[case::un_tag_invalido(ListSource::Tag("a/b".into()), &[])]
    #[case::una_carpeta_y_sus_subcarpetas(
        ListSource::Folder("ui".into()),
        &["ui/gpui.md", "ui/hondo/viejo.md", "ui/suelta.md"]
    )]
    #[case::una_carpeta_que_no_esta(ListSource::Folder("nada".into()), &[])]
    #[case::una_vista(
        ListSource::View { name: "gs".into(), query: "#rust -#archivado".into() },
        &["parser.md", "ui/gpui.md"]
    )]
    #[case::una_vista_con_texto(
        ListSource::View { name: "x".into(), query: "#rust parser".into() },
        &[]
    )]
    #[case::todas(
        ListSource::All,
        &["parser.md", "sin-tags.md", "tags/gpui.md", "ui/gpui.md", "ui/hondo/viejo.md", "ui/suelta.md"]
    )]
    fn cada_fuente_trae_sus_notas(#[case] source: ListSource, #[case] expected: &[&str]) {
        let (_dir, vault) = vault();
        let notes = source.notes(&vault.snapshot(), &vault.tags());
        assert_eq!(sorted(&notes), expected);
    }

    #[test]
    fn las_fuentes_para_elegir_son_buzon_todas_tags_y_carpetas() {
        let (_dir, vault) = vault();
        let views = [SavedView {
            name: "Para hoy".into(),
            query: "#gpui".into(),
        }];
        let labels: Vec<String> = ListSource::choices(&vault.snapshot(), &vault.tags(), &views)
            .iter()
            .map(ListSource::label)
            .collect();
        assert_eq!(
            labels,
            [
                "Buzón",
                "Todas las notas",
                "Para hoy",
                "#archivado",
                "#gpui",
                "#Rust",
                "tags/",
                "ui/",
                "ui/hondo/",
            ]
        );
    }

    #[rstest]
    #[case::la_de_la_nota(None, Some("ui/hondo/viejo.md"), Ok("ui/hondo"))]
    #[case::la_de_una_nota_en_la_raiz(None, Some("parser.md"), Ok(""))]
    #[case::la_nombrada_gana(Some("ui/"), Some("parser.md"), Ok("ui"))]
    #[case::sin_nota_del_vault(
        None,
        None,
        Err("la nota no es del vault: no tiene carpeta que mostrar en la lista")
    )]
    #[case::una_nota_no_es_carpeta(
        Some("parser.md"),
        None,
        Err("parser.md no es una carpeta del vault")
    )]
    #[case::una_que_no_esta(Some("nada"), None, Err("nada no es una carpeta del vault"))]
    fn la_carpeta_a_mostrar(
        #[case] named: Option<&str>,
        #[case] note: Option<&str>,
        #[case] expected: Result<&str, &str>,
    ) {
        let (_dir, vault) = vault();
        let note = note.map(|note| VaultPath::new(note).unwrap());
        let shown = folder_to_show(named, note.as_ref(), &vault.snapshot());
        let shown = shown
            .map(|folder| folder.as_str().to_owned())
            .map_err(|error| error.to_string());
        assert_eq!(shown, expected.map(str::to_owned).map_err(str::to_owned));
    }

    #[rstest]
    #[case::una_carpeta("ui/hondo", ListSource::Folder("ui/hondo".into()))]
    #[case::la_raiz_son_todas("", ListSource::All)]
    fn la_carpeta_de_una_nota_como_fuente(#[case] folder: &str, #[case] expected: ListSource) {
        assert_eq!(
            ListSource::folder(&VaultPath::new(folder).unwrap()),
            expected
        );
    }

    #[test]
    fn una_carpeta_renombrada_sigue_siendo_la_fuente() {
        let renamed = |from: &str, to: &str| VaultChange::Renamed {
            from: VaultPath::new(from).unwrap(),
            to: VaultPath::new(to).unwrap(),
            kind: EntryKind::Folder,
        };
        let mut source = ListSource::Folder("ui/hondo".into());
        source.follow(&[renamed("otra", "nada"), renamed("ui", "vista")]);
        assert_eq!(source, ListSource::Folder("vista/hondo".into()));
        let mut tag = ListSource::Tag("ui".into());
        tag.follow(&[renamed("ui", "vista")]);
        assert_eq!(tag, ListSource::Tag("ui".into()));
    }

    #[test]
    fn un_padre_sin_notas_propias_tambien_se_elige() {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir_all(dir.path().join("tags")).unwrap();
        fs::write(dir.path().join("a.md"), "#hijo").unwrap();
        fs::write(
            dir.path().join("tags/hijo.md"),
            "---\nparents: [padre]\n---\n",
        )
        .unwrap();
        let (vault, _updates) = Vault::open(dir.path()).unwrap();
        let choices = ListSource::choices(&vault.snapshot(), &vault.tags(), &[]);
        let padre = ListSource::Tag("padre".into());
        assert!(choices.contains(&padre), "{choices:?}");
        assert_eq!(
            sorted(&padre.notes(&vault.snapshot(), &vault.tags())),
            ["a.md"]
        );
    }

    #[test]
    fn un_tag_renombrado_sigue_siendo_la_fuente() {
        let mut source = ListSource::Tag("Rust".into());
        assert!(!source.rename_tag("#gpui", "#ui"));
        assert!(source.rename_tag("#rust", "#lenguaje"));
        assert_eq!(source, ListSource::Tag("lenguaje".into()));
        let mut inbox = ListSource::Inbox;
        assert!(!inbox.rename_tag("rust", "x"));
    }

    #[test]
    fn se_guarda_como_se_lee() {
        for source in [
            ListSource::Inbox,
            ListSource::All,
            ListSource::Tag("rust".into()),
            ListSource::Folder("ui/hondo".into()),
            ListSource::View {
                name: "Para hoy".into(),
                query: "#hoy -#hecho".into(),
            },
        ] {
            let text = serde_saphyr::to_string(&source).unwrap();
            assert_eq!(serde_saphyr::from_str::<ListSource>(&text).unwrap(), source);
        }
    }

    #[test]
    fn una_vista_toma_la_consulta_del_archivo_o_vuelve_al_buzon() {
        let views = [SavedView {
            name: "gs".into(),
            query: "#gs".into(),
        }];
        let old = ListSource::View {
            name: "gs".into(),
            query: "#viejo".into(),
        };
        assert_eq!(old.resolve(&views), ListSource::of_view(&views[0]));
        let gone = ListSource::View {
            name: "otra".into(),
            query: "#x".into(),
        };
        assert_eq!(gone.resolve(&views), ListSource::Inbox);
        let tag = ListSource::Tag("gs".into());
        assert_eq!(tag.clone().resolve(&views), tag);
    }
}
