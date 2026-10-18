#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "en un benchmark, un unwrap que falla es una corrida que falla"
)]

//! Arranque y render de la app real, en un Xvfb propio como el smoke: cada
//! `metalmind` corre con `--frame-log` y los tiempos salen de su registro.
//!
//!   cargo bench -p metalmind --bench ui
//!
//! Necesita lo mismo que el smoke (Xvfb, un driver Vulkan y una fuente; ver
//! `docs/testing.md`). Bajo lavapipe el render es por CPU: los números sirven
//! para comparar corridas en la misma máquina, no contra el presupuesto de
//! `docs/performance.md`, que es con GPU.

mod frames;
#[allow(
    dead_code,
    reason = "el benchmark usa solo una parte del harness del smoke"
)]
#[path = "../../tests/support/mod.rs"]
mod support;

use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};

use frames::{Frame, Summary};
use support::{Metalmind, Smoke, Window};
use tempfile::TempDir;

/// Arranques medidos en cada Xvfb, después de uno que se descarta.
const LAUNCHES: usize = 7;
const KEYS: usize = 60;
const BURST: usize = 200;
const BIG_NOTE: usize = 50_000;
const VAULT: usize = 5_000;
const FRAME_TIMEOUT: Duration = Duration::from_secs(15);
/// Lo que tarda la primera ventana: lavapipe tarda en levantar.
const WINDOW_TIMEOUT: Duration = Duration::from_mins(1);

fn main() {
    println!("| Medición | p50 | p95 | máx | muestras |\n|---|---|---|---|---|");
    startup("archivo chico", |smoke| {
        smoke.write("a.md", "# Nota\n\nUna nota chica.\n");
    });
    startup("50 k líneas", |smoke| {
        smoke.write("a.md", &metalmind_perf::note(BIG_NOTE));
    });
    finder_in_vault();
    keys_on_big_note();
}

/// Un Xvfb y un `HOME` vacío para varios arranques, y el registro de frames
/// fuera de la carpeta de notas, donde el finder no lo ve.
struct Run {
    smoke: Smoke,
    logs: TempDir,
}

impl Run {
    fn new(fill: impl Fn(&Smoke)) -> Self {
        let smoke = Smoke::new();
        fill(&smoke);
        // Que la escritura diferida de las notas no caiga sobre la medición.
        let synced = Command::new("sync").status().expect("correr sync");
        assert!(synced.success(), "sync falló");
        Self {
            smoke,
            logs: tempfile::tempdir().expect("carpeta del registro"),
        }
    }

    fn log(&self) -> PathBuf {
        self.logs.path().join("frames.log")
    }

    /// Lanza `metalmind a.md` y espera su primer frame.
    fn launch(&self) -> (Metalmind<'_>, Window<'_>, Frame) {
        let log = self.log();
        let log_arg = log.to_str().expect("ruta UTF-8");
        let mut metalmind = self.smoke.launch(&["--frame-log", log_arg, "a.md"]);
        let window = metalmind.window();
        let first = frames::wait_first(&log, WINDOW_TIMEOUT);
        (metalmind, window, first)
    }

    /// El primer arranque de cada Xvfb sale a veces varias veces más lento
    /// y otras no; no se cuenta.
    fn warm_up(&self) {
        let (metalmind, window, _) = self.launch();
        quit(metalmind, window);
    }
}

fn quit(mut metalmind: Metalmind<'_>, window: Window<'_>) {
    window.type_keys(":q!<CR>");
    assert!(metalmind.wait_exit().success(), "{}", metalmind.log());
}

fn startup(what: &str, fill: impl Fn(&Smoke)) {
    let run = Run::new(fill);
    run.warm_up();
    let times: Vec<u64> = (0..LAUNCHES)
        .map(|_| {
            let (metalmind, window, first) = run.launch();
            quit(metalmind, window);
            first.t
        })
        .collect();
    startup_row(what, times);
}

fn startup_row(what: &str, times: Vec<u64>) {
    row(
        &format!("Arranque al primer frame, {what}"),
        &Summary::of_micros(times),
        LAUNCHES,
    );
}

