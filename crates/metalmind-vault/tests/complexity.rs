//! Lo que recorre el índice de links crece lineal con el vault. Ver
//! `docs/performance.md`.

#![allow(
    clippy::unwrap_used,
    reason = "en tests, un unwrap que falla es un test que falla"
)]

use std::fmt::Write as _;

use metalmind_perf::{NoteNames, assert_linear, linked_vault};
use metalmind_vault::{EntryKind, LinkIndex, VaultChange, VaultPath};

type Files = Vec<(VaultPath, String)>;

fn files(notes: usize, names: NoteNames) -> Files {
    linked_vault(notes, names)
        .into_iter()
        .map(|(path, text)| (VaultPath::new(&path).unwrap(), text))
        .collect()
}

fn build(files: &Files) -> LinkIndex {
    LinkIndex::build(
        files.iter().map(|(path, _)| (path.clone(), kind(path))),
        |path| {
            files
                .binary_search_by(|(file, _)| file.cmp(path))
                .ok()
                .map(|at| files[at].1.clone())
        },
    )
}

fn kind(path: &VaultPath) -> EntryKind {
    if std::path::Path::new(path.as_str())
        .extension()
        .is_some_and(|ext| ext.eq_ignore_ascii_case("md"))
    {
        EntryKind::Note
    } else {
        EntryKind::Attachment
    }
}

fn sorted(mut files: Files) -> Files {
    files.sort();
    files
}

#[test]
fn armar_el_indice_es_lineal() {
    for names in [NoteNames::Repeated, NoteNames::Unique] {
        assert_linear(
            500,
            |notes| sorted(files(notes, names)),
            |files| build(&files),
        );
    }
}

/// Mover `area-3`, con la décima parte de las notas.
#[test]
fn planear_mover_una_carpeta_es_lineal() {
    let from = VaultPath::new("area-3").unwrap();
    let to = VaultPath::new("archivo/area-3").unwrap();
    assert_linear(
        500,
        |notes| build(&sorted(files(notes, NoteNames::Repeated))),
        |index| index.rename_plan(&from, &to).unwrap(),
    );
}

#[test]
fn borrar_una_carpeta_es_lineal() {
    let change = VaultChange::Removed {
        path: VaultPath::new("area-3").unwrap(),
        kind: EntryKind::Folder,
    };
    assert_linear(
        1_000,
        |notes| build(&sorted(files(notes, NoteNames::Repeated))),
        |mut index| {
            index.apply(&change, |_| None);
            index
        },
    );
}

/// Una `index.md` en cada carpeta, cada una con `[[index]]`: el nombre
/// empata en todas y lo desempata la carpeta de la nota.
fn folder_notes(folders: usize) -> Files {
    sorted(
        (0..folders)
            .flat_map(|folder| {
                [
                    (format!("c{folder}/index.md"), "[[index]] [[x]]"),
                    (format!("c{folder}/x.md"), "[[index]]"),
                ]
            })
            .map(|(path, text)| (VaultPath::new(&path).unwrap(), text.to_owned()))
            .collect(),
    )
}

#[test]
fn un_nombre_en_cada_carpeta_es_lineal() {
    assert_linear(500, folder_notes, |files| build(&files));
    let change = VaultChange::Created {
        path: VaultPath::new("c0/sub/index.md").unwrap(),
        kind: EntryKind::Note,
    };
    assert_linear(
        1_000,
        |folders| build(&folder_notes(folders)),
        |mut index| {
            index.apply(&change, |_| Some(String::new()));
            index
        },
    );
}

/// Una nota de una sola línea con `links` links: el contexto de cada uno no
/// puede volver a recorrer la línea entera.
#[test]
fn muchos_links_en_una_linea_es_lineal() {
    let path = VaultPath::new("larga.md").unwrap();
    assert_linear(
        1_000,
        |links| {
            let mut text = String::new();
            for n in 0..links {
                write!(text, "[[nota-{n}]] y ").unwrap();
            }
            (path.clone(), text)
        },
        |(path, text)| LinkIndex::build([(path.clone(), EntryKind::Note)], |_| Some(text.clone())),
    );
}
