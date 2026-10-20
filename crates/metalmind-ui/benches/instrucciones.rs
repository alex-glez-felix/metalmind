//! Lo de la UI en el presupuesto (`docs/performance.md`), contado en
//! instrucciones con callgrind, como `metalmind-perf/benches/instrucciones.rs`:
//! los modelos (finder, árbol, fórmulas, búsqueda de texto) y el editor en la
//! plataforma de pruebas de GPUI, de la tecla al frame (`render`, layout y
//! `paint`) con el sistema de texto de verdad. Sin la GPU ni `present`.
//! Cada benchmark devuelve lo que recibe, para que se suelte fuera de la
//! cuenta, y la ventana no se suelta (ver `keep`).
//! Correr con `cargo bench -p metalmind-ui --bench instrucciones`.

#![allow(
    clippy::unwrap_used,
    reason = "en un benchmark, un unwrap que falla es un benchmark roto"
)]

#[path = "../tests/support/headless.rs"]
mod headless;

use std::collections::BTreeSet;
use std::hint::black_box;
use std::sync::Arc;

use gungraun::prelude::*;
use gungraun::{Callgrind, EventKind};
use headless::{Headless, NOTE, Prepared};
use metalmind_buffer::Buffer;
use metalmind_perf::{FORMULAS, WORD};
use metalmind_search::{FuzzyMatcher, NoteMatches, TextSearcher};
use metalmind_syntax::{Lines, Styles, TextLines};
use metalmind_ui::FileTree;
use metalmind_ui::math::{Look, MetricsCache, render};
use metalmind_ui::picker::{LineEdit, PickerState};
use metalmind_ui::text_search::{OpenNote, SearchWorker, TextSearchState, row_text};
use metalmind_vault::{EntryKind, VaultChange, VaultIndex, VaultPath};
use tempfile::TempDir;

const NOTES: usize = 5_000;
/// Las filas que pinta la lista del picker por defecto.
const VISIBLE: usize = 12;

fn path(path: &str) -> VaultPath {
    VaultPath::new(path).unwrap()
}

/// Las rutas de `vault_paths`, compartidas como las tiene el picker.
fn paths(notes: usize) -> Arc<[String]> {
    metalmind_perf::vault_paths(notes).into()
}

#[library_benchmark]
#[bench::peor_consulta_5k(args = (NOTES), setup = paths)]
fn finder_tecla(paths: Arc<[String]>) -> (PickerState<String>, FuzzyMatcher, FuzzyMatcher) {
    let mut search = FuzzyMatcher::paths();
    let mut paint = FuzzyMatcher::paths();
    let mut picker = PickerState::new(paths);
    if let Some(job) = picker.edit(LineEdit::Insert("tema nota")).job {
        let _ = picker.finish(job.run(&mut search));
    }
    for row in 0..picker.len().min(VISIBLE) {
        black_box(picker.positions(row, &mut paint));
    }
    black_box((picker, search, paint))
}

/// El índice de `vault_paths(notes)` con sus carpetas; `top` cambia el
/// prefijo de las de arriba, para simular un rename.
fn index(notes: usize, top: &str) -> Arc<VaultIndex> {
    let files: Vec<VaultPath> = metalmind_perf::vault_paths(notes)
        .iter()
        .map(|file| path(&file.replacen("area", top, 1)))
        .collect();
    let folders: BTreeSet<VaultPath> = files
        .iter()
        .flat_map(VaultPath::ancestors)
        .filter(|folder| !folder.is_root())
        .collect();
    let entries = folders
        .into_iter()
        .map(|folder| (folder, EntryKind::Folder))
        .chain(files.into_iter().map(|file| {
            let kind = if file.name() == "imagen.png" {
                EntryKind::Attachment
            } else {
                EntryKind::Note
            };
            (file, kind)
        }));
    Arc::new(VaultIndex::from_entries(entries))
}

fn folders(index: &VaultIndex) -> Vec<VaultPath> {
    index
        .iter()
        .filter(|entry| entry.kind == EntryKind::Folder)
        .map(|entry| entry.path.clone())
        .collect()
}

fn expanded(notes: usize) -> FileTree {
    let index = index(notes, "area");
    let all = folders(&index);
    FileTree::with_expanded(index, all)
}

fn index_and_folders(notes: usize) -> (Arc<VaultIndex>, Vec<VaultPath>) {
    let index = index(notes, "area");
    let all = folders(&index);
    (index, all)
}

#[library_benchmark]
#[bench::notas_5k(args = (NOTES), setup = index_and_folders)]
fn arbol_construir_todo_abierto((index, all): (Arc<VaultIndex>, Vec<VaultPath>)) -> FileTree {
    black_box(FileTree::with_expanded(index, all))
}

// Cerrar y volver a abrir una carpeta con todo lo demás abierto.
#[library_benchmark]
#[bench::notas_5k(args = (NOTES), setup = expanded)]
fn arbol_expandir_y_colapsar(mut tree: FileTree) -> FileTree {
    let folder = path("area-5/tema-5");
    tree.collapse(&folder);
    tree.expand(&folder);
    black_box(tree)
}

