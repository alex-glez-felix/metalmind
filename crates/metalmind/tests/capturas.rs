#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "en tests, un unwrap que falla es un test que falla"
)]

//! Capturas de cada pantalla en un Xvfb propio, en los dos temas y en
//! ventanas de 1920x1080 y 1280x720, sobre un vault que estresa el layout
//! (`capturas/vault.rs`). Con `METALMIND_CAPTURAS_ANTES` compara cada una con
//! la del mismo nombre en esa carpeta y falla si cambia algún píxel.
//!
//!   scripts/capturas.sh [salida] [--antes carpeta]
//!
//! Cómo y cuándo usarlas: `docs/design.md`, "Proceso".

#[path = "capturas/image.rs"]
mod image;
#[allow(dead_code, reason = "las capturas usan solo una parte del harness")]
mod support;
#[path = "capturas/vault.rs"]
mod vault;

use std::env;
use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use image::Image;
use metalmind_config::{Paths, Themes};
use rstest::rstest;
use support::wait::{self, TIMEOUT};
use support::{Smoke, Window};

/// Sin frames nuevos durante este tiempo, la ventana ya pintó lo último.
const QUIET: Duration = Duration::from_millis(400);

/// Donde caen las capturas; por defecto, en `target/tmp/capturas`.
const OUT_VAR: &str = "METALMIND_CAPTURAS";
/// Una carpeta con capturas de antes para comparar.
const BEFORE_VAR: &str = "METALMIND_CAPTURAS_ANTES";

