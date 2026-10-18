#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "en tests, un unwrap que falla es un test que falla"
)]

mod support;

use std::fs::{self, OpenOptions};
use std::io::Write as _;
use std::time::Duration;

use nota_config::HistoryRetention;
use nota_vault::{ChunkKind, DeletedNote, History, Pruned, VaultPath, Version};
use pretty_assertions::assert_eq;
use proptest::prelude::*;
use support::history::{DAY, HOUR, contents, epoch, path};
use tempfile::TempDir;

fn history(dir: &TempDir) -> History {
    History::new(dir.path(), HistoryRetention::default())
}

/// Una edición chica sobre el texto anterior, como un guardado de verdad.
#[derive(Debug, Clone)]
enum Edit {
    Insert(usize, String),
    Delete(usize),
    Replace(String),
}

fn edit_strategy() -> impl Strategy<Value = Edit> {
    let line = "[a-zñ #\\[\\]]{0,12}(\n|\r\n)?";
    prop_oneof![
        4 => (any::<usize>(), line).prop_map(|(at, text)| Edit::Insert(at, text)),
        2 => any::<usize>().prop_map(Edit::Delete),
        1 => "[a-z\n]{0,40}".prop_map(Edit::Replace),
    ]
}

fn apply(text: &str, edit: &Edit) -> String {
    let mut lines: Vec<&str> = text.split_inclusive('\n').collect();
    match edit {
        Edit::Insert(at, new) => lines.insert(at % (lines.len() + 1), new),
        Edit::Delete(at) if !lines.is_empty() => {
            lines.remove(at % lines.len());
        }
        Edit::Delete(_) => {}
        Edit::Replace(new) => return new.clone(),
    }
    lines.concat()
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]

    #[test]
    fn cada_version_vuelve_exacta(edits in prop::collection::vec(edit_strategy(), 1..40)) {
        let dir = TempDir::new().unwrap();
        let history = history(&dir);
        let note = path("nota.md");
        let mut text = String::new();
        let mut expected: Vec<String> = Vec::new();
        for (minute, edit) in (0..).zip(&edits) {
            text = apply(&text, edit);
            let at = epoch() + Duration::from_secs(60 * minute);
            let recorded = history.record(&note, &text, at).unwrap();
            let same_as_last = expected.last() == Some(&text);
            prop_assert_eq!(recorded.is_none(), same_as_last);
            if !same_as_last {
                expected.push(text.clone());
            }
        }
        prop_assert_eq!(contents(&history, &note), expected);
    }
}

#[test]
fn guardar_lo_mismo_no_es_otra_version() {
    let dir = TempDir::new().unwrap();
    let history = history(&dir);
    let note = path("a.md");
    let first = history.record(&note, "uno", epoch()).unwrap();
    assert_eq!(history.record(&note, "uno", epoch() + HOUR).unwrap(), None);
    let second = history.record(&note, "dos", epoch() + HOUR).unwrap();
    let versions = history.versions(&note).unwrap();
    assert_eq!(
        versions
            .iter()
            .map(|version| Some(version.id))
            .collect::<Vec<_>>(),
        [first, second]
    );
    assert_eq!(versions[1].saved_at, epoch() + HOUR);
    assert_eq!(versions[1].len, 3);
}

#[test]
fn diff_y_plan_de_restaurar() {
    let dir = TempDir::new().unwrap();
    let history = history(&dir);
    let note = path("a.md");
    let old = history
        .record(&note, "uno\ndos\ntres\n", epoch())
        .unwrap()
        .unwrap();
    let new = history
        .record(&note, "uno\n2\ntres\ncuatro\n", epoch() + HOUR)
        .unwrap()
        .unwrap();

    let diff = history.diff(&note, old, new).unwrap();
    let kinds: Vec<ChunkKind> = diff.chunks.iter().map(|chunk| chunk.kind).collect();
    assert_eq!(
        kinds,
        [
            ChunkKind::Equal,
            ChunkKind::Replace,
            ChunkKind::Equal,
            ChunkKind::Insert
        ]
    );

    let current = "uno\n2\ntres\ncuatro\ncinco\n";
    let plan = history.restore_plan(&note, old, current).unwrap();
    assert_eq!(plan.apply(current), "uno\ndos\ntres\n");
    assert!(
        history
            .content(&note, new)
            .is_ok_and(|text| text.ends_with("cuatro\n"))
    );
}

