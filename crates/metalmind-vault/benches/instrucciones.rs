//! Los índices del vault del presupuesto (`docs/performance.md`), contados en
//! instrucciones con callgrind, como `metalmind-perf/benches/instrucciones.rs`.
//! Solo lo que corre en un hilo sobre datos en memoria: abrir el vault lee
//! el disco con varios hilos y su conteo no es determinista, así que va en
//! la preparación, fuera de la cuenta.
//! Correr con `cargo bench -p metalmind-vault --bench instrucciones`.

#![allow(
    clippy::unwrap_used,
    reason = "en un benchmark, un unwrap que falla es un benchmark roto"
)]

use std::collections::HashMap;
use std::fmt::Write as _;
use std::hint::black_box;
use std::path::Path;

use gungraun::prelude::*;
use metalmind_perf::{NoteNames, linked_vault};
use metalmind_vault::tags::{Scope, TagQuery, Tags};
use metalmind_vault::{
    Destination, EntryKind, LinkIndex, RenamePlan, Vault, VaultChange, VaultPath,
};
use tempfile::TempDir;

const NOTES: usize = 5_000;
const ATTACHMENTS: usize = 1_000;

type Files = HashMap<VaultPath, (EntryKind, String)>;

fn path(path: &str) -> VaultPath {
    VaultPath::new(path).unwrap()
}

fn files(names: NoteNames) -> Files {
    linked_vault(NOTES, names)
        .into_iter()
        .map(|(file, text)| {
            let kind = if Path::new(&file)
                .extension()
                .is_some_and(|ext| ext.eq_ignore_ascii_case("md"))
            {
                EntryKind::Note
            } else {
                EntryKind::Attachment
            };
            (path(&file), (kind, text))
        })
        .collect()
}

fn build(files: &Files) -> LinkIndex {
    LinkIndex::build(
        files.iter().map(|(path, (kind, _))| (path.clone(), *kind)),
        |path| files.get(path).map(|(_, text)| text.clone()),
    )
}

fn index(names: NoteNames) -> LinkIndex {
    build(&files(names))
}

#[library_benchmark]
#[bench::nombres_repetidos(args = (NoteNames::Repeated), setup = files)]
#[bench::nombres_unicos(args = (NoteNames::Unique), setup = files)]
fn links_indexar(files: Files) -> LinkIndex {
    black_box(build(&files))
}

// Un cambio del watcher: modificar una nota y crear una cuyo nombre usan
// 300 links de las 100 carpetas.
#[library_benchmark]
#[bench::modificar_una_nota(args = (NoteNames::Repeated), setup = modified)]
#[bench::crear_una_nota_que_empata(args = (NoteNames::Repeated), setup = created)]
fn links_cambio_del_watcher(
    (mut index, change, text): (LinkIndex, VaultChange, String),
) -> LinkIndex {
    index.apply(&change, |_| Some(text.clone()));
    black_box(index)
}

fn modified(names: NoteNames) -> (LinkIndex, VaultChange, String) {
    let change = VaultChange::Modified {
        path: path("area-3/tema-4/nota-7.md"),
    };
    let text = "[[nota-8]] [[nota-9]] [[area-0/tema-0/nota-0]]".to_owned();
    (index(names), change, text)
}

fn created(names: NoteNames) -> (LinkIndex, VaultChange, String) {
    let change = VaultChange::Created {
        path: path("nota-8.md"),
        kind: EntryKind::Note,
    };
    (index(names), change, String::new())
}

#[library_benchmark]
#[bench::notas_5k(args = (NoteNames::Repeated), setup = index)]
fn links_clonar_el_indice(index: LinkIndex) -> LinkIndex {
    black_box(index.clone())
}

// Renombrar una nota con 6 links que llegan, y mover una carpeta de 500
// notas con cada nombre en las 100 carpetas o con nombres únicos.
#[library_benchmark]
#[bench::una_nota(args = (NoteNames::Unique, "area-3/tema-4/nota-3-4-7.md", "area-3/tema-4/otra.md"), setup = rename)]
#[bench::una_carpeta_nombres_unicos(args = (NoteNames::Unique, "area-3", "archivo/area-3"), setup = rename)]
#[bench::una_carpeta_nombres_repetidos(args = (NoteNames::Repeated, "area-3", "archivo/area-3"), setup = rename)]
fn links_plan_de_renombre((index, from, to): (LinkIndex, VaultPath, VaultPath)) -> RenamePlan {
    black_box(index.rename_plan(&from, &to).unwrap())
}

fn rename(names: NoteNames, from: &str, to: &str) -> (LinkIndex, VaultPath, VaultPath) {
    (index(names), path(from), path(to))
}

// `gd` sobre el último link de una nota de 5 000 líneas.
#[library_benchmark]
#[bench::lineas_5k(args = (NoteNames::Repeated), setup = follow_setup)]
fn links_seguir_un_link((index, text, offset): (LinkIndex, String, usize)) -> Destination {
    let source = path("area-0/tema-0/nota-0.md");
    black_box(index.follow(&source, &text, offset).unwrap())
}

