#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "en tests, un unwrap que falla es un test que falla"
)]

use std::collections::HashSet;
use std::fs;
use std::path::Path;
use std::sync::Arc;
use std::sync::mpsc::Receiver;

use nota_search::{Case, NoteMatches, SearchError, Syntax, TextQuery, TextSearcher};
use nota_vault::VaultPath;
use pretty_assertions::assert_eq;
use proptest::prelude::*;
use rstest::rstest;
use tempfile::TempDir;

fn vault(files: &[(&str, &[u8])]) -> TempDir {
    let dir = TempDir::new().unwrap();
    for (path, contents) in files {
        let path = dir.path().join(path);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, contents).unwrap();
    }
    dir
}

fn query(pattern: &str, syntax: Syntax, case: Case) -> TextQuery {
    TextQuery {
        pattern: pattern.to_owned(),
        syntax,
        case,
        scope: None,
    }
}

/// Cada coincidencia como `ruta:línea:columna:offset:línea`, con lo que
/// coincide entre corchetes; ordenado, porque llegan en cualquier orden.
fn rendered(found: Receiver<NoteMatches>) -> Vec<String> {
    let mut out = Vec::new();
    for note in found {
        for line in note.lines {
            for range in &line.matches {
                let text = &line.text;
                out.push(format!(
                    "{}:{}:{}:{}:{}[{}]{}",
                    note.path,
                    line.line,
                    range.start,
                    line.offset + range.start,
                    &text[..range.start],
                    &text[range.clone()],
                    &text[range.end..],
                ));
            }
        }
    }
    out.sort();
    out
}

fn search(root: &Path, query: &TextQuery) -> Vec<String> {
    rendered(TextSearcher::new(root).search(query).unwrap())
}

#[rstest]
#[case::literal("a.b", Syntax::Literal, Case::Smart, &["n.md:0:2:2:x [a.b] y"])]
#[case::regex("a.b", Syntax::Regex, Case::Smart, &["n.md:0:2:2:x [a.b] y", "n.md:1:0:8:[axb]"])]
#[case::varias_en_una_linea("o", Syntax::Literal, Case::Smart, &[
    "n.md:2:0:12:[o]so oca",
    "n.md:2:4:16:oso [o]ca",
    "n.md:2:2:14:os[o] oca",
])]
#[case::lineas_y_offsets("oca", Syntax::Literal, Case::Smart, &["n.md:2:4:16:oso [oca]"])]
#[case::coincidencias_vacias_no_cuentan("^", Syntax::Regex, Case::Smart, &[])]
#[case::patron_vacio("", Syntax::Literal, Case::Smart, &[])]
#[case::sin_coincidencias("zzz", Syntax::Literal, Case::Smart, &[])]
fn literal_y_regex(
    #[case] pattern: &str,
    #[case] syntax: Syntax,
    #[case] case: Case,
    #[case] expected: &[&str],
) {
    let dir = vault(&[("n.md", b"x a.b y\naxb\noso oca\n")]);
    let mut expected = expected.to_vec();
    expected.sort_unstable();
    assert_eq!(search(dir.path(), &query(pattern, syntax, case)), expected);
}

#[rstest]
#[case::smart_minusculas("niño", Syntax::Literal, Case::Smart, &[
    "n.md:0:0:0:[niño]",
    "n.md:1:0:6:[NIÑO]",
    "n.md:2:0:12:[Niño]",
])]
#[case::smart_con_mayuscula("Niño", Syntax::Literal, Case::Smart, &["n.md:2:0:12:[Niño]"])]
#[case::smart_mayuscula_no_ascii("NIÑO", Syntax::Literal, Case::Smart, &["n.md:1:0:6:[NIÑO]"])]
#[case::lo_escapado_no_cuenta(r"\Sola", Syntax::Regex, Case::Smart, &["n.md:3:0:18:[HOLA]"])]
#[case::sensitive("niño", Syntax::Literal, Case::Sensitive, &["n.md:0:0:0:[niño]"])]
#[case::insensitive("Niño", Syntax::Literal, Case::Insensitive, &[
    "n.md:0:0:0:[niño]",
    "n.md:1:0:6:[NIÑO]",
    "n.md:2:0:12:[Niño]",
])]
fn mayusculas(
    #[case] pattern: &str,
    #[case] syntax: Syntax,
    #[case] case: Case,
    #[case] expected: &[&str],
) {
    let dir = vault(&[("n.md", "niño\nNIÑO\nNiño\nHOLA\n".as_bytes())]);
    let mut expected = expected.to_vec();
    expected.sort_unstable();
    assert_eq!(search(dir.path(), &query(pattern, syntax, case)), expected);
}

