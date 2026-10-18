//! El índice de links de un vault sintético de 5 000 notas: armarlo de cero
//! y actualizar una nota. Correr con `cargo bench -p nota-vault --bench links`.

#![allow(
    clippy::unwrap_used,
    reason = "en un benchmark, un unwrap que falla es un benchmark roto"
)]

use std::collections::HashMap;

use divan::Bencher;
use nota_vault::{EntryKind, LinkIndex, VaultChange, VaultPath};

const NOTES: usize = 5_000;

fn main() {
    divan::main();
}

/// Las mismas 10 × 10 carpetas de 50 notas que `open`, cada nota con seis
/// links: por nombre, por ruta, a un heading, markdown, un embed y uno sin
/// resolver. Cada nombre se repite en las 100 carpetas, así que cada link por
/// nombre empata y lo desempata la carpeta propia: el peor caso.
fn synthetic_vault() -> HashMap<VaultPath, (EntryKind, String)> {
    vault_with_names(|_, _, note| format!("nota-{note}"))
}

/// El mismo vault, pero cada nota con un nombre distinto: lo común en un
/// vault real, donde un nombre casi nunca se repite.
fn unique_names_vault() -> HashMap<VaultPath, (EntryKind, String)> {
    vault_with_names(|top, sub, note| format!("nota-{top}-{sub}-{note}"))
}

fn vault_with_names(
    name: impl Fn(usize, usize, usize) -> String,
) -> HashMap<VaultPath, (EntryKind, String)> {
    let mut files = HashMap::new();
    for top in 0..10 {
        for sub in 0..10 {
            let dir = format!("area-{top}/tema-{sub}");
            for note in 0..NOTES / 100 {
                let next = name(top, sub, (note + 1) % (NOTES / 100));
                let elsewhere = name((top + 1) % 10, sub, note);
                let body = format!(
                    "# Nota {note}\n\nVer [[{next}]] y [[area-{}/tema-{sub}/{elsewhere}]].\n\
                     Más en [[{next}#Nota]] o [aquí]({next}.md).\n\
                     ![[imagen.png]] y [[pendiente-{note}]]\n",
                    (top + 1) % 10
                );
                let path = VaultPath::new(&format!("{dir}/{}.md", name(top, sub, note))).unwrap();
                files.insert(path, (EntryKind::Note, body));
            }
            let image = VaultPath::new(&format!("{dir}/imagen.png")).unwrap();
            files.insert(image, (EntryKind::Attachment, String::new()));
        }
    }
    files
}

fn build(files: &HashMap<VaultPath, (EntryKind, String)>) -> LinkIndex {
    LinkIndex::build(
        files.iter().map(|(path, (kind, _))| (path.clone(), *kind)),
        |path| files.get(path).map(|(_, text)| text.clone()),
    )
}

#[divan::bench(sample_count = 20)]
fn indexar_links_5k_notas(bencher: Bencher) {
    let files = synthetic_vault();
    bencher.bench(|| build(&files));
}

#[divan::bench(sample_count = 20)]
fn indexar_links_5k_notas_nombres_unicos(bencher: Bencher) {
    let files = unique_names_vault();
    bencher.bench(|| build(&files));
}

#[divan::bench(sample_count = 100)]
fn modificar_una_nota(bencher: Bencher) {
    let files = synthetic_vault();
    let index = build(&files);
    let path = VaultPath::new("area-3/tema-4/nota-7.md").unwrap();
    let text = "[[nota-8]] [[nota-9]] [[area-0/tema-0/nota-0]]".to_owned();
    let change = VaultChange::Modified { path };
    bencher
        .with_inputs(|| index.clone())
        .bench_local_refs(|index| index.apply(&change, |_| Some(text.clone())));
}

#[divan::bench(sample_count = 100)]
fn crear_una_nota_que_empata(bencher: Bencher) {
    let files = synthetic_vault();
    let index = build(&files);
    let change = VaultChange::Created {
        path: VaultPath::new("nota-8.md").unwrap(),
        kind: EntryKind::Note,
    };
    bencher
        .with_inputs(|| index.clone())
        .bench_local_refs(|index| index.apply(&change, |_| Some(String::new())));
}

/// Lo que paga el watcher por lote para publicar el índice.
#[divan::bench(sample_count = 100)]
fn clonar_el_indice(bencher: Bencher) {
    let index = build(&synthetic_vault());
    bencher.bench(|| index.clone());
}