/// El vault se abre antes de la ventana, así que el arranque lo incluye. El
/// finder se mide desde `space f` hasta que la nota elegida está en el
/// título, desde el benchmark y con su sondeo de 10 ms.
fn finder_in_vault() {
    let run = Run::new(|smoke| {
        metalmind_perf::write_vault(&smoke.notes(), VAULT).expect("escribir el vault");
        smoke.write("a.md", "# Nota\n");
    });
    run.warm_up();
    let target = "area-9/tema-9/nota-49.md";
    let mut startups = Vec::new();
    let mut finder = Vec::new();
    for _ in 0..LAUNCHES {
        let (metalmind, window, first) = run.launch();
        startups.push(first.t);
        let start = Instant::now();
        window.type_keys(&format!("<Space>f{target}<CR>"));
        window.wait_title("de la nota elegida", |title| title.starts_with(target));
        finder.push(micros(start.elapsed()));
        quit(metalmind, window);
    }
    startup_row("en un vault de 5 k notas", startups);
    row(
        "`space f`, la ruta de una nota y `enter`, vault de 5 k notas",
        &Summary::of_micros(finder),
        LAUNCHES,
    );
}

fn keys_on_big_note() {
    let run = Run::new(|smoke| smoke.write("a.md", &metalmind_perf::note(BIG_NOTE)));
    run.warm_up();
    let (metalmind, window, _) = run.launch();
    let log = run.log();
    press(&window, &log, "25000G");
    press(&window, &log, "i");
    let typed: Vec<Frame> = "abcdefghij"
        .chars()
        .cycle()
        .take(KEYS)
        .map(|c| press(&window, &log, &c.to_string()))
        .collect();
    press(&window, &log, "<Esc>");
    key_rows("en Insert", &typed);

    let motions = ["j", "w", "j", "b", "k", "e"];
    let normal: Vec<Frame> = motions
        .iter()
        .cycle()
        .take(KEYS)
        .map(|key| press(&window, &log, key))
        .collect();
    key_rows("en Normal (`j w b k e`)", &normal);

    press(&window, &log, "gg");
    let scroll: Vec<Frame> = (0..KEYS).map(|_| press(&window, &log, "<C-d>")).collect();
    key_rows("`ctrl-d`", &scroll);

    burst(&window, &log);
    quit(metalmind, window);
}

/// Teclea `keys` y espera el frame que muestra la última; con una sola
/// tecla, su `input` es lo que tardó.
fn press(window: &Window<'_>, log: &Path, keys: &str) -> Frame {
    let total = frames::keys_shown(log) + support::keys::parse(keys).len();
    window.type_keys(keys);
    frames::wait_for_keys(log, total, FRAME_TIMEOUT)
}

fn key_rows(what: &str, samples: &[Frame]) {
    let input = samples.iter().filter_map(|frame| frame.input);
    row(
        &format!("Tecla {what}, 50 k líneas: de la tecla al frame"),
        &Summary::of_micros(input.collect::<Vec<_>>()),
        samples.len(),
    );
    let cpu = samples.iter().filter_map(|frame| frame.cpu);
    row(
        &format!("Tecla {what}, 50 k líneas: CPU del frame (render → paint)"),
        &Summary::of_micros(cpu.collect::<Vec<_>>()),
        samples.len(),
    );
}

/// `ctrl-d` sin esperar cada frame, como una tecla sostenida: cuánto tarda
/// en mostrarse la última y cuántos frames se pintan en el camino.
fn burst(window: &Window<'_>, log: &Path) {
    press(window, log, "gg");
    let seen = frames::read(log).len();
    let last = press(window, log, &"<C-d>".repeat(BURST));
    let frames = frames::read(log);
    let end = frames
        .iter()
        .position(|frame| *frame == last)
        .expect("el frame de la última tecla");
    let shown = &frames[seen..=end];
    let first = shown[0];
    let start = first.t - first.input.expect("el primer frame muestra una tecla");
    let per_key = (last.t - start) / u64::try_from(BURST).expect("cabe");
    println!(
        "| Ráfaga de {BURST} `ctrl-d`, 50 k líneas: {} frames en {:.0} ms ({per_key} µs por tecla) | | | | 1 |",
        shown.len(),
        frames::millis(last.t - start),
    );
    let cpu = shown.iter().filter_map(|frame| frame.cpu);
    row(
        "Ráfaga de `ctrl-d`: CPU por frame (render → paint)",
        &Summary::of_micros(cpu.collect::<Vec<_>>()),
        shown.len(),
    );
}

fn row(what: &str, summary: &Summary, samples: usize) {
    println!(
        "| {what} | {:.2} ms | {:.2} ms | {:.2} ms | {samples} |",
        summary.p50, summary.p95, summary.max
    );
}

fn micros(duration: Duration) -> u64 {
    u64::try_from(duration.as_micros()).unwrap_or(u64::MAX)
}