#[rstest]
#[case::columna_en_bytes("ü", Syntax::Literal, &["n.md:0:11:11:ñandú, ñ[ü] 🦀ü", "n.md:0:18:18:ñandú, ñü 🦀[ü]"])]
#[case::emoji("🦀", Syntax::Literal, &["n.md:0:14:14:ñandú, ñü [🦀]ü"])]
#[case::punto_es_un_caracter("ñ.", Syntax::Regex, &["n.md:0:0:0:[ña]ndú, ñü 🦀ü", "n.md:0:9:9:ñandú, [ñü] 🦀ü"])]
fn multibyte(#[case] pattern: &str, #[case] syntax: Syntax, #[case] expected: &[&str]) {
    let dir = vault(&[("n.md", "ñandú, ñü 🦀ü\n".as_bytes())]);
    let mut expected = expected.to_vec();
    expected.sort_unstable();
    assert_eq!(
        search(dir.path(), &query(pattern, syntax, Case::Smart)),
        expected
    );
}

#[rstest]
#[case::fin_de_linea("o$", &["n.md:0:2:2:un[o]"])]
#[case::sin_retorno_en_la_linea("s$", &["n.md:1:2:7:do[s]"])]
#[case::el_salto_no_coincide(r"o\s", &[])]
fn crlf(#[case] pattern: &str, #[case] expected: &[&str]) {
    let dir = vault(&[("n.md", b"uno\r\ndos\r\n")]);
    assert_eq!(
        search(dir.path(), &query(pattern, Syntax::Regex, Case::Smart)),
        expected
    );
}

#[test]
fn crlf_sin_salto_final() {
    let dir = vault(&[("n.md", b"uno\r\ndos")]);
    assert_eq!(
        search(dir.path(), &query("s$", Syntax::Regex, Case::Smart)),
        ["n.md:1:2:7:do[s]"]
    );
}

#[test]
fn solo_busca_en_notas_que_nota_puede_abrir() {
    let mut big_binary = b"aguja\n".repeat(10_000);
    big_binary.push(0);
    let dir = vault(&[
        ("nota.md", b"aguja\n"),
        ("MAYUS.MD", b"aguja\n"),
        ("con-nul.md", b"aguja\0\n"),
        ("nul-al-final.md", &big_binary),
        ("latin1.md", b"aguja \xf1\n"),
        ("adjunto.txt", b"aguja\n"),
        ("imagen.png", b"aguja\n"),
        (".md", b"aguja\n"),
    ]);
    assert_eq!(
        search(dir.path(), &query("aguja", Syntax::Literal, Case::Smart)),
        ["MAYUS.MD:0:0:0:[aguja]", "nota.md:0:0:0:[aguja]"]
    );
}

#[test]
fn respeta_las_reglas_del_vault() {
    let dir = vault(&[
        (".gitignore", b"privado/\n*.borrador.md\n"),
        ("a/.ignore", b"local.md\n"),
        ("visible.md", b"aguja\n"),
        ("privado/secreto.md", b"aguja\n"),
        ("idea.borrador.md", b"aguja\n"),
        ("a/local.md", b"aguja\n"),
        ("a/otra.md", b"aguja\n"),
        (".obsidian/plantilla.md", b"aguja\n"),
        (".trash/vieja.md", b"aguja\n"),
        ("carpeta/.oculta.md", b"aguja\n"),
    ]);
    #[cfg(unix)]
    std::os::unix::fs::symlink(dir.path().join("visible.md"), dir.path().join("enlace.md"))
        .unwrap();
    assert_eq!(
        search(dir.path(), &query("aguja", Syntax::Literal, Case::Smart)),
        ["a/otra.md:0:0:0:[aguja]", "visible.md:0:0:0:[aguja]"]
    );
}

#[test]
fn el_scope_limita_las_notas() {
    let dir = vault(&[
        ("a.md", b"aguja\n"),
        ("b/c.md", b"aguja\n"),
        ("d.md", b"aguja\n"),
    ]);
    let scope = ["b/c.md", "d.md", "no-existe.md"]
        .into_iter()
        .map(|path| VaultPath::new(path).unwrap())
        .collect::<HashSet<_>>();
    let query = TextQuery {
        scope: Some(Arc::new(scope)),
        ..query("aguja", Syntax::Literal, Case::Smart)
    };
    assert_eq!(
        search(dir.path(), &query),
        ["b/c.md:0:0:0:[aguja]", "d.md:0:0:0:[aguja]"]
    );
}