#[test]
fn una_version_que_no_existe_es_un_error() {
    let dir = TempDir::new().unwrap();
    let history = history(&dir);
    let note = path("a.md");
    let id = history.record(&note, "uno", epoch()).unwrap().unwrap();
    assert!(history.content(&path("otra.md"), id).is_err());
    assert_eq!(
        history.versions(&path("otra.md")).unwrap(),
        Vec::<Version>::new()
    );
}

#[test]
fn un_renombre_en_medio_sigue_la_historia() {
    let dir = TempDir::new().unwrap();
    let history = history(&dir);
    let (a, b) = (path("a.md"), path("carpeta/b.md"));
    history.record(&a, "uno", epoch()).unwrap();
    history.record(&a, "dos", epoch() + HOUR).unwrap();
    history.rename(&a, &b).unwrap();
    history.record(&b, "tres", epoch() + 2 * HOUR).unwrap();
    assert_eq!(contents(&history, &b), ["uno", "dos", "tres"]);
    assert_eq!(history.versions(&a).unwrap(), Vec::<Version>::new());

    // Mover la carpeta mueve la historia de lo que tiene dentro.
    history.rename(&path("carpeta"), &path("otra")).unwrap();
    assert_eq!(
        contents(&history, &path("otra/b.md")),
        ["uno", "dos", "tres"]
    );
}

#[test]
fn mover_una_carpeta_no_toca_las_que_empiezan_igual() {
    let dir = TempDir::new().unwrap();
    let history = history(&dir);
    for note in ["a/x.md", "a-b/x.md", "a.md", "ab/x.md"] {
        history.record(&path(note), note, epoch()).unwrap();
    }
    history.rename(&path("a"), &path("z")).unwrap();
    for (note, content) in [
        ("z/x.md", "a/x.md"),
        ("a-b/x.md", "a-b/x.md"),
        ("a.md", "a.md"),
        ("ab/x.md", "ab/x.md"),
    ] {
        assert_eq!(contents(&history, &path(note)), [content], "{note}");
    }
    assert_eq!(
        history.versions(&path("a/x.md")).unwrap(),
        Vec::<Version>::new()
    );
}

#[test]
fn renombrar_encima_de_otra_historia_las_junta_por_fecha() {
    let dir = TempDir::new().unwrap();
    let history = history(&dir);
    let (a, b) = (path("a.md"), path("b.md"));
    history.record(&b, "b vieja", epoch()).unwrap();
    history.record(&a, "a uno", epoch() + HOUR).unwrap();
    history.record(&b, "b nueva", epoch() + 2 * HOUR).unwrap();
    history.record(&a, "a dos", epoch() + 3 * HOUR).unwrap();
    history.rename(&a, &b).unwrap();
    assert_eq!(
        contents(&history, &b),
        ["b vieja", "a uno", "b nueva", "a dos"]
    );
    assert_eq!(history.versions(&a).unwrap(), Vec::<Version>::new());
}

#[test]
fn la_retencion_poda_por_edad_y_deja_el_contenido_exacto() {
    let dir = TempDir::new().unwrap();
    let history = history(&dir);
    let note = path("a.md");
    let now = epoch() + 100 * DAY;
    // Hace 40 días, dos en el mismo día; hace 10, dos en la misma hora; hoy, dos.
    let saves = [
        (now - 40 * DAY - 2 * HOUR, "día viejo, primera"),
        (now - 40 * DAY - HOUR, "día viejo, última"),
        (
            now - 10 * DAY - 2 * Duration::from_mins(10),
            "hora vieja, primera",
        ),
        (
            now - 10 * DAY - Duration::from_mins(10),
            "hora vieja, última",
        ),
        (now - 2 * HOUR, "hoy, primera"),
        (now - HOUR, "hoy, última"),
    ];
    for (at, text) in saves {
        history.record(&note, text, at).unwrap();
    }
    let before = history.versions(&note).unwrap();

    let pruned = history.prune(now).unwrap();
    assert_eq!(
        pruned,
        Pruned {
            versions: 2,
            notes: 0
        }
    );
    assert_eq!(
        contents(&history, &note),
        [
            "día viejo, última",
            "hora vieja, última",
            "hoy, primera",
            "hoy, última"
        ]
    );
    let after = history.versions(&note).unwrap();
    assert_eq!(
        after[1..],
        before[3..],
        "lo que queda conserva número y fecha"
    );
    assert_eq!(history.prune(now).unwrap(), Pruned::default());

    // Y se sigue guardando sobre lo podado.
    history.record(&note, "después", now).unwrap();
    assert_eq!(contents(&history, &note).last().unwrap(), "después");
}

