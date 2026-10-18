//! Buscar texto en vaults sintéticos de 5 k, 20 k y 50 k notas de ~1.5 KB,
//! para saber hasta dónde alcanza un grep sin índice (03 §4). Correr con
//! `cargo bench -p nota-search --bench text`.

#![allow(
    clippy::unwrap_used,
    reason = "en un benchmark, un unwrap que falla es un benchmark roto"
)]

use std::fmt::Write as _;
use std::fs;
use std::path::Path;

use divan::Bencher;
use nota_search::{Case, Syntax, TextQuery, TextSearcher};
use tempfile::TempDir;

const SIZES: [usize; 3] = [5_000, 20_000, 50_000];
const NOTES_PER_FOLDER: usize = 100;
const LINES_PER_NOTE: usize = 24;

const WORDS: [&str; 32] = [
    "la",
    "de",
    "que",
    "el",
    "en",
    "nota",
    "idea",
    "texto",
    "vault",
    "enlace",
    "tarea",
    "lista",
    "reunión",
    "diseño",
    "código",
    "lectura",
    "pendiente",
    "semana",
    "árbol",
    "búsqueda",
    "rápido",
    "lento",
    "archivo",
    "carpeta",
    "tema",
    "resumen",
    "libro",
    "capítulo",
    "pregunta",
    "respuesta",
    "proyecto",
    "cambio",
];

fn main() {
    divan::main();
}

/// Un generador congruencial: el mismo vault en cada corrida, sin depender
/// de `rand`.
struct Lcg(u64);

impl Lcg {
    fn next(&mut self) -> usize {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        usize::try_from(self.0 >> 33).unwrap()
    }
}

/// Notas de markdown con encabezado, frontmatter, links y una fecha. Una de
/// cada mil lleva `zafiro`; `proyecto` sale en casi todas.
fn note(number: usize, random: &mut Lcg) -> String {
    let mut body = format!(
        "---\ntags: [tema-{}]\n---\n# Nota {number}\n\n",
        number % 37
    );
    for line in 0..LINES_PER_NOTE {
        for _ in 0..10 {
            body.push_str(WORDS[random.next() % WORDS.len()]);
            body.push(' ');
        }
        if line % 8 == 0 {
            write!(body, "[[nota-{}]] ", random.next() % 1000).unwrap();
        }
        if line == 3 {
            write!(body, "2026-{:02}-{:02}", 1 + number % 12, 1 + number % 28).unwrap();
        }
        body.push('\n');
    }
    if number.is_multiple_of(1000) {
        body.push_str("un zafiro escondido\n");
    }
    body
}

fn synthetic_vault(root: &Path, notes: usize) {
    let mut random = Lcg(notes as u64);
    for number in 0..notes {
        let folder = number / NOTES_PER_FOLDER;
        let dir = root.join(format!("area-{}/tema-{}", folder / 10, folder % 10));
        if number.is_multiple_of(NOTES_PER_FOLDER) {
            fs::create_dir_all(&dir).unwrap();
        }
        fs::write(
            dir.join(format!("nota-{number}.md")),
            note(number, &mut random),
        )
        .unwrap();
    }
}

fn bench_query(bencher: Bencher, notes: usize, pattern: &str, syntax: Syntax) {
    let dir = TempDir::new().unwrap();
    synthetic_vault(dir.path(), notes);
    let query = TextQuery {
        pattern: pattern.to_owned(),
        syntax,
        case: Case::Smart,
        scope: None,
    };
    bencher.bench(|| {
        let mut searcher = TextSearcher::new(dir.path());
        searcher.search(&query).unwrap().iter().count()
    });
}

/// Pocas coincidencias: lo que cuesta recorrer y leer todo el vault.
#[divan::bench(args = SIZES, sample_count = 10, sample_size = 1)]
fn palabra_rara(bencher: Bencher, notes: usize) {
    bench_query(bencher, notes, "zafiro", Syntax::Literal);
}

/// Casi todas las notas coinciden: suma armar y mandar los resultados.
#[divan::bench(args = SIZES, sample_count = 10, sample_size = 1)]
fn palabra_comun(bencher: Bencher, notes: usize) {
    bench_query(bencher, notes, "proyecto", Syntax::Literal);
}

/// Una regex sin literal que la acelere y que no distingue mayúsculas.
#[divan::bench(args = SIZES, sample_count = 10, sample_size = 1)]
fn regex_de_fecha(bencher: Bencher, notes: usize) {
    bench_query(bencher, notes, r"\d{4}-\d{2}-1\d", Syntax::Regex);
}

/// Lo que tarda en volver `cancel` con la búsqueda recién empezada: lo que
/// cuesta una tecla que reemplaza la búsqueda anterior.
#[divan::bench(args = SIZES, sample_count = 10, sample_size = 1)]
fn empezar_y_cancelar(bencher: Bencher, notes: usize) {
    let dir = TempDir::new().unwrap();
    synthetic_vault(dir.path(), notes);
    let query = TextQuery {
        pattern: "proyecto".to_owned(),
        ..TextQuery::default()
    };
    bencher.bench(|| {
        let mut searcher = TextSearcher::new(dir.path());
        let found = searcher.search(&query).unwrap();
        found.recv().unwrap();
        searcher.cancel();
    });
}
