//! El índice de links de un vault sintético de 5 000 notas con 30 000 links:
//! armarlo de cero, actualizarlo, publicarlo y planear un rename. Objetivos
//! en `docs/performance.md`. Correr con `cargo bench -p metalmind-vault --bench links`.

#![allow(
    clippy::unwrap_used,
    reason = "en un benchmark, un unwrap que falla es un benchmark roto"
)]

use std::collections::HashMap;

use divan::Bencher;
use metalmind_perf::{NoteNames, linked_vault};
use metalmind_vault::{EntryKind, LinkIndex, VaultChange, VaultPath};

fn main() {
    divan::main();
}

type Files = HashMap<VaultPath, (EntryKind, String)>;

fn vault(notes: usize, names: NoteNames) -> Files {
    linked_vault(notes, names)
        .into_iter()
        .map(|(path, text)| {
            let kind = if std::path::Path::new(&path)
                .extension()
                .is_some_and(|ext| ext.eq_ignore_ascii_case("md"))
            {
                EntryKind::Note
            } else {
                EntryKind::Attachment
            };
            (VaultPath::new(&path).unwrap(), (kind, text))
        })
        .collect()
}

fn build(files: &Files) -> LinkIndex {
    LinkIndex::build(
        files.iter().map(|(path, (kind, _))| (path.clone(), *kind)),
        |path| files.get(path).map(|(_, text)| text.clone()),
    )
}

#[divan::bench(args = [5_000, 50_000], sample_count = 10)]
fn indexar_links(bencher: Bencher, notes: usize) {
    let files = vault(notes, NoteNames::Repeated);
    bencher.bench(|| build(&files));
}

#[divan::bench(args = [5_000, 50_000], sample_count = 10)]
fn indexar_links_nombres_unicos(bencher: Bencher, notes: usize) {
    let files = vault(notes, NoteNames::Unique);
    bencher.bench(|| build(&files));
}

#[divan::bench(sample_count = 100)]
fn modificar_una_nota(bencher: Bencher) {
    let index = build(&vault(5_000, NoteNames::Repeated));
    let path = VaultPath::new("area-3/tema-4/nota-7.md").unwrap();
    let text = "[[nota-8]] [[nota-9]] [[area-0/tema-0/nota-0]]".to_owned();
    let change = VaultChange::Modified { path };
    bencher
        .with_inputs(|| index.clone())
        .bench_local_refs(|index| index.apply(&change, |_| Some(text.clone())));
}

/// `nota-8` está en las 100 carpetas: 300 links de 100 notas vuelven a resolver.
#[divan::bench(sample_count = 100)]
fn crear_una_nota_que_empata(bencher: Bencher) {
    let index = build(&vault(5_000, NoteNames::Repeated));
    let change = VaultChange::Created {
        path: VaultPath::new("nota-8.md").unwrap(),
        kind: EntryKind::Note,
    };
    bencher
        .with_inputs(|| index.clone())
        .bench_local_refs(|index| index.apply(&change, |_| Some(String::new())));
}

/// Lo que paga el watcher por lote para publicar el índice.
#[divan::bench(args = [5_000, 50_000], sample_count = 20)]
fn clonar_el_indice(bencher: Bencher, notes: usize) {
    let index = build(&vault(notes, NoteNames::Repeated));
    bencher.bench(|| index.clone());
}

/// Renombrar una nota con 6 links que llegan a ella.
#[divan::bench(sample_count = 100)]
fn plan_de_renombrar_una_nota(bencher: Bencher) {
    let index = build(&vault(5_000, NoteNames::Unique));
    let from = VaultPath::new("area-3/tema-4/nota-3-4-7.md").unwrap();
    let to = VaultPath::new("area-3/tema-4/otra.md").unwrap();
    bencher.bench(|| index.rename_plan(&from, &to).unwrap());
}

/// Mover una carpeta de 500 notas: sus links relativos y los que llegan.
/// Con nombres repetidos, cada nombre movido vuelve a mirar los links que lo
/// usan en todo el vault.
#[divan::bench(args = [NoteNames::Unique, NoteNames::Repeated], sample_count = 20)]
fn plan_de_mover_una_carpeta(bencher: Bencher, names: NoteNames) {
    let index = build(&vault(5_000, names));
    let from = VaultPath::new("area-3").unwrap();
    let to = VaultPath::new("archivo/area-3").unwrap();
    bencher.bench(|| index.rename_plan(&from, &to).unwrap());
}

/// `gd` sobre el último link de una nota de 5 000 líneas: escanea la nota
/// entera, con los cambios sin guardar del buffer.
#[divan::bench(sample_count = 50)]
fn seguir_un_link_en_5k_lineas(bencher: Bencher) {
    let index = build(&vault(5_000, NoteNames::Repeated));
    let source = VaultPath::new("area-0/tema-0/nota-0.md").unwrap();
    let text = metalmind_perf::note(5_000);
    let offset = text.rfind("[[").unwrap() + 2;
    bencher.bench(|| index.follow(&source, &text, offset).unwrap());
}

/// Lo que insertaría el autocompletado de `[[` para una página de 50
/// resultados, mientras se escribe.
#[divan::bench(sample_count = 100)]
fn nombrar_50_notas_para_completar(bencher: Bencher) {
    let files = vault(5_000, NoteNames::Repeated);
    let index = build(&files);
    let source = VaultPath::new("area-0/tema-0/nota-0.md").unwrap();
    let mut targets: Vec<&VaultPath> = files.keys().collect();
    targets.sort();
    let page: Vec<&VaultPath> = targets.into_iter().step_by(100).take(50).collect();
    bencher.bench(|| {
        page.iter()
            .filter_map(|target| index.wikilink_target(&source, target))
            .count()
    });
}