#[test]
fn la_historia_de_una_nota_borrada_dura_lo_que_dice_la_retencion() {
    let dir = TempDir::new().unwrap();
    let history = history(&dir);
    let note = path("a.md");
    history.record(&note, "uno", epoch()).unwrap();
    history.mark_deleted(&note, epoch() + DAY).unwrap();
    assert_eq!(
        history.deleted_notes().unwrap(),
        [DeletedNote {
            path: note.clone(),
            deleted_at: epoch() + DAY
        }]
    );
    assert_eq!(contents(&history, &note), ["uno"], "se puede recuperar");

    assert_eq!(
        history.prune(epoch() + 90 * DAY).unwrap(),
        Pruned::default()
    );
    assert_eq!(
        history.prune(epoch() + 92 * DAY).unwrap(),
        Pruned {
            versions: 0,
            notes: 1
        }
    );
    assert_eq!(history.versions(&note).unwrap(), Vec::<Version>::new());
    assert_eq!(history.deleted_notes().unwrap(), Vec::<DeletedNote>::new());
}

#[test]
fn volver_a_guardar_una_nota_borrada_sigue_su_historia() {
    let dir = TempDir::new().unwrap();
    let history = history(&dir);
    let note = path("a.md");
    history.record(&note, "uno", epoch()).unwrap();
    history.mark_deleted(&note, epoch() + HOUR).unwrap();
    history.record(&note, "dos", epoch() + 2 * HOUR).unwrap();
    assert_eq!(history.deleted_notes().unwrap(), Vec::<DeletedNote>::new());
    assert_eq!(contents(&history, &note), ["uno", "dos"]);
}

#[test]
fn un_guardado_cortado_no_pierde_lo_anterior() {
    let dir = TempDir::new().unwrap();
    let history = history(&dir);
    let note = path("a.md");
    history.record(&note, "uno", epoch()).unwrap();
    let log = fs::read_dir(dir.path().join("notes"))
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    OpenOptions::new()
        .append(true)
        .open(&log)
        .unwrap()
        .write_all(&[0x40, 1, 2])
        .unwrap();
    assert_eq!(contents(&history, &note), ["uno"]);
    history.record(&note, "dos", epoch() + HOUR).unwrap();
    assert_eq!(contents(&history, &note), ["uno", "dos"]);
}

#[test]
fn una_nota_de_5k_lineas_ocupa_poco_por_version() {
    let dir = TempDir::new().unwrap();
    let history = history(&dir);
    let note = path("larga.md");
    let lines: Vec<String> = (0..5_000)
        .map(|n| format!("- punto {n}: algo que revisar en [[nota {}]]\n", n % 97))
        .collect();
    let mut text = lines.concat();
    for n in 0..200 {
        let line = format!("- punto {}:", (n * 37) % 5_000);
        text = text.replacen(&line, &format!("{line} editado {n}"), 1);
        history
            .record(&note, &text, epoch() + Duration::from_secs(n))
            .unwrap();
    }
    let on_disk: u64 = fs::read_dir(dir.path().join("notes"))
        .unwrap()
        .map(|entry| entry.unwrap().metadata().unwrap().len())
        .sum();
    // Una sola copia comprimida de la nota más unos cientos de bytes por guardado.
    assert!(
        on_disk < text.len() as u64 / 2,
        "{on_disk} bytes para 200 versiones de {}",
        text.len()
    );
    assert_eq!(contents(&history, &note).last(), Some(&text));
}

#[test]
fn renombrar_una_nota_borrada_que_volvio_la_deja_viva() {
    let dir = TempDir::new().unwrap();
    let history = history(&dir);
    let (a, b) = (path("a.md"), path("b.md"));
    history.record(&a, "uno", epoch()).unwrap();
    history.mark_deleted(&a, epoch() + HOUR).unwrap();
    history.rename(&a, &b).unwrap();
    assert_eq!(history.deleted_notes().unwrap(), Vec::<DeletedNote>::new());
    assert_eq!(contents(&history, &b), ["uno"]);
}

#[test]
fn las_borradas_de_una_carpeta_movida_siguen_borradas() {
    let dir = TempDir::new().unwrap();
    let history = history(&dir);
    history.record(&path("a/x.md"), "uno", epoch()).unwrap();
    history
        .mark_deleted(&path("a/x.md"), epoch() + HOUR)
        .unwrap();
    history.rename(&path("a"), &path("b")).unwrap();
    let deleted: Vec<VaultPath> = history
        .deleted_notes()
        .unwrap()
        .into_iter()
        .map(|note| note.path)
        .collect();
    assert_eq!(deleted, [path("b/x.md")]);
}

