#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "en tests, un unwrap que falla es un test que falla"
)]

//! Las versiones del autoguardado en el historial (06 §3): una por grupo de
//! [`AUTOSAVE_GROUP`] mientras se edita, la última al terminar, y un
//! guardado pedido siempre deja la suya. El reloj es el `at` de cada
//! guardado.

mod support;

use std::fmt::Write as _;
use std::fs;
use std::time::{Duration, SystemTime};

use metalmind_vault::{AUTOSAVE_GROUP, History, VaultPath, VersionKind};
use pretty_assertions::assert_eq;
use support::history::{RETENTION, contents, epoch, path};
use tempfile::TempDir;

const SECOND: Duration = Duration::from_secs(1);

fn history(dir: &TempDir) -> History {
    History::new(dir.path(), RETENTION)
}

fn kinds(history: &History, note: &VaultPath) -> Vec<VersionKind> {
    let versions = history.versions(note).unwrap();
    versions.iter().map(|version| version.kind).collect()
}

fn at(seconds: u32) -> SystemTime {
    epoch() + SECOND * seconds
}

#[test]
fn una_version_por_grupo_y_la_ultima_al_terminar() {
    let dir = TempDir::new().unwrap();
    let history = history(&dir);
    let note = path("a.md");
    // Doce minutos tecleando, con un autoguardado por segundo.
    for second in 0..=720 {
        let text = format!("{second}\n");
        history.record_autosave(&note, &text, at(second)).unwrap();
    }
    let group = u32::try_from(AUTOSAVE_GROUP.as_secs()).unwrap();
    assert_eq!(
        contents(&history, &note),
        [
            format!("{}\n", group - 1),
            format!("{}\n", 2 * group - 1),
            "720\n".to_owned()
        ]
    );
    assert_eq!(kinds(&history, &note), [VersionKind::Autosaved; 3]);
    let saved_at: Vec<SystemTime> = history
        .versions(&note)
        .unwrap()
        .iter()
        .map(|version| version.saved_at)
        .collect();
    assert_eq!(saved_at, [at(group - 1), at(2 * group - 1), at(720)]);
}

#[test]
fn un_guardado_pedido_siempre_deja_su_version() {
    let dir = TempDir::new().unwrap();
    let history = history(&dir);
    let note = path("a.md");
    history.record_autosave(&note, "a", at(0)).unwrap();
    history.record_autosave(&note, "ab", at(1)).unwrap();
    assert!(history.record(&note, "abc", at(2)).unwrap().is_some());
    history.record_autosave(&note, "abcd", at(3)).unwrap();
    assert_eq!(contents(&history, &note), ["ab", "abc", "abcd"]);

    // `:w` de lo que ya se autoguardó: la versión queda fija.
    assert!(history.record(&note, "abcd", at(4)).unwrap().is_some());
    history.record_autosave(&note, "abcde", at(5)).unwrap();
    assert_eq!(contents(&history, &note), ["ab", "abc", "abcd", "abcde"]);
    assert_eq!(
        kinds(&history, &note),
        [
            VersionKind::Autosaved,
            VersionKind::Saved,
            VersionKind::Saved,
            VersionKind::Autosaved
        ]
    );
    // Autoguardar lo mismo otra vez no es nada.
    assert_eq!(
        history.record_autosave(&note, "abcde", at(6)).unwrap(),
        None
    );
}

#[test]
fn el_grupo_se_lee_igual_tras_cerrar() {
    let dir = TempDir::new().unwrap();
    let note = path("a.md");
    history(&dir).record_autosave(&note, "a", at(0)).unwrap();
    // Otro `History` sobre la misma carpeta, como al volver a abrir la app.
    let reopened = history(&dir);
    reopened.record_autosave(&note, "ab", at(60)).unwrap();
    assert_eq!(contents(&reopened, &note), ["ab"]);
}

#[test]
fn podar_quita_lo_que_ocupaban_las_reemplazadas() {
    let dir = TempDir::new().unwrap();
    let history = history(&dir);
    let note = path("a.md");
    let mut text = String::new();
    // Menos de las que hacen compactar el log al registrar.
    for second in 0..60 {
        writeln!(text, "línea {second} de la nota").unwrap();
        history.record_autosave(&note, &text, at(second)).unwrap();
    }
    let before = logs_size(&dir);
    let pruned = history.prune(at(200)).unwrap();
    assert_eq!(pruned.versions, 0, "ninguna versión se pierde");
    assert_eq!(contents(&history, &note), [text]);
    let after = logs_size(&dir);
    assert!(after * 4 < before, "{after} de {before} bytes");
}

#[test]
fn una_sesion_larga_no_hace_crecer_el_log() {
    let dir = TempDir::new().unwrap();
    let history = history(&dir);
    let note = path("a.md");
    let autosave = |tenths: u32| {
        let text = format!("{tenths:06}\n");
        let at = epoch() + Duration::from_millis(u64::from(tenths) * 100);
        history.record_autosave(&note, &text, at).unwrap();
    };
    autosave(0);
    let first = logs_size(&dir);
    for tenths in 1..=10 {
        autosave(tenths);
    }
    let each = (logs_size(&dir) - first) / 10;
    // Mil autoguardadas en cien segundos: un solo grupo, una sola versión.
    for tenths in 11..1000 {
        autosave(tenths);
    }
    assert_eq!(contents(&history, &note), ["000999\n"]);
    let size = logs_size(&dir);
    assert!(
        size < first + each * 100,
        "{size} bytes, {each} por autoguardado"
    );
}

fn logs_size(dir: &TempDir) -> u64 {
    fs::read_dir(dir.path().join("notes"))
        .unwrap()
        .map(|entry| entry.unwrap().metadata().unwrap().len())
        .sum()
}
