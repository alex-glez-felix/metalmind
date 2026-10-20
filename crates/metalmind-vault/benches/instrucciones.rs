//! Los índices del vault del presupuesto (`docs/performance.md`), contados en
//! instrucciones con callgrind, como `metalmind-perf/benches/instrucciones.rs`.
//! Solo lo que corre en un hilo sobre datos en memoria: gungraun cuenta el
//! hilo de la función medida. Abrir el vault (disco, varios hilos) y armar
//! el índice de links, que reparte el scan en un hilo por núcleo, van en la
//! preparación; que armarlo siga lineal lo cuenta, con todos sus hilos,
//! `tests/complexity.rs`.
//! Cada benchmark devuelve lo que recibe, para que se suelte fuera de la
//! cuenta: soltar un índice de 5 k notas cuesta más que lo que se mide.
//! Correr con `cargo bench -p metalmind-vault --bench instrucciones`.

#![allow(
    clippy::unwrap_used,
    reason = "en un benchmark, un unwrap que falla es un benchmark roto"
)]
#![allow(
    clippy::must_use_candidate,
    reason = "lo pide la función que genera `library_benchmark(config = …)`"
)]

use std::collections::HashMap;
use std::fmt::Write as _;
use std::hint::black_box;
use std::path::Path;

use gungraun::prelude::*;
use gungraun::{Callgrind, EventKind};
use metalmind_perf::{NoteNames, linked_vault};
use metalmind_vault::tags::{Scope, TagCompletion, TagPlan, TagQuery, TagTree, Tags};
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

// Un cambio del watcher: modificar una nota y crear una cuyo nombre usan
// 300 links de las 100 carpetas.
#[library_benchmark(config = limit(metalmind_perf::MAX_NOISY_INSTRUCTION_REGRESSION))]
#[bench::modificar_una_nota(args = (NoteNames::Repeated), setup = modified)]
#[bench::crear_una_nota_que_empata(args = (NoteNames::Repeated), setup = created)]
fn links_cambio_del_watcher(
    (mut index, change, text): (LinkIndex, VaultChange, String),
) -> LinkIndex {
    apply(&mut index, &change, &text);
    black_box(index)
}