#[test]
fn un_log_danado_no_frena_la_poda_de_las_demas() {
    let dir = TempDir::new().unwrap();
    let history = history(&dir);
    let (a, b) = (path("a.md"), path("b.md"));
    history.record(&a, "uno", epoch()).unwrap();
    history.record(&b, "uno", epoch()).unwrap();
    history.record(&b, "dos", epoch() + HOUR / 2).unwrap();
    let damaged = fs::read_dir(dir.path().join("notes"))
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .min()
        .unwrap();
    fs::write(damaged, "no es un historial").unwrap();
    assert_eq!(
        history.prune(epoch() + 2 * DAY).unwrap(),
        Pruned {
            versions: 1,
            notes: 0
        }
    );
    assert_eq!(contents(&history, &b), ["dos"]);
}

#[test]
fn leer_un_historial_que_no_existe_no_crea_nada() {
    let dir = TempDir::new().unwrap();
    let history = History::new(dir.path().join("historial"), HistoryRetention::default());
    assert_eq!(
        history.versions(&path("a.md")).unwrap(),
        Vec::<Version>::new()
    );
    assert_eq!(history.deleted_notes().unwrap(), Vec::<DeletedNote>::new());
    assert!(!dir.path().join("historial").exists());
}

#[test]
fn una_nota_nueva_no_hereda_un_log_que_quedo_fuera_del_indice() {
    let dir = TempDir::new().unwrap();
    let history = history(&dir);
    history.record(&path("a.md"), "de a", epoch()).unwrap();
    // Como si el guardado se hubiera cortado antes de escribir el índice.
    fs::remove_file(dir.path().join("index")).unwrap();
    history
        .record(&path("b.md"), "de b", epoch() + HOUR)
        .unwrap();
    assert_eq!(contents(&history, &path("b.md")), ["de b"]);
}

/// El log de cada nota, en el orden en que se crearon.
fn logs(dir: &TempDir) -> Vec<std::path::PathBuf> {
    let mut logs: Vec<_> = fs::read_dir(dir.path().join("notes"))
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "log"))
        .collect();
    logs.sort();
    logs
}

#[test]
fn una_nota_con_el_log_danado_sigue_guardando_versiones() {
    let dir = TempDir::new().unwrap();
    let history = history(&dir);
    let note = path("a.md");
    history.record(&note, "uno", epoch()).unwrap();
    let log = logs(&dir).remove(0);
    fs::write(&log, "no es un historial").unwrap();
    history.record(&note, "dos", epoch() + HOUR).unwrap();
    assert_eq!(contents(&history, &note), ["dos"]);
    let mut aside = log.into_os_string();
    aside.push(".damaged");
    assert_eq!(fs::read_to_string(aside).unwrap(), "no es un historial");
}

#[test]
fn renombrar_encima_de_una_historia_danada_conserva_la_sana() {
    let dir = TempDir::new().unwrap();
    let history = history(&dir);
    let (a, b, c) = (path("a.md"), path("b.md"), path("c.md"));
    history.record(&b, "b vieja", epoch()).unwrap();
    history.mark_deleted(&b, epoch() + HOUR).unwrap();
    history.record(&a, "a", epoch() + 2 * HOUR).unwrap();
    history.record(&c, "c", epoch() + 3 * HOUR).unwrap();
    fs::write(&logs(&dir)[0], "no es un historial").unwrap();
    history.rename(&a, &b).unwrap();
    assert_eq!(contents(&history, &b), ["a"]);
    // Las demás historias no se tocan.
    assert_eq!(contents(&history, &c), ["c"]);
}

#[test]
fn juntar_dos_historias_no_cambia_los_ids_de_sus_versiones() {
    let dir = TempDir::new().unwrap();
    let history = history(&dir);
    let (a, b) = (path("a.md"), path("b.md"));
    let old = history.record(&b, "b vieja", epoch()).unwrap().unwrap();
    history.mark_deleted(&b, epoch() + HOUR).unwrap();
    let moved = history
        .record(&a, "a", epoch() + 2 * HOUR)
        .unwrap()
        .unwrap();
    history.rename(&a, &b).unwrap();
    assert_eq!(history.content(&b, old).unwrap(), "b vieja");
    assert_eq!(history.content(&b, moved).unwrap(), "a");
}
