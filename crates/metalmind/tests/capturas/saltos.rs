//! Metalmind abre con la jumplist y las marks `A`–`Z` de una sesión
//! anterior. Reponerlas no mueve el cursor ni el scroll (`saltos-al-abrir`
//! se ve igual que `editor`); `ctrl-o` baja por la nota larga y `'B` lleva a
//! la nota de la ruta más honda.

use std::fmt::Write as _;
use std::fs;

use metalmind_config::Paths;
use rstest::rstest;

use super::support::Smoke;
use super::{compare, finish, frame_count, out_dir, prepare, settle, vault, wait_first_paint};

/// Cada pantalla parte de cómo dejó la ventana la anterior.
const SCENES: &[(&str, &str)] = &[
    ("saltos-al-abrir", ""),
    ("saltos-ctrl-o", "<C-o>"),
    ("saltos-mark-b", "'B"),
];

#[rstest]
#[case::oscuro_1920("oscuro", 1920, 1080)]
#[case::oscuro_1280("oscuro", 1280, 720)]
#[case::claro_1920("claro", 1920, 1080)]
#[case::claro_1280("claro", 1280, 720)]
#[ignore = "necesita Xvfb y Vulkan: ver docs/testing.md"]
fn saltos(#[case] theme: &str, #[case] width: u16, #[case] height: u16) {
    let out = out_dir();
    let smoke = Smoke::with_screen_at("1920x1080", &format!("metalmind-saltos-{theme}-{width}"));
    prepare(&smoke, theme);
    write_saved_jumps(&smoke);
    let logs = tempfile::tempdir().unwrap();
    let frames = logs.path().join("frames.log");
    let mut metalmind = smoke.launch(&["--frame-log", frames.to_str().unwrap(), vault::NOTE]);
    let window = metalmind.window();
    let rest = (
        i16::try_from(width).unwrap() - 2,
        i16::try_from(height).unwrap() - 2,
    );
    let mut seen = frame_count(&frames);
    window.resize(width.into(), height.into());
    wait_first_paint(&window);
    let mut report = Vec::new();
    let mut titles = String::new();
    for (scene, keys) in SCENES {
        window.type_keys(keys);
        window.move_pointer(rest.0, rest.1);
        let image = settle(&window, &frames, seen);
        seen = frame_count(&frames);
        assert_eq!((image.width, image.height), (width, height), "{scene}");
        let name = format!("{scene}-{theme}-{width}");
        image.save(&out.join(format!("{name}.png")));
        report.push(compare(&out, &name, &image));
        writeln!(titles, "{name}: {}", window.title()).unwrap();
    }
    drop(metalmind);
    fs::write(
        out.join(format!("titulos-saltos-{theme}-{width}.txt")),
        titles,
    )
    .unwrap();
    finish(&out, &format!("saltos-{theme}-{width}"), &report);
}

/// La jumplist pasa por tres notas y termina hacia el final de la nota
/// larga, lejos de lo que se ve al abrir; las marks están en otras notas.
/// Nada con fórmulas: se componen después y la captura no se repetiría.
fn write_saved_jumps(smoke: &Smoke) {
    let paths = Paths::from_xdg(|_| None, &smoke.home());
    let state = paths.vault_state(&smoke.notes().canonicalize().unwrap());
    let deep = format!("{}/{}", vault::DEEP_FOLDER_2, vault::DEEP_NOTE);
    let saved = format!(
        "metalmind-saltos 1\n\
         salto 0 0 otra.md\n\
         salto 0 0 {deep}\n\
         salto 1180 10 {note}\n\
         mark A 0 0 otra.md\n\
         mark B 0 4 {deep}\n",
        note = vault::NOTE,
    );
    fs::write(state.join("saltos"), saved).unwrap();
}