// Los closures van fuera de las funciones medidas: el nombre de lo que los
// recibe (`apply::<…::links_cambio_del_watcher::{closure}>`) contiene el de
// la función medida, y el `--toggle-collect` de gungraun apagaría la cuenta
// al entrar ahí.
fn apply(index: &mut LinkIndex, change: &VaultChange, text: &str) {
    index.apply(change, |_| Some(text.to_owned()));
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
fn links_clonar_el_indice(index: LinkIndex) -> (LinkIndex, LinkIndex) {
    black_box((index.clone(), index))
}

// Renombrar una nota con 6 links que llegan, y mover una carpeta de 500
// notas con cada nombre en las 100 carpetas o con nombres únicos.
#[library_benchmark]
#[bench::una_nota(args = (NoteNames::Unique, "area-3/tema-4/nota-3-4-7.md", "area-3/tema-4/otra.md"), setup = rename)]
#[bench::una_carpeta_nombres_repetidos(args = (NoteNames::Repeated, "area-3", "archivo/area-3"), setup = rename)]
fn links_plan_de_renombre(
    (index, from, to): (LinkIndex, VaultPath, VaultPath),
) -> (RenamePlan, LinkIndex) {
    black_box((index.rename_plan(&from, &to).unwrap(), index))
}

// Mover una carpeta con nombres únicos varía más entre corridas (ver
// `MAX_NOISY_INSTRUCTION_REGRESSION`).
#[library_benchmark(config = limit(metalmind_perf::MAX_NOISY_INSTRUCTION_REGRESSION))]
#[bench::una_carpeta_nombres_unicos(args = (NoteNames::Unique, "area-3", "archivo/area-3"), setup = rename)]
fn links_plan_de_mover_con_nombres_unicos(
    (index, from, to): (LinkIndex, VaultPath, VaultPath),
) -> (RenamePlan, LinkIndex) {
    black_box((index.rename_plan(&from, &to).unwrap(), index))
}

fn rename(names: NoteNames, from: &str, to: &str) -> (LinkIndex, VaultPath, VaultPath) {
    (index(names), path(from), path(to))
}

// `gd` sobre el último link de una nota de 5 000 líneas.
#[library_benchmark(config = limit(metalmind_perf::MAX_NOISY_INSTRUCTION_REGRESSION))]
#[bench::lineas_5k(args = (NoteNames::Repeated), setup = follow_setup)]
fn links_seguir_un_link(
    (index, text, offset): (LinkIndex, String, usize),
) -> (Destination, LinkIndex, String) {
    let source = path("area-0/tema-0/nota-0.md");
    black_box((index.follow(&source, &text, offset).unwrap(), index, text))
}

fn follow_setup(names: NoteNames) -> (LinkIndex, String, usize) {
    let text = metalmind_perf::note(5_000);
    let offset = text.rfind("[[").unwrap() + 2;
    (index(names), text, offset)
}

// Lo que insertaría el autocompletado de `[[` para una página de 50.
#[library_benchmark]
#[bench::pagina_de_50(args = (NoteNames::Repeated), setup = completion_setup)]
fn links_nombrar_para_completar(
    (index, page): (LinkIndex, Vec<VaultPath>),
) -> (usize, LinkIndex, Vec<VaultPath>) {
    let source = path("area-0/tema-0/nota-0.md");
    black_box((complete(&index, &source, &page), index, page))
}

fn complete(index: &LinkIndex, source: &VaultPath, page: &[VaultPath]) -> usize {
    page.iter()
        .filter_map(|target| index.wikilink_target(source, target))
        .count()
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
fn adjuntos_huerfanos((dir, vault): (TempDir, Vault)) -> (Vec<VaultPath>, TempDir, Vault) {
    black_box((vault.orphan_attachments(), dir, vault))
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
fn tags_consulta((dir, tags): (TempDir, Tags)) -> (TempDir, Tags) {
    // Lo que sale toma prestado `tags`: no puede volver con él, y no se suelta.
    std::mem::forget(black_box(TagQuery::parse("#t1 -#t3").query.notes(&tags)));
    black_box((dir, tags))
}

#[library_benchmark]
#[bench::entero(args = (NOTES, None), setup = tags_in)]
#[bench::en_focus(args = (NOTES, Some("area-1/ #t1")), setup = tags_in)]
fn tags_panel((dir, tags, scope): (TempDir, Tags, Scope)) -> (TagTree, TempDir, Tags) {
    let filter = scope.filter(&tags);
    let tree = tags.tree(&filter);
    // Toma prestado `tags`: no puede volver con él, y no se suelta.
    std::mem::forget(filter);
    black_box((tree, dir, tags))
}

fn tags_in(notes: usize, scope: Option<&str>) -> (TempDir, Tags, Scope) {
    let (dir, tags) = tagged(notes);
    let scope = scope.map_or_else(Scope::default, |scope| Scope::parse(scope).unwrap());
    (dir, tags, scope)
}

#[library_benchmark]
#[bench::notas_5k(args = (NOTES), setup = tagged)]
fn tags_candidatos_de_autocompletado(
    (dir, tags): (TempDir, Tags),
) -> (Vec<TagCompletion>, TempDir, Tags) {
    black_box((tags.completions(), dir, tags))
}

#[library_benchmark]
#[bench::con_pagina(args = (NOTES), setup = tagged)]
fn tags_plan_de_renombre((dir, tags): (TempDir, Tags)) -> (TagPlan, TempDir, Tags) {
    black_box((tags.plan_rename("#t1", "#uno").unwrap(), dir, tags))
}

#[library_benchmark]
#[bench::notas_5k(args = (NOTES), setup = tagged)]
fn tags_clonar((dir, tags): (TempDir, Tags)) -> (Tags, TempDir, Tags) {
    black_box((Tags::clone(&tags), dir, tags))
}

library_benchmark_group!(
    name = links,
    benchmarks = [
        links_cambio_del_watcher,
        links_clonar_el_indice,
        links_plan_de_renombre,
        links_plan_de_mover_con_nombres_unicos,
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

/// El límite de regresión contra la base, en instrucciones.
fn limit(percent: f64) -> LibraryBenchmarkConfig {
    let mut config = LibraryBenchmarkConfig::default();
    config.tool(Callgrind::default().soft_limits([(EventKind::Ir, percent)]));
    config
}

main!(
    config = limit(metalmind_perf::MAX_INSTRUCTION_REGRESSION),
    library_benchmark_groups = [links, tags]
);
