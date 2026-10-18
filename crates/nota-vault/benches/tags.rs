//! Los tags del vault sintético de `nota_perf::tagged_vault`: 5 k y 50 k
//! notas con 200 y 2 000 páginas de tag (05 §9). Objetivos en
//! `docs/performance.md`. Correr con `cargo bench -p nota-vault --bench tags`.

#![allow(
    clippy::unwrap_used,
    reason = "en un benchmark, un unwrap que falla es un benchmark roto"
)]

use divan::Bencher;
use nota_vault::Vault;
use nota_vault::tags::{Scope, TagQuery, Tags};
use tempfile::TempDir;

const SIZES: [usize; 2] = [5_000, 50_000];

fn main() {
    divan::main();
}

/// Una página de tag cada 25 notas: 200 con 5 k.
fn pages(notes: usize) -> usize {
    notes / 25
}

fn vault(notes: usize) -> (TempDir, Vault) {
    let dir = TempDir::new().unwrap();
    nota_perf::write_files(dir.path(), &nota_perf::tagged_vault(notes, pages(notes))).unwrap();
    let (vault, _updates) = Vault::open(dir.path()).unwrap();
    (dir, vault)
}

#[divan::bench(args = SIZES, sample_count = 10)]
fn abrir_con_tags(bencher: Bencher, notes: usize) {
    let (dir, _vault) = vault(notes);
    bencher.bench(|| {
        let (vault, _updates) = Vault::open(dir.path()).unwrap();
        assert_eq!(vault.tags().graph().pages().count(), pages(notes));
        vault
    });
}

fn with_tags(bencher: Bencher, notes: usize, run: impl Fn(&Tags) -> usize + Sync) {
    let (_dir, vault) = vault(notes);
    let tags = vault.tags();
    bencher.bench(|| run(&tags));
}

#[divan::bench(args = SIZES)]
fn consulta_por_tag_con_descendientes(bencher: Bencher, notes: usize) {
    let query = TagQuery::parse("#t1 -#t3").query;
    with_tags(bencher, notes, |tags| query.notes(tags).len());
}

#[divan::bench(args = SIZES)]
fn panel_de_tags(bencher: Bencher, notes: usize) {
    with_tags(bencher, notes, |tags| {
        tags.tree(&Scope::default().filter(tags)).roots().len()
    });
}

#[divan::bench(args = SIZES)]
fn panel_en_focus(bencher: Bencher, notes: usize) {
    let scope = Scope::parse("area-1/ #t1").unwrap();
    with_tags(bencher, notes, |tags| {
        tags.tree(&scope.filter(tags)).roots().len()
    });
}

#[divan::bench(args = SIZES)]
fn candidatos_de_autocompletado(bencher: Bencher, notes: usize) {
    with_tags(bencher, notes, |tags| tags.completions().len());
}

#[divan::bench(args = SIZES)]
fn plan_de_renombrar_un_tag_con_pagina(bencher: Bencher, notes: usize) {
    with_tags(bencher, notes, |tags| {
        tags.plan_rename("#t1", "#uno")
            .unwrap()
            .changes
            .edit_count()
    });
}
