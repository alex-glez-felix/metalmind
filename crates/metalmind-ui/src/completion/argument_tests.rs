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

fn vault() -> Arc<VaultIndex> {
    index(&[
        "Mis notas",
        "Mis notas/rust.md",
        "proyectos",
        "proyectos/app",
        "proyectos/app/plan.md",
        "#ideas",
    ])
}

fn folders() -> Arc<PathCandidates> {
    Arc::new(PathCandidates::folders(vault()))
}

struct Fixture {
    tags: Arc<[TagCompletion]>,
    folders: Arc<PathCandidates>,
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
        let sources = FocusSources {
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
    fixture.folders = Arc::new(PathCandidates::folders(index(&[
        "proyectos",
        "proyectos/app",
        "proyectos/nuevo",
    ])));
    let list = fixture.open("pro", Some(list)).unwrap();
    assert_eq!(list.accept(), "proyectos/app/");
    let list = fixture.open("proy", Some(list)).unwrap();
    assert_eq!(list.accept(), "proyectos/");
}

/// Lo que escribe aceptar cada fila de `:e` tras `typed`, en orden.
fn edit_offers(typed: &str) -> Vec<String> {
    let files = Arc::new(PathCandidates::files(vault()));
    let Some(mut list) =
        ArgumentCompletions::edit(&word(typed), files, &mut FuzzyMatcher::paths(), None)
    else {
        return Vec::new();
    };
    (0..list.len())
        .map(|row| {
            list.selection_mut().select(row);
            list.accept()
        })
        .collect()
}

/// `:e` ofrece los archivos y no las carpetas, escritos como los lee: con
/// sus espacios escapados, y `#` no es un tag (#45).
#[rstest::rstest]
#[case::por_ruta("plan", &["proyectos/app/plan.md"])]
#[case::por_carpeta("proy", &["proyectos/app/plan.md"])]
#[case::con_espacio(r"Mis\ n", &[r"Mis\ notas/rust.md"])]
#[case::entre_comillas(r#""Mis n"#, &[r"Mis\ notas/rust.md"])]
#[case::nada_coincide("zzz", &[])]
fn e_ofrece_archivos(#[case] typed: &str, #[case] expected: &[&str]) {
    assert_eq!(edit_offers(typed), expected);
}

#[test]
fn e_sin_nada_escrito_no_hay_lista() {
    assert_eq!(edit_offers(""), [""; 0]);
}

/// Un lote nuevo del vault no cambia el archivo que eligió el usuario.
#[test]
fn e_sigue_con_el_archivo_elegido_con_otro_lote() {
    let mut matcher = FuzzyMatcher::paths();
    let files = |paths: &[&str]| Arc::new(PathCandidates::files(index(paths)));
    let typed = word("md");
    let first = files(&["a.md", "b.md"]);
    let mut list = ArgumentCompletions::edit(&typed, first, &mut matcher, None).unwrap();
    list.selection_mut().select(1);
    let next = files(&["0.md", "a.md", "b.md"]);
    let list = ArgumentCompletions::edit(&typed, next, &mut matcher, Some(list)).unwrap();
    assert_eq!(list.accept(), "b.md");
}