fn follow_setup(names: NoteNames) -> (LinkIndex, String, usize) {
    let text = metalmind_perf::note(5_000);
    let offset = text.rfind("[[").unwrap() + 2;
    (index(names), text, offset)
}

// Lo que insertaría el autocompletado de `[[` para una página de 50.
#[library_benchmark]
#[bench::pagina_de_50(args = (NoteNames::Repeated), setup = completion_setup)]
fn links_nombrar_para_completar((index, page): (LinkIndex, Vec<VaultPath>)) -> usize {
    let source = path("area-0/tema-0/nota-0.md");
    black_box(
        page.iter()
            .filter_map(|target| index.wikilink_target(&source, target))
            .count(),
    )
}

fn completion_setup(names: NoteNames) -> (LinkIndex, Vec<VaultPath>) {
    let files = files(names);
    let mut targets: Vec<VaultPath> = files.keys().cloned().collect();
    targets.sort();
    let page = targets.into_iter().step_by(100).take(50).collect();
    (build(&files), page)
}

/// Un vault abierto de verdad: lo que se mide son sus índices ya armados.
fn opened(write: impl FnOnce(&Path)) -> (TempDir, Vault) {
    let dir = TempDir::new().unwrap();
    write(dir.path());
    let (vault, _updates) = Vault::open(dir.path()).unwrap();
    (dir, vault)
}

/// `linked_vault` con 1 000 adjuntos, de los que la mitad usa alguna nota.
fn with_attachments(notes: usize) -> (TempDir, Vault) {
    opened(|root| {
        let mut files = linked_vault(notes, NoteNames::Repeated);
        let notes = files.iter_mut().filter(|(_, text)| !text.is_empty());
        for (number, (_, text)) in notes.take(ATTACHMENTS / 2).enumerate() {
            let _ = writeln!(text, "![[captura-{}.png]]", number * 2);
        }
        files
            .extend((0..ATTACHMENTS).map(|n| (format!("adjuntos/captura-{n}.png"), String::new())));
        metalmind_perf::write_files(root, &files).unwrap();
    })
}

#[library_benchmark]
#[bench::notas_5k(args = (NOTES), setup = with_attachments)]
fn adjuntos_huerfanos((_dir, vault): (TempDir, Vault)) -> usize {
    black_box(vault.orphan_attachments().len())
}

/// Una página de tag cada 25 notas, como `benches/tags.rs`.
fn tagged(notes: usize) -> (TempDir, Tags) {
    let (dir, vault) = opened(|root| {
        metalmind_perf::write_files(root, &metalmind_perf::tagged_vault(notes, notes / 25))
            .unwrap();
    });
    let tags = Tags::clone(&vault.tags());
    (dir, tags)
}

#[library_benchmark]
#[bench::consulta_con_descendientes(args = (NOTES), setup = tagged)]
fn tags_consulta((_dir, tags): (TempDir, Tags)) -> usize {
    black_box(TagQuery::parse("#t1 -#t3").query.notes(&tags).len())
}

#[library_benchmark]
#[bench::entero(args = (NOTES, None), setup = tags_in)]
#[bench::en_focus(args = (NOTES, Some("area-1/ #t1")), setup = tags_in)]
fn tags_panel((_dir, tags, scope): (TempDir, Tags, Scope)) -> usize {
    black_box(tags.tree(&scope.filter(&tags)).roots().len())
}

fn tags_in(notes: usize, scope: Option<&str>) -> (TempDir, Tags, Scope) {
    let (dir, tags) = tagged(notes);
    let scope = scope.map_or_else(Scope::default, |scope| Scope::parse(scope).unwrap());
    (dir, tags, scope)
}

#[library_benchmark]
#[bench::notas_5k(args = (NOTES), setup = tagged)]
fn tags_candidatos_de_autocompletado((_dir, tags): (TempDir, Tags)) -> usize {
    black_box(tags.completions().len())
}

#[library_benchmark]
#[bench::con_pagina(args = (NOTES), setup = tagged)]
fn tags_plan_de_renombre((_dir, tags): (TempDir, Tags)) -> usize {
    black_box(
        tags.plan_rename("#t1", "#uno")
            .unwrap()
            .changes
            .edit_count(),
    )
}

#[library_benchmark]
#[bench::notas_5k(args = (NOTES), setup = tagged)]
fn tags_clonar((_dir, tags): (TempDir, Tags)) -> Tags {
    black_box(Tags::clone(&tags))
}

library_benchmark_group!(
    name = links,
    benchmarks = [
        links_indexar,
        links_cambio_del_watcher,
        links_clonar_el_indice,
        links_plan_de_renombre,
        links_seguir_un_link,
        links_nombrar_para_completar,
        adjuntos_huerfanos
    ]
);

library_benchmark_group!(
    name = tags,
    benchmarks = [
        tags_consulta,
        tags_panel,
        tags_candidatos_de_autocompletado,
        tags_plan_de_renombre,
        tags_clonar
    ]
);

main!(library_benchmark_groups = links, tags);
