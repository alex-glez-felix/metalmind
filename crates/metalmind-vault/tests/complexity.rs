//! Lo que recorre el índice de links crece lineal con el vault. Ver
//! `docs/performance.md`.

#![allow(
    clippy::unwrap_used,
    reason = "en tests, un unwrap que falla es un test que falla"
)]

use std::collections::HashSet;
use std::fmt::Write as _;

use metalmind_perf::{NoteNames, assert_flat, assert_linear, linked_vault};
use metalmind_vault::{EntryKind, LinkIndex, NoteBatch, VaultChange, VaultIndex, VaultPath};

type Files = Vec<(VaultPath, String)>;

fn files(notes: usize, names: NoteNames) -> Files {
    linked_vault(notes, names)
        .into_iter()
        .map(|(path, text)| (VaultPath::new(&path).unwrap(), text))
        .collect()
}

fn build(files: &Files) -> LinkIndex {
    LinkIndex::from_texts(
        files
            .iter()
            .map(|(path, text)| (path.clone(), kind(path), text.as_str())),
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
#[cfg_attr(not(valgrind_available), ignore = "valgrind no está en el PATH")]
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
#[cfg_attr(not(valgrind_available), ignore = "valgrind no está en el PATH")]
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
#[cfg_attr(not(valgrind_available), ignore = "valgrind no está en el PATH")]
fn borrar_una_carpeta_es_lineal() {
    let change = VaultChange::Removed {
        path: VaultPath::new("area-3").unwrap(),
        kind: EntryKind::Folder,
    };
    assert_linear(
        1_000,
        |notes| build(&sorted(files(notes, NoteNames::Repeated))),
        |mut index| {
            index.apply(&change, &mut NoteBatch::new(&|_: &VaultPath| None));
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
#[cfg_attr(not(valgrind_available), ignore = "valgrind no está en el PATH")]
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
            index.apply(
                &change,
                &mut NoteBatch::new(&|_: &VaultPath| Some(String::new())),
            );
            index
        },
    );
}

/// Una nota de una sola línea con `links` links: el contexto de cada uno no
/// puede volver a recorrer la línea entera.
#[test]
#[cfg_attr(not(valgrind_available), ignore = "valgrind no está en el PATH")]
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
        |(path, text)| LinkIndex::from_texts([(path.clone(), EntryKind::Note, text.as_str())]),
    );
}

/// El índice de archivos de `files`, con sus carpetas.
fn file_tree(files: &Files) -> VaultIndex {
    let folders: HashSet<VaultPath> = files
        .iter()
        .flat_map(|(file, _)| std::iter::successors(file.parent(), VaultPath::parent))
        .filter(|folder| !folder.is_root())
        .collect();
    let folders = folders
        .into_iter()
        .map(|folder| (folder, EntryKind::Folder));
    let files = files.iter().map(|(path, _)| (path.clone(), kind(path)));
    VaultIndex::from_entries(folders.chain(files))
}

/// Un lote con una nota editada copia lo que toca, aunque lo publicado antes
/// siga vivo: no cuesta lo que el vault (#60). La nota lleva 200 links para que
/// escanearla pese más que lo fijo de la corrida.
#[test]
#[cfg_attr(not(valgrind_available), ignore = "valgrind no está en el PATH")]
fn publicar_una_nota_cambiada_no_depende_del_vault() {
    let path = VaultPath::new("area-3/tema-4/nota-7.md").unwrap();
    let text = (0..200)
        .map(|note| format!("[[nota-{note}]]"))
        .collect::<Vec<_>>()
        .join(" ");
    let setup = |notes| {
        let vault = files(notes, NoteNames::Repeated);
        let tree = file_tree(&vault);
        let index = build(&sorted(vault));
        (index.clone(), index, tree)
    };
    assert_flat(2_000, setup, |(published, mut index, tree)| {
        let change = VaultChange::Modified { path: path.clone() };
        let read = |_: &VaultPath| Some(text.clone());
        index.apply(&change, &mut NoteBatch::new(&read));
        // Soltar el vault viejo es lineal y no es parte de publicar.
        (published, index.clone(), index, tree.clone(), tree)
    });
}
