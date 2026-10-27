use metalmind_vault::tags::TagKey;
use metalmind_vault::{EntryKind, VaultIndex, VaultPath};
use metalmind_vim::argument_words;
use pretty_assertions::assert_eq;

use super::*;

fn tag(name: &str, count: usize, aliases: &[&str]) -> TagCompletion {
    TagCompletion {
        key: TagKey::new(name),
        name: name.to_owned(),
        aliases: aliases.iter().map(|&alias| alias.to_owned()).collect(),
        count,
        color: None,
        icon: None,
    }
}

fn tags() -> Arc<[TagCompletion]> {
    Arc::from([tag("Rust", 20, &["rs"]), tag("archivado", 5, &[])])
}

fn index(paths: &[&str]) -> Arc<VaultIndex> {
    let entries = paths.iter().map(|path| {
        let path = VaultPath::new(path).unwrap();
        let kind = match EntryKind::of_file(&path) {
            EntryKind::Note => EntryKind::Note,
            EntryKind::Attachment | EntryKind::Folder => EntryKind::Folder,
        };
        (path, kind)
    });
    Arc::new(VaultIndex::from_entries(entries))
}

fn folders() -> Arc<FolderCandidates> {
    Arc::new(FolderCandidates::new(index(&[
        "Mis notas",
        "Mis notas/rust.md",
        "proyectos",
        "proyectos/app",
        "#ideas",
    ])))
}

struct Fixture {
    tags: Arc<[TagCompletion]>,
    folders: Arc<FolderCandidates>,
    names: FuzzyMatcher,
    paths: FuzzyMatcher,
}

impl Fixture {
    fn new() -> Self {
        Self {
            tags: tags(),
            folders: folders(),
            names: FuzzyMatcher::text(),
            paths: FuzzyMatcher::paths(),
        }
    }

    fn open(
        &mut self,
        typed: &str,
        previous: Option<ArgumentCompletions>,
    ) -> Option<ArgumentCompletions> {
        let word = word(typed);
        let sources = ArgumentSources {
            tags: Arc::clone(&self.tags),
            folders: Arc::clone(&self.folders),
            names: &mut self.names,
            paths: &mut self.paths,
        };
        ArgumentCompletions::focus(&word, sources, previous)
    }

    /// Lo que escribe aceptar cada fila, en orden.
    fn offered(&mut self, typed: &str) -> Vec<String> {
        let Some(mut list) = self.open(typed, None) else {
            return Vec::new();
        };
        (0..list.len())
            .map(|row| {
                list.selection_mut().select(row);
                list.accept()
            })
            .collect()
    }
}

/// La última palabra de `typed`, como la da `Vim::typed_argument`.
fn word(typed: &str) -> ArgumentWord {
    argument_words(typed)
        .pop()
        .filter(|word| word.range.end == typed.len())
        .unwrap_or(ArgumentWord {
            range: typed.len()..typed.len(),
            text: String::new(),
            literal_start: false,
        })
}

/// Tras `#` o `-#` van los tags, también por sus alias, y aceptar deja la
/// forma establecida con el mismo signo.
#[rstest::rstest]
#[case::incluir("#ru", &["#Rust"])]
#[case::por_alias("#rs", &["#Rust"])]
#[case::excluir("-#ar", &["-#archivado"])]
#[case::todos("#", &["#Rust", "#archivado"])]
#[case::buzon("-#", &["-#Rust", "-#archivado"])]
fn un_tag_ofrece_tags(#[case] typed: &str, #[case] expected: &[&str]) {
    assert_eq!(Fixture::new().offered(typed), expected);
}

/// Lo demás son carpetas, escritas para que `:focus` las lea igual: con
/// sus espacios escapados y `\#` si parecen un tag (#157).
#[rstest::rstest]
#[case::por_ruta("pro", &["proyectos/", "proyectos/app/"])]
#[case::con_espacio(r"Mis\ n", &[r"Mis\ notas/"])]
#[case::entre_comillas(r#""Mis n"#, &[r"Mis\ notas/"])]
#[case::parece_un_tag(r"\#id", &[r"\#ideas/"])]
#[case::nada_coincide("zzz", &[])]
fn lo_demas_ofrece_carpetas(#[case] typed: &str, #[case] expected: &[&str]) {
    assert_eq!(Fixture::new().offered(typed), expected);
}

#[test]
fn sin_nada_escrito_no_hay_lista() {
    assert!(Fixture::new().open("proyectos/ ", None).is_none());
}

/// Un lote nuevo del vault no cambia la fila que eligió el usuario.
#[test]
fn la_fila_elegida_sigue_elegida_con_otro_lote() {
    let mut fixture = Fixture::new();
    let mut list = fixture.open("pro", None).unwrap();
    list.selection_mut().select(1);
    fixture.folders = Arc::new(FolderCandidates::new(index(&[
        "proyectos",
        "proyectos/app",
        "proyectos/nuevo",
    ])));
    let list = fixture.open("pro", Some(list)).unwrap();
    assert_eq!(list.accept(), "proyectos/app/");
    let list = fixture.open("proy", Some(list)).unwrap();
    assert_eq!(list.accept(), "proyectos/");
}