#[rstest]
#[case::sin_cerrar("(")]
#[case::salto_de_linea(r"a\nb")]
fn patron_invalido(#[case] pattern: &str) {
    let dir = vault(&[("n.md", b"a\nb\n")]);
    let result = TextSearcher::new(dir.path()).search(&query(pattern, Syntax::Regex, Case::Smart));
    assert!(matches!(result, Err(SearchError::InvalidPattern(_))));
}

/// Suficientes notas para que la búsqueda no termine antes de cancelarla.
fn big_vault() -> (TempDir, usize) {
    const NOTES: usize = 4_000;
    let dir = TempDir::new().unwrap();
    for folder in 0..40 {
        let folder = dir.path().join(format!("carpeta-{folder}"));
        fs::create_dir(&folder).unwrap();
        for note in 0..NOTES / 40 {
            fs::write(
                folder.join(format!("nota-{note}.md")),
                "aguja\n".repeat(200),
            )
            .unwrap();
        }
    }
    (dir, NOTES)
}

#[test]
fn cancelar_corta_el_stream() {
    let (dir, notes) = big_vault();
    let mut searcher = TextSearcher::new(dir.path());
    let found = searcher
        .search(&query("aguja", Syntax::Literal, Case::Smart))
        .unwrap();
    found.recv().unwrap();
    searcher.cancel();
    // Después de cancelar no llega nada más: el receptor ya se desconectó.
    let received = 1 + found.iter().count();
    assert!(received < notes, "llegaron {received} de {notes}");
}

#[test]
fn una_busqueda_nueva_cancela_la_anterior() {
    let (dir, notes) = big_vault();
    fs::write(dir.path().join("otra.md"), "pajar\n").unwrap();
    let mut searcher = TextSearcher::new(dir.path());
    let old = searcher
        .search(&query("aguja", Syntax::Literal, Case::Smart))
        .unwrap();
    old.recv().unwrap();
    let new = searcher
        .search(&query("pajar", Syntax::Literal, Case::Smart))
        .unwrap();
    assert!(1 + old.iter().count() < notes);
    assert_eq!(rendered(new), ["otra.md:0:0:0:[pajar]"]);
}

#[test]
fn un_patron_invalido_no_cancela_la_anterior() {
    let dir = vault(&[("n.md", b"aguja\n")]);
    let mut searcher = TextSearcher::new(dir.path());
    let found = searcher
        .search(&query("aguja", Syntax::Literal, Case::Smart))
        .unwrap();
    assert!(
        searcher
            .search(&query("(", Syntax::Regex, Case::Smart))
            .is_err()
    );
    assert_eq!(rendered(found), ["n.md:0:0:0:[aguja]"]);
}

#[test]
fn una_busqueda_completa_trae_todas_las_notas() {
    let (dir, notes) = big_vault();
    let mut searcher = TextSearcher::new(dir.path());
    let found = searcher
        .search(&query("aguja", Syntax::Literal, Case::Smart))
        .unwrap();
    let all: Vec<_> = found.iter().collect();
    assert_eq!(all.len(), notes);
    assert!(all.iter().all(|note| note.lines.len() == 200));
}

#[test]
fn find_busca_en_texto_en_memoria() {
    let matcher = query("o", Syntax::Literal, Case::Smart).compile().unwrap();
    let found = matcher.find("uno\r\ndos\nNUL\0o").unwrap();
    let rendered: Vec<_> = found
        .iter()
        .map(|line| {
            format!(
                "{}:{}:{}:{:?}",
                line.line, line.offset, line.text, line.matches
            )
        })
        .collect();
    assert_eq!(
        rendered,
        ["0:0:uno:[2..3]", "1:5:dos:[1..2]", "2:9:NUL\0o:[4..5]"]
    );
}

#[test]
fn compile_rechaza_un_patron_invalido() {
    assert!(matches!(
        query("(", Syntax::Regex, Case::Smart).compile(),
        Err(SearchError::InvalidPattern(_))
    ));
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]

    /// Una nota en disco da lo mismo que su texto en memoria.
    #[test]
    fn find_da_lo_mismo_que_buscar_en_disco(
        text in "[aAñ\\- \r\n]{0,60}",
        pattern in "[aAñ]{1,2}",
    ) {
        let dir = vault(&[("n.md", text.as_bytes())]);
        let query = query(&pattern, Syntax::Literal, Case::Smart);
        let on_disk: Vec<_> = TextSearcher::new(dir.path())
            .search(&query)
            .unwrap()
            .into_iter()
            .flat_map(|note| note.lines)
            .collect();
        prop_assert_eq!(query.compile().unwrap().find(&text).unwrap(), on_disk);
    }
}
