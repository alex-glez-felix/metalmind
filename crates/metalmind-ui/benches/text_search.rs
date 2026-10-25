//! `space /` en los vaults sintéticos de `metalmind_perf::write_vault` (5 k y
//! 50 k notas de ~1.5 KB), con una nota abierta de 5 k líneas con cambios sin
//! guardar. Lo que corre en el hilo de la UI tiene los 8 ms de un prompt
//! (`docs/performance.md`); cancelar la búsqueda anterior y lanzar la nueva va
//! en el hilo de `SearchWorker` y se mide como latencia hasta el primer
//! resultado. Correr con `cargo bench -p metalmind-ui --bench text_search`.

#![allow(
    clippy::unwrap_used,
    reason = "en un benchmark, un unwrap que falla es un benchmark roto"
)]

use divan::Bencher;
use metalmind_buffer::Buffer;
use metalmind_perf::WORD;
use metalmind_search::{NoteMatches, TextSearcher};
use metalmind_ui::picker::LineEdit;
use metalmind_ui::text_search::{Event, OpenNote, SearchWorker, TextSearchState, row_text};
use metalmind_vault::VaultPath;
use tempfile::TempDir;

const NOTES: [usize; 2] = [5_000, 50_000];
/// Las filas que pinta la lista del picker por defecto.
const VISIBLE: usize = 12;

fn main() {
    divan::main();
}

fn vault(notes: usize) -> TempDir {
    let dir = TempDir::new().unwrap();
    metalmind_perf::write_vault(dir.path(), notes).unwrap();
    dir
}

fn open_note() -> Vec<OpenNote> {
    let buffer = Buffer::from(metalmind_perf::note(5_000).as_str());
    vec![OpenNote {
        path: VaultPath::new("abierta.md").unwrap(),
        text: buffer.snapshot(),
    }]
}

struct Typing {
    state: TextSearchState,
    worker: SearchWorker,
    events: async_channel::Receiver<Event>,
}

/// Lo escrito es `WORD` menos su última letra, y su búsqueda está corriendo
/// y ya entregó notas: casi todas coinciden, así que es la que más tarda en
/// cancelarse.
fn typing(dir: &TempDir) -> Typing {
    let (sender, events) = async_channel::unbounded();
    let worker = SearchWorker::spawn(dir.path().to_owned(), open_note(), sender).unwrap();
    let mut state = TextSearchState::new();
    let search = state
        .edit(LineEdit::Insert(&WORD[..WORD.len() - 1]))
        .unwrap();
    worker.search(search);
    events.recv_blocking().unwrap();
    Typing {
        state,
        worker,
        events,
    }
}

/// Una tecla en el hilo de la UI: validar el patrón y mandarlo al hilo.
#[divan::bench(args = NOTES, sample_count = 20, sample_size = 1)]
fn tecla_con_una_busqueda_corriendo(bencher: Bencher, notes: usize) {
    let dir = vault(notes);
    let last = &WORD[WORD.len() - 1..];
    bencher
        .with_inputs(|| typing(&dir))
        .bench_local_values(|mut typing| {
            let search = typing.state.edit(LineEdit::Insert(last)).unwrap();
            typing.worker.search(search);
            typing
        });
}

/// De la tecla a la primera nota de lo escrito, con la búsqueda anterior
/// corriendo: cancelarla, buscar en la nota abierta y lanzar la nueva. No
/// bloquea la UI.
#[divan::bench(args = NOTES, sample_count = 20, sample_size = 1)]
fn de_la_tecla_al_primer_resultado(bencher: Bencher, notes: usize) {
    let dir = vault(notes);
    let last = &WORD[WORD.len() - 1..];
    bencher
        .with_inputs(|| typing(&dir))
        .bench_local_values(|mut typing| {
            let search = typing.state.edit(LineEdit::Insert(last)).unwrap();
            let version = search.version;
            typing.worker.search(search);
            loop {
                if let Event::Found { version: v, .. } = typing.events.recv_blocking().unwrap()
                    && v == version
                {
                    break;
                }
            }
            typing
        });
}

/// Un lote de 50 notas que llega mientras se busca `WORD`, con las filas
/// visibles que hay que preparar para pintarlo.
#[divan::bench(sample_count = 50)]
fn recibir_un_lote_y_preparar_las_filas(bencher: Bencher) {
    let dir = vault(500);
    let mut searcher = TextSearcher::new(dir.path());
    let mut probe = TextSearchState::new();
    let query = probe.edit(LineEdit::Insert(WORD)).unwrap().query;
    // La búsqueda llega en cualquier orden: ordenada, cada corrida mide las
    // mismas 50 notas.
    let mut batch: Vec<NoteMatches> = searcher.search(&query).unwrap().iter().collect();
    batch.sort_by(|a, b| a.path.cmp(&b.path));
    batch.truncate(50);
    bencher
        .with_inputs(|| {
            let mut state = TextSearchState::new();
            let version = state.edit(LineEdit::Insert(WORD)).unwrap().version;
            (state, version, batch.clone())
        })
        .bench_local_values(|(mut state, version, batch)| {
            let _ = state.receive(version, batch);
            for row in 0..state.len().min(VISIBLE) {
                divan::black_box(row_text(&state.hit(row).unwrap().line));
            }
            state
        });
}
