//! De dónde salen las notas de la lista: el buzón, todas, un tag con sus
//! descendientes o una carpeta con sus subcarpetas.

use metalmind_vault::tags::{TagQuery, Tags};
use metalmind_vault::{EntryKind, VaultIndex, VaultPath};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ListSource {
    /// Las notas sin tags (06 §9).
    #[default]
    Inbox,
    All,
    /// El nombre del tag, sin `#`, como se eligió.
    Tag(String),
    /// La ruta de la carpeta en el vault.
    Folder(String),
}

impl ListSource {
    /// Cómo se nombra en la cabecera y en el picker de fuentes.
    #[must_use]
    pub fn label(&self) -> String {
        match self {
            Self::Inbox => "Buzón".to_owned(),
            Self::All => "Todas las notas".to_owned(),
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
            Self::All | Self::Folder(_) => None,
        }
    }

    /// Sus notas, en cualquier orden. Un tag que ya no existe o una carpeta
    /// que ya no está no tienen ninguna.
    #[must_use]
    pub fn notes(&self, index: &VaultIndex, tags: &Tags) -> Vec<VaultPath> {
        match self {
            Self::Inbox | Self::Tag(_) => {
                let text = self.query().unwrap_or_default();
                let parsed = TagQuery::parse(&text);
                if !parsed.invalid.is_empty() {
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

    /// Lo que se puede elegir como fuente en este vault: el buzón, todas,
    /// cada tag y cada carpeta, en ese orden.
    #[must_use]
    pub fn choices(index: &VaultIndex, tags: &Tags) -> Vec<Self> {
        let mut tag_names: Vec<String> = tags
            .index()
            .keys()
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
        let labels: Vec<String> = ListSource::choices(&vault.snapshot(), &vault.tags())
            .iter()
            .map(ListSource::label)
            .collect();
        assert_eq!(
            labels,
            [
                "Buzón",
                "Todas las notas",
                "#archivado",
                "#gpui",
                "#Rust",
                "tags/",
                "ui/",
                "ui/hondo/",
            ]
        );
    }

    #[test]
    fn se_guarda_como_se_lee() {
        for source in [
            ListSource::Inbox,
            ListSource::All,
            ListSource::Tag("rust".into()),
            ListSource::Folder("ui/hondo".into()),
        ] {
            let text = serde_saphyr::to_string(&source).unwrap();
            assert_eq!(serde_saphyr::from_str::<ListSource>(&text).unwrap(), source);
        }
    }
}