/// Lo que se hace antes de capturar una pantalla.
enum Step {
    Keys(&'static str),
    /// Clic derecho en la fila `n` del árbol.
    RightClickRow(i16),
    /// Clic derecho en la última fila del árbol, cerca de su borde derecho:
    /// el menú no cabe hacia abajo.
    RightClickBottomRow,
    /// Escribe [`vault::SEARCHED`].
    Search,
    /// Deja el ratón sobre una mark hasta que sale su tooltip.
    HoverMark(MarkAt),
}

/// Sobre qué mark se deja el ratón.
#[derive(Clone, Copy)]
enum MarkAt {
    /// En el panel de la izquierda, la línea `n` desde arriba, sin headings
    /// antes.
    Line(i16),
    /// La más baja del panel de la izquierda: el tooltip junto al borde de
    /// abajo.
    Lowest,
    /// La primera línea del último de cuatro paneles lado a lado: el tooltip
    /// junto al borde derecho.
    RightmostPane,
}

/// Cada pantalla parte de cómo dejó la ventana la anterior.
const SCENES: &[(&str, &[Step])] = &[
    ("editor", &[]),
    ("editor-final", &[Step::Keys("G")]),
    ("arbol", &[Step::Keys("gg<Space>ejjj")]),
    ("menu", &[Step::RightClickRow(2)]),
    (
        "menu-borde",
        &[Step::Keys("<Esc>"), Step::RightClickBottomRow],
    ),
    ("split", &[Step::Keys("<Esc><C-w>l<C-w>v")]),
    (
        "linea-de-comandos",
        &[Step::Keys(
            "<C-w>o:una linea de comandos larga, que sigue mas alla de la mitad de la barra de estado y no termina",
        )],
    ),
    ("finder", &[Step::Keys("<Esc><Space>f")]),
    ("finder-filtrado", &[Step::Keys("idea")]),
    ("finder-ruta-larga", &[Step::Keys("<C-u>honda")]),
    ("finder-vacio", &[Step::Keys("<C-u>zzqqxx")]),
    ("buscar", &[Step::Keys("<Esc><Space>/"), Step::Search]),
    ("buscar-vacio", &[Step::Keys("<C-u>zzqqxx")]),
    ("paleta", &[Step::Keys("<Esc><C-S-p>")]),
    // Al final, para que las marks no cambien las demás pantallas: tres en
    // una línea (la `b` y un `+`), una sobre un heading y el tooltip.
    (
        "marks",
        &[
            Step::Keys("<Esc>ggmajmbmcmBjjmA"),
            Step::HoverMark(MarkAt::Line(1)),
        ],
    ),
    // La nota larga: marks en líneas de cuatro cifras, la última con once
    // (`a`–`h` y `A`–`C`: un tooltip largo) y, con `zb`, el tooltip junto al
    // borde de abajo. La última línea es más larga que la ventana.
    (
        "marks-nota-larga",
        &[
            Step::Keys("GmambmcmdmemfmgmhmAmBmCkkkmzkkkkkmyGzb"),
            Step::HoverMark(MarkAt::Lowest),
        ],
    ),
    (
        "marks-borde-derecho",
        &[
            Step::Keys("<C-w>v<C-w>v<C-w>vGzt"),
            Step::HoverMark(MarkAt::RightmostPane),
        ],
    ),
];

/// Alto de una fila del árbol y del encabezado, y dónde empieza el panel,
/// con la fuente por defecto: para hacer clic sobre una fila.
const TREE_ROW: i16 = 26;
const TREE_LEFT: i16 = 36;
const TREE_WIDTH: i16 = 260;
/// Con `editor.padding` por defecto (12 a los lados, 8 arriba) y líneas de
/// texto de 21: la columna de las marks y el centro de cada línea sin heading.
const EDITOR_LEFT: i16 = TREE_LEFT + TREE_WIDTH;
const MARK_X: i16 = EDITOR_LEFT + 12 + 4;
const TEXT_TOP: i16 = 8;
const TEXT_LINE: i16 = 21;

#[rstest]
#[case::oscuro_1920("oscuro", 1920, 1080)]
#[case::oscuro_1280("oscuro", 1280, 720)]
#[case::claro_1920("claro", 1920, 1080)]
#[case::claro_1280("claro", 1280, 720)]
#[ignore = "necesita Xvfb y Vulkan: ver docs/testing.md"]
fn capturas(#[case] theme: &str, #[case] width: u16, #[case] height: u16) {
    let out = out_dir();
    let smoke = Smoke::with_screen("1920x1080");
    prepare(&smoke, theme);
    let logs = tempfile::tempdir().unwrap();
    let frames = logs.path().join("frames.log");
    let frame_log = frames.to_str().unwrap();
    let mut metalmind = smoke.launch(&["--frame-log", frame_log, vault::NOTE]);
    let window = metalmind.window();
    let rest = (
        i16::try_from(width).unwrap() - 2,
        i16::try_from(height).unwrap() - 2,
    );
    let mut seen = frame_count(&frames);
    let mark_color = gutter_mark_color(theme);
    window.resize(width.into(), height.into());
    let mut report = Vec::new();
    for (scene, steps) in SCENES {
        for step in *steps {
            match step {
                Step::Keys(keys) => window.type_keys(keys),
                Step::Search => window.type_keys(vault::SEARCHED),
                Step::RightClickRow(row) => {
                    window.right_click(TREE_LEFT + 80, TREE_ROW * (row + 1) + TREE_ROW / 2);
                }
                Step::RightClickBottomRow => {
                    window.right_click(TREE_LEFT + TREE_WIDTH - 8, rest.1 - TREE_ROW / 2);
                }
                Step::HoverMark(at) => {
                    // El tooltip sale tras una demora con el ratón quieto, más
                    // larga que la quietud de `settle`: se espera a verlo.
                    let still = settle(&window, &frames, seen);
                    seen = frame_count(&frames);
                    let (x, y) = mark_position(*at, &still, mark_color);
                    window.move_pointer(x, y);
                    wait::until("el tooltip de la mark", TIMEOUT, || {
                        if window.pixels().2 == still.rgb {
                            Err("sin tooltip".to_owned())
                        } else {
                            Ok(())
                        }
                    });
                }
            }
        }
        if !matches!(
            steps.last(),
            Some(Step::RightClickRow(_) | Step::RightClickBottomRow | Step::HoverMark(_))
        ) {
            window.move_pointer(rest.0, rest.1);
        }
        let image = settle(&window, &frames, seen);
        seen = frame_count(&frames);
        assert_eq!((image.width, image.height), (width, height), "{scene}");
        let name = format!("{scene}-{theme}-{width}");
        image.save(&out.join(format!("{name}.png")));
        report.push(compare(&out, &name, &image));
    }
    drop(metalmind);
    finish(&out, &format!("{theme}-{width}"), &report);
}

/// Dónde dejar el ratón para ver el tooltip de una mark en `image`.
fn mark_position(at: MarkAt, image: &Image, mark_color: [u8; 3]) -> (i16, i16) {
    match at {
        MarkAt::Line(line) => (MARK_X, TEXT_TOP + TEXT_LINE * line + TEXT_LINE / 2),
        MarkAt::Lowest => (MARK_X, lowest_mark(image, MARK_X, mark_color)),
        MarkAt::RightmostPane => {
            let editors = i16::try_from(image.width).unwrap() - EDITOR_LEFT;
            let pane_left = EDITOR_LEFT + editors * 3 / 4;
            (pane_left + MARK_X - EDITOR_LEFT, TEXT_TOP + TEXT_LINE / 2)
        }
    }
}

/// La fila más baja con la letra de una mark en la columna de `x`: se busca
/// su color en la imagen, porque dónde termina el texto depende de la ventana.
fn lowest_mark(image: &Image, x: i16, mark_color: [u8; 3]) -> i16 {
    let width = usize::from(image.width);
    let columns = usize::try_from(x - 4).unwrap()..usize::try_from(x + 6).unwrap();
    (0..usize::from(image.height))
        .rev()
        .find(|&row| {
            columns.clone().any(|column| {
                let at = (row * width + column) * 3;
                image.rgb[at..at + 3] == mark_color
            })
        })
        .map(|row| i16::try_from(row).unwrap() - 4)
        .expect("ninguna mark en la columna")
}

/// El color de `ui.gutter_mark` en el tema de fábrica `theme`.
fn gutter_mark_color(theme: &str) -> [u8; 3] {
    let dir = tempfile::tempdir().unwrap();
    let loaded = Themes::new(dir.path().to_owned())
        .load(&format!("metalmind-{theme}"))
        .unwrap();
    let [_, red, green, blue] = loaded.value.ui.gutter_mark.rgb.to_be_bytes();
    [red, green, blue]
}

fn out_dir() -> PathBuf {
    let out = env::var_os(OUT_VAR).map_or_else(
        || Path::new(env!("CARGO_TARGET_TMPDIR")).join("capturas"),
        PathBuf::from,
    );
    fs::create_dir_all(&out).unwrap();
    out
}

/// El vault, el tema y las carpetas del árbol abiertas.
fn prepare(smoke: &Smoke, theme: &str) {
    vault::write(&smoke.notes());
    let home = smoke.home();
    let config = home.join(".config/metalmind");
    fs::create_dir_all(&config).unwrap();
    fs::write(
        config.join("config.yaml"),
        format!("theme: metalmind-{theme}\n"),
    )
    .unwrap();
    let paths = Paths::from_xdg(|_| None, &home);
    let state = paths.vault_state(&smoke.notes().canonicalize().unwrap());
    fs::create_dir_all(&state).unwrap();
    let mut expanded = String::new();
    for folder in vault::EXPANDED {
        writeln!(expanded, "    - {folder}").unwrap();
    }
    fs::write(
        state.join("ui.yaml"),
        format!("panel:\n  visible: true\n  view: folders\n  expanded:\n{expanded}"),
    )
    .unwrap();
}

fn frame_count(frames: &Path) -> usize {
    fs::read_to_string(frames).map_or(0, |log| log.lines().count())
}

/// Espera a que la ventana pinte algo después de `before` frames y deje de
/// pintar, y devuelve lo que se ve.
fn settle(window: &Window<'_>, frames: &Path, before: usize) -> Image {
    wait::until("un frame nuevo", TIMEOUT, || {
        let count = frame_count(frames);
        if count > before {
            Ok(())
        } else {
            Err(format!("{count} frames"))
        }
    });
    let mut last = (frame_count(frames), Instant::now());
    wait::until("que la ventana deje de pintar", TIMEOUT, || {
        let count = frame_count(frames);
        if count != last.0 {
            last = (count, Instant::now());
        }
        if last.1.elapsed() >= QUIET {
            Ok(())
        } else {
            Err(format!("{count} frames"))
        }
    });
    let (width, height, rgb) = window.pixels();
    Image { width, height, rgb }
}

/// Una línea del resumen, y la imagen de lo que cambió si cambió algo.
fn compare(out: &Path, name: &str, image: &Image) -> (String, usize) {
    let Some(before) = env::var_os(BEFORE_VAR).map(PathBuf::from) else {
        return (format!("| {name} | sin comparar |"), 0);
    };
    let path = before.join(format!("{name}.png"));
    if !path.exists() {
        // Sin la de antes no hay comparación: cuenta como si cambiara todo.
        let pixels = usize::from(image.width) * usize::from(image.height);
        return (format!("| {name} | no hay captura de antes |"), pixels);
    }
    match Image::load(&path).diff(image) {
        None => (format!("| {name} | 0 |"), 0),
        Some((changed, marked)) => {
            marked.save(&out.join(format!("diff-{name}.png")));
            (
                format!("| {name} | {changed} (`diff-{name}.png`) |"),
                changed,
            )
        }
    }
}

/// Escribe las filas del resumen del caso (el script les pone el
/// encabezado) y falla si alguna captura cambió.
fn finish(out: &Path, case: &str, report: &[(String, usize)]) {
    let mut summary = String::new();
    for (line, _) in report {
        writeln!(summary, "{line}").unwrap();
    }
    fs::write(out.join(format!("resumen-{case}.md")), &summary).unwrap();
    let changed: Vec<&str> = report
        .iter()
        .filter(|(_, changed)| *changed > 0)
        .map(|(line, _)| line.as_str())
        .collect();
    assert!(
        changed.is_empty(),
        "cambiaron píxeles:\n{}",
        changed.join("\n")
    );
}
