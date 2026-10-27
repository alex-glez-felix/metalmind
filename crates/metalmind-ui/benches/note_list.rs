//! La lista de notas (#164) con el vault sintético de 5 k notas y 200 páginas
//! de tag de `metalmind-perf`: leer las tarjetas del disco (en el fondo, antes
//! de que la lista se vea), armar la lista de una fuente en su orden (en el
//! hilo de la UI: un frame, 16 ms) y una tecla en su filtro (8 ms, como un
//! prompt). Presupuesto en `docs/performance.md`.

#![allow(
    clippy::unwrap_used,
    reason = "en un benchmark, un unwrap que falla es un benchmark roto"
)]

use std::sync::Arc;

use divan::Bencher;
use metalmind_search::FuzzyMatcher;
use metalmind_ui::note_list::{Card, ListSource, NoteList, read_all};
use metalmind_ui::picker::LineEdit;
use metalmind_vault::{EntryKind, Vault, VaultPath};

const NOTES: usize = 5_000;
const PAGES: usize = 200;

fn main() {
    divan::main();
}

struct Fixture {
    _dir: tempfile::TempDir,
    vault: Vault,
    notes: Vec<VaultPath>,
    cards: Vec<(VaultPath, Card)>,
}

fn fixture() -> Fixture {
    let dir = tempfile::tempdir().unwrap();
    let files = metalmind_perf::tagged_vault(NOTES, PAGES);
    metalmind_perf::write_files(dir.path(), &files).unwrap();
    let (vault, _updates) = Vault::open(dir.path()).unwrap();
    let notes: Vec<VaultPath> = vault
        .snapshot()
        .files()
        .filter(|entry| entry.kind == EntryKind::Note)
        .map(|entry| entry.path.clone())
        .collect();
    let cards = read_all(vault.root(), &notes);
    Fixture {
        _dir: dir,
        vault,
        notes,
        cards,
    }
}

#[divan::bench]
fn leer_las_tarjetas_de_5k(bencher: Bencher) {
    let fixture = fixture();
    bencher.bench_local(|| read_all(fixture.vault.root(), &fixture.notes).len());
}

#[divan::bench(args = [ListSource::All, ListSource::Inbox, ListSource::Tag("t1".to_owned())])]
fn armar_la_lista_de_5k(bencher: Bencher, source: &ListSource) {
    let fixture = fixture();
    bencher.bench_local(|| {
        let mut list = NoteList::new(
            source.clone(),
            "",
            fixture.vault.snapshot(),
            fixture.vault.tags(),
        );
        let _ = list.set_cards(fixture.cards.clone());
        list.len()
    });
}

#[divan::bench(args = ["n", "#t1 -#t3 nota", "zzz"])]
fn tecla_en_el_filtro_de_5k(bencher: Bencher, query: &str) {
    let fixture = fixture();
    let mut list = NoteList::new(
        ListSource::All,
        "",
        fixture.vault.snapshot(),
        fixture.vault.tags(),
    );
    let _ = list.set_cards(fixture.cards.clone());
    let mut matcher = FuzzyMatcher::paths();
    let query = Arc::<str>::from(query);
    bencher.bench_local(|| {
        let _ = list.edit_filter(LineEdit::End);
        let _ = list.edit_filter(LineEdit::DeleteToStart);
        let mut job = list.edit_filter(LineEdit::Insert(&query)).job;
        while let Some(pending) = job {
            job = list.finish(pending.run(&mut matcher)).job;
        }
        list.len()
    });
}
