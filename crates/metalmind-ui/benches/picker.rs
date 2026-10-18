//! Una tecla en el finder con las 5 100 rutas del vault sintético de
//! `metalmind-vault/benches/open.rs`: filtrar todo y preparar las filas que se
//! ven. Tiene que caber holgado en un frame (16 ms, `docs/performance.md`).

use std::sync::Arc;

use divan::Bencher;
use metalmind_search::FuzzyMatcher;
use metalmind_ui::picker::{LineEdit, PickerState};

/// Las filas que pinta la lista del picker por defecto.
const VISIBLE: usize = 12;

fn main() {
    divan::main();
}

fn synthetic_paths() -> Arc<[String]> {
    let mut paths = Vec::new();
    for top in 0..10 {
        for sub in 0..10 {
            for note in 0..50 {
                paths.push(format!("area-{top}/tema-{sub}/nota-{note}.md"));
            }
            paths.push(format!("area-{top}/tema-{sub}/imagen.png"));
        }
    }
    paths.into()
}

#[divan::bench(args = ["n", "tema nota", "a3t7n42", "zzz"])]
fn tecla_en_5k_rutas(bencher: Bencher, query: &str) {
    let paths = synthetic_paths();
    let mut search = FuzzyMatcher::paths();
    let mut paint = FuzzyMatcher::paths();
    bencher.bench_local(|| {
        let mut picker = PickerState::new(Arc::clone(&paths));
        if let Some(job) = picker.edit(LineEdit::Insert(query)).job {
            let _ = picker.finish(job.run(&mut search));
        }
        let rows = picker.len().min(VISIBLE);
        for row in 0..rows {
            divan::black_box(picker.positions(row, &mut paint));
        }
        picker.len()
    });
}
