//! Los tags del vault sintético de M2 con 200 páginas de tag (05 §9):
//! índice y grafo completos al abrir (objetivo: < 200 ms) y una consulta por
//! un tag con descendientes (objetivo: < 16 ms). Correr con
//! `cargo bench -p nota-vault --bench tags`.

#![allow(
    clippy::unwrap_used,
    reason = "en un benchmark, un unwrap que falla es un benchmark roto"
)]

use std::fs;
use std::path::Path;

use divan::Bencher;
use nota_vault::Vault;
use nota_vault::tags::{TagKey, TagQuery};
use tempfile::TempDir;

const NOTES: usize = 5_000;
const PAGES: usize = 200;

fn main() {
    divan::main();
}

/// Las páginas forman un árbol: `t<i>` tiene de padre a `t<i/10>`, y las que
/// son múltiplo de 7 también a `t1`, para que haya varios padres.
fn page(index: usize) -> String {
    let mut parents = Vec::new();
    if index > 0 {
        parents.push(format!("t{}", index / 10));
    }
    if index.is_multiple_of(7) && index > 1 {
        parents.push("t1".to_owned());
    }
    format!(
        "---\ncolor: naranja\naliases: [a{index}]\nparents: [{}]\n---\nDe qué trata #t{index}.\n",
        parents.join(", ")
    )
}

/// 10 carpetas × 10 subcarpetas × 50 notas, cada una con dos tags en el
/// texto, uno en `tags:` y un alias; una de cada diez sin tags.
fn synthetic_vault(root: &Path) {
    fs::create_dir_all(root.join("tags")).unwrap();
    for index in 0..PAGES {
        fs::write(root.join(format!("tags/t{index}.md")), page(index)).unwrap();
    }
    for top in 0..10 {
        for sub in 0..10 {
            let dir = root.join(format!("area-{top}/tema-{sub}"));
            fs::create_dir_all(&dir).unwrap();
            for note in 0..NOTES / 100 {
                let n = top * 500 + sub * 50 + note;
                let body = if n.is_multiple_of(10) {
                    format!("# Nota {n}\n\nSin tags.\n")
                } else {
                    format!(
                        "---\ntags: [t{}]\n---\n# Nota {n}\n\nVer #t{} y #T{}, también #a{}.\n",
                        n % PAGES,
                        (n * 7) % PAGES,
                        (n * 13) % PAGES,
                        (n * 3) % PAGES,
                    )
                };
                fs::write(dir.join(format!("nota-{note}.md")), body).unwrap();
            }
        }
    }
}

#[divan::bench(sample_count = 20)]
fn abrir_5k_notas_con_tags(bencher: Bencher) {
    let dir = TempDir::new().unwrap();
    synthetic_vault(dir.path());
    bencher.bench(|| {
        let (vault, _updates) = Vault::open(dir.path()).unwrap();
        assert_eq!(vault.tags().graph().pages().count(), PAGES);
        vault
    });
}

#[divan::bench(args = ["#t1", "#t1 -#t3", "-#"])]
fn consulta(bencher: Bencher, query: &str) {
    let dir = TempDir::new().unwrap();
    synthetic_vault(dir.path());
    let (vault, _updates) = Vault::open(dir.path()).unwrap();
    let tags = vault.tags();
    let query = TagQuery::parse(query).query;
    bencher.bench(|| query.notes(&tags).len());
}

#[divan::bench]
fn renombrar_un_tag_con_pagina(bencher: Bencher) {
    let dir = TempDir::new().unwrap();
    synthetic_vault(dir.path());
    let (vault, _updates) = Vault::open(dir.path()).unwrap();
    let tags = vault.tags();
    assert!(tags.count(&TagKey::new("t1")) > 0);
    bencher.bench(|| tags.plan_rename("#t1", "#uno").unwrap().edit_count());
}