// Poner el filtro con la peor consulta y quitarlo: una tecla en `/`.
#[library_benchmark]
#[bench::peor_consulta_5k(args = (NOTES), setup = expanded)]
fn arbol_filtrar(mut tree: FileTree) -> FileTree {
    tree.set_filter("tema nota");
    tree.set_filter("");
    black_box(tree)
}

// Un lote del watcher con el rename de las 10 carpetas de arriba.
#[library_benchmark]
#[bench::notas_5k(args = (NOTES), setup = renamed)]
fn arbol_aplicar_un_rename(
    (mut tree, changes, after): (FileTree, Vec<VaultChange>, Arc<VaultIndex>),
) -> FileTree {
    tree.apply(&changes, after);
    black_box(tree)
}

fn renamed(notes: usize) -> (FileTree, Vec<VaultChange>, Arc<VaultIndex>) {
    let changes = (0..10)
        .map(|top| VaultChange::Renamed {
            from: path(&format!("area-{top}")),
            to: path(&format!("zona-{top}")),
            kind: EntryKind::Folder,
        })
        .collect();
    (expanded(notes), changes, index(notes, "zona"))
}

/// El LaTeX de las fórmulas de `math_note(count)`, tal como las ve el editor.
fn latex(count: usize) -> Vec<(String, bool)> {
    let text = metalmind_perf::math_note(count);
    let lines = TextLines::new(&text);
    let mut styles = Styles::new(lines.line_count());
    styles.ensure(usize::MAX, &lines);
    (0..lines.line_count())
        .filter_map(|line| styles.line(line))
        .flat_map(|style| {
            style
                .math
                .iter()
                .map(|spot| (spot.latex.clone(), spot.display))
        })
        .collect()
}

fn warm(count: usize) -> (MetricsCache, Vec<(String, bool)>) {
    let formulas = latex(count);
    let cache = MetricsCache::default();
    for (latex, display) in &formulas {
        let _ = cache.get(latex, *display);
    }
    (cache, formulas)
}

fn cold(count: usize) -> (MetricsCache, Vec<(String, bool)>) {
    (MetricsCache::default(), latex(count))
}

// Medir las fórmulas de la nota en el hilo de la UI: al abrirla, cada una
// por primera vez; en cada frame después, todas en la caché.
#[library_benchmark]
#[bench::en_frio_200(args = (200), setup = cold)]
#[bench::en_caliente_200(args = (200), setup = warm)]
fn formulas_medir(
    (cache, formulas): (MetricsCache, Vec<(String, bool)>),
) -> (MetricsCache, Vec<(String, bool)>) {
    for (latex, display) in &formulas {
        let _ = black_box(cache.get(latex, *display));
    }
    black_box((cache, formulas))
}

// Pintar las 22 del spike a escala 1, como en el executor de fondo.
#[library_benchmark]
#[bench::escala_1()]
fn formulas_pintar_las_22() -> usize {
    black_box(paint_22(Look {
        size: 16.0 * 1.21,
        scale: 1.0,
        color: [0.85, 0.85, 0.88, 1.0],
        padding: 1.0,
    }))
}

// Fuera de la función medida: el closure lleva su nombre, y el
// `--toggle-collect` de gungraun apagaría la cuenta dentro de `filter_map`.
fn paint_22(look: Look) -> usize {
    FORMULAS
        .iter()
        .filter_map(|formula| black_box(render(formula.latex, formula.display, look)).ok())
        .count()
}

/// Una nota abierta de 5 k líneas con cambios sin guardar.
fn open_note() -> Vec<OpenNote> {
    let buffer = Buffer::from(metalmind_perf::note(5_000).as_str());
    vec![OpenNote {
        path: path("abierta.md"),
        text: buffer.snapshot(),
    }]
}

struct Typing {
    state: TextSearchState,
    worker: SearchWorker,
    _dir: TempDir,
}

/// Lo escrito es `WORD` menos su última letra y su búsqueda está corriendo:
/// casi todas las notas coinciden, así que es la que más tarda en cancelarse.
fn typing(notes: usize) -> Typing {
    let dir = TempDir::new().unwrap();
    metalmind_perf::write_vault(dir.path(), notes).unwrap();
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
        _dir: dir,
    }
}

// `space /`: la tecla en el hilo de la UI valida el patrón y lo manda al
// hilo de la búsqueda; lo que hace ese hilo no cuenta.
#[library_benchmark]
#[bench::notas_5k(args = (NOTES), setup = typing)]
fn busqueda_tecla_con_una_corriendo(mut typing: Typing) -> Typing {
    let search = typing
        .state
        .edit(LineEdit::Insert(&WORD[WORD.len() - 1..]))
        .unwrap();
    typing.worker.search(search);
    black_box(typing)
}

/// Un lote de 50 notas que llega mientras se busca `WORD`.
fn batch(notes: usize) -> (TextSearchState, u64, Vec<NoteMatches>, TempDir) {
    let dir = TempDir::new().unwrap();
    metalmind_perf::write_vault(dir.path(), notes).unwrap();
    let mut searcher = TextSearcher::new(dir.path());
    let mut state = TextSearchState::new();
    let search = state.edit(LineEdit::Insert(WORD)).unwrap();
    let batch = searcher
        .search(&search.query)
        .unwrap()
        .iter()
        .take(50)
        .collect();
    (state, search.version, batch, dir)
}

#[library_benchmark]
#[bench::lote_de_50(args = (500), setup = batch)]
fn busqueda_recibir_un_lote(
    (mut state, version, batch, dir): (TextSearchState, u64, Vec<NoteMatches>, TempDir),
) -> (TextSearchState, TempDir) {
    let _ = state.receive(version, batch);
    for row in 0..state.len().min(VISIBLE) {
        black_box(row_text(&state.hit(row).unwrap().line));
    }
    black_box((state, dir))
}

fn long_note(lines: usize) -> Prepared {
    Prepared::note(&metalmind_perf::note(lines))
}

fn formulas_note(count: usize) -> Prepared {
    Prepared::note(&metalmind_perf::math_note(count))
}

/// Una nota chica dentro del vault de `write_vault`: el árbol del panel y el
/// finder tienen todas sus notas.
fn in_vault(notes: usize) -> Prepared {
    Prepared::new(
        |root| {
            metalmind_perf::write_vault(root, notes).unwrap();
            std::fs::write(root.join(NOTE), metalmind_perf::note(50)).unwrap();
        },
        NOTE,
    )
}

// Arrancar: abrir la ventana y pintar el primer frame, con el vault ya
// abierto (eso lee el disco con varios hilos y va antes).
#[library_benchmark]
#[bench::nota_de_50k_lineas(args = (50_000), setup = long_note)]
#[bench::nota_con_200_formulas(args = (200), setup = formulas_note)]
#[bench::vault_de_5k_notas(args = (NOTES), setup = in_vault)]
fn editor_primer_frame(prepared: Prepared) -> TempDir {
    keep(prepared.first_frame())
}

/// La ventana abierta en `prepared`, tras teclear `keys`.
fn after(prepared: Prepared, keys: &[&str]) -> Headless {
    let mut headless = prepared.open();
    headless.type_keys(keys);
    headless
}

/// A la mitad de una nota de 50 k líneas.
const MIDDLE: &[&str] = &["2", "5", "0", "0", "0", "shift-g"];

fn insert_in_the_middle(lines: usize) -> Headless {
    after(long_note(lines), &[MIDDLE, &["i"]].concat())
}

fn normal_in_the_middle(lines: usize) -> Headless {
    after(long_note(lines), MIDDLE)
}

fn formulas_open(count: usize) -> Headless {
    after(formulas_note(count), &[])
}

// De la tecla al frame que la muestra: el motor de vim, `render`, layout y
// `paint` de la ventana.
#[library_benchmark]
#[bench::insert_50k(args = (50_000), setup = insert_in_the_middle)]
fn editor_tecla_en_insert(headless: Headless) -> TempDir {
    frame(headless, "x")
}

#[library_benchmark]
#[bench::j_50k(args = (50_000), setup = normal_in_the_middle)]
fn editor_tecla_en_normal(headless: Headless) -> TempDir {
    frame(headless, "j")
}

// Media pantalla de líneas que GPUI no tenía en caché; con las fórmulas,
// las que entran se miden (sus imágenes se pintan en el fondo, después).
#[library_benchmark]
#[bench::nota_de_50k_lineas(args = (50_000), setup = normal_in_the_middle)]
#[bench::cache_fria_200_formulas(args = (200), setup = formulas_open)]
fn editor_ctrl_d(headless: Headless) -> TempDir {
    frame(headless, "ctrl-d")
}

fn frame(mut headless: Headless, key: &str) -> TempDir {
    headless.key_to_frame(key);
    keep(headless)
}

/// La ventana queda sin soltar: soltarla espera a los hilos del vault, que
/// bajo valgrind no hace falta, y el proceso del benchmark sale enseguida.
/// La carpeta del vault vuelve, y gungraun la borra fuera de la cuenta.
fn keep(headless: Headless) -> TempDir {
    black_box(headless).leak_window()
}

library_benchmark_group!(
    name = modelos,
    benchmarks = [
        finder_tecla,
        arbol_construir_todo_abierto,
        arbol_expandir_y_colapsar,
        arbol_filtrar,
        arbol_aplicar_un_rename,
        formulas_medir,
        formulas_pintar_las_22,
        busqueda_tecla_con_una_corriendo,
        busqueda_recibir_un_lote
    ]
);

library_benchmark_group!(
    name = editor,
    benchmarks = [
        editor_primer_frame,
        editor_tecla_en_insert,
        editor_tecla_en_normal,
        editor_ctrl_d
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
    library_benchmark_groups = [modelos, editor]
);
