#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "en tests, un unwrap que falla es un test que falla"
)]

mod support;

use std::collections::HashSet;
use std::fs;
use std::path::Path;
use std::sync::mpsc::Receiver;
use std::time::SystemTime;

use metalmind_vault::{
    History, HistoryOptions, SaveKind, Vault, VaultChange, VaultOptions, VaultPath, VaultUpdate,
    Version, VersionError, VersionId, VersionKind,
};
use pretty_assertions::assert_eq;
use support::history::{HOUR, RETENTION, contents, epoch, path};
use support::wait_for_changes;
use tempfile::TempDir;

fn open_vault(root: &Path, history: &Path) -> (Vault, Receiver<VaultUpdate>) {
    let history = HistoryOptions {
        dir: history.to_owned(),
        retention: RETENTION,
    };
    let options = VaultOptions {
        history: Some(history),
        ..VaultOptions::default()
    };
    Vault::open_with(root, options).unwrap()
}

/// El hilo del vault alinea el historial con lo que pasó con el vault
/// cerrado antes de atender nada: un guardado vuelve después de eso.
fn wait_until_open(vault: &Vault) {
    save(vault, &path("abierto.md"), "abierto");
}

/// Guarda desde el vault y espera a que se registre la versión.
fn save(vault: &Vault, note: &VaultPath, text: &str) -> Option<VersionId> {
    vault
        .save(note, text.to_owned(), SaveKind::Explicit)
        .unwrap()
        .unwrap()
        .wait()
        .unwrap()
}

#[test]
fn guardar_desde_el_vault_registra_la_version() {
    let (dir, data) = (TempDir::new().unwrap(), TempDir::new().unwrap());
    let (vault, _updates) = open_vault(dir.path(), data.path());
    let note = path("sub/a.md");
    assert!(
        vault
            .save(&note, "uno\n".to_owned(), SaveKind::Explicit)
            .is_err(),
        "como el E212 de vim, no crea la carpeta"
    );
    fs::create_dir(dir.path().join("sub")).unwrap();
    assert!(save(&vault, &note, "uno\n").is_some());
    assert_eq!(save(&vault, &note, "uno\n"), None);
    save(&vault, &note, "dos\n");
    assert_eq!(
        fs::read_to_string(dir.path().join("sub/a.md")).unwrap(),
        "dos\n"
    );
    assert_eq!(
        contents(vault.history().unwrap(), &note),
        ["uno\n", "dos\n"]
    );
    assert!(
        vault
            .save(&path("imagen.png"), "x".to_owned(), SaveKind::Explicit)
            .is_err(),
        "solo notas"
    );
}

/// Lo que guarda el autoguardado y lo que el watcher ve de esos guardados
/// no suma una versión por guardado.
#[test]
fn el_autoguardado_del_vault_se_agrupa() {
    let (dir, data) = (TempDir::new().unwrap(), TempDir::new().unwrap());
    let (vault, _updates) = open_vault(dir.path(), data.path());
    let note = path("a.md");
    let autosave = |text: &str| {
        vault
            .save(&note, text.to_owned(), SaveKind::Autosave)
            .unwrap()
            .unwrap()
            .wait()
            .unwrap()
    };
    autosave("uno");
    autosave("dos");
    assert_eq!(contents(vault.history().unwrap(), &note), ["dos"]);
    save(&vault, &note, "dos");
    autosave("tres");
    let history = vault.history().unwrap();
    assert_eq!(contents(history, &note), ["dos", "tres"]);
    let kinds: Vec<VersionKind> = history
        .versions(&note)
        .unwrap()
        .iter()
        .map(|version| version.kind)
        .collect();
    assert_eq!(kinds, [VersionKind::Saved, VersionKind::Autosaved]);
}

#[test]
fn shut_down_espera_las_versiones_de_los_guardados() {
    let (dir, data) = (TempDir::new().unwrap(), TempDir::new().unwrap());
    let (vault, _updates) = open_vault(dir.path(), data.path());
    let note = path("a.md");
    for text in ["uno", "dos"] {
        drop(
            vault
                .save(&note, text.to_owned(), SaveKind::Explicit)
                .unwrap(),
        );
    }
    vault.shut_down();
    assert_eq!(contents(vault.history().unwrap(), &note), ["uno", "dos"]);
}

#[test]
fn el_vault_sigue_renombres_y_borrados_del_watcher() {
    let (dir, data) = (TempDir::new().unwrap(), TempDir::new().unwrap());
    let (vault, updates) = open_vault(dir.path(), data.path());
    let (a, b) = (path("a.md"), path("b.md"));
    save(&vault, &a, "uno");
    wait_for_changes(&updates, |changes| {
        changes
            .iter()
            .any(|change| matches!(change, VaultChange::Created { path, .. } if *path == a))
    });
    vault.rename(&a, &b).unwrap();
    wait_for_changes(&updates, |changes| {
        changes.iter().any(|change| matches!(change, VaultChange::Renamed { from, to, .. } if *from == a && *to == b))
    });
    save(&vault, &b, "dos");
    assert_eq!(contents(vault.history().unwrap(), &b), ["uno", "dos"]);
    // Borrar en la misma ventana del debouncer que el guardado pierde el
    // borrado en el watcher (pasa también sin historial); aquí se prueba el
    // historial, no eso.
    wait_for_changes(&updates, |changes| {
        changes
            .iter()
            .any(|change| matches!(change, VaultChange::Modified { path } if *path == b))
    });

    vault.move_to_trash(&b).unwrap();
    wait_for_changes(&updates, |changes| {
        changes
            .iter()
            .any(|change| matches!(change, VaultChange::Removed { path, .. } if *path == b))
    });
    let deleted = vault.history().unwrap().deleted_notes().unwrap();
    assert_eq!(
        deleted.iter().map(|note| &note.path).collect::<Vec<_>>(),
        [&b]
    );
}

#[test]
fn al_abrir_sigue_lo_que_paso_con_el_vault_cerrado() {
    let (dir, data) = (TempDir::new().unwrap(), TempDir::new().unwrap());
    {
        let (vault, _updates) = open_vault(dir.path(), data.path());
        save(&vault, &path("movida.md"), "me movieron");
        save(&vault, &path("borrada.md"), "me borraron");
        save(&vault, &path("vuelve.md"), "vuelvo");
        vault
            .history()
            .unwrap()
            .mark_deleted(&path("vuelve.md"), SystemTime::now())
            .unwrap();
    }
    fs::create_dir(dir.path().join("x")).unwrap();
    fs::rename(dir.path().join("movida.md"), dir.path().join("x/nueva.md")).unwrap();
    fs::remove_file(dir.path().join("borrada.md")).unwrap();

    let (vault, _updates) = open_vault(dir.path(), data.path());
    wait_until_open(&vault);
    let history = vault.history().unwrap();
    assert_eq!(contents(history, &path("x/nueva.md")), ["me movieron"]);
    assert_eq!(
        history.versions(&path("movida.md")).unwrap(),
        Vec::<Version>::new()
    );
    let deleted: Vec<VaultPath> = history
        .deleted_notes()
        .unwrap()
        .into_iter()
        .map(|note| note.path)
        .collect();
    assert_eq!(deleted, [path("borrada.md")]);
}

#[test]
fn un_cambio_desde_fuera_tambien_es_una_version() {
    let (dir, data) = (TempDir::new().unwrap(), TempDir::new().unwrap());
    let note = path("a.md");
    let (vault, updates) = open_vault(dir.path(), data.path());
    save(&vault, &note, "mía");
    wait_for_changes(&updates, |changes| {
        changes
            .iter()
            .any(|change| matches!(change, VaultChange::Created { path, .. } if *path == note))
    });
    // Como Syncthing: temporal y rename, para que el watcher no lea el
    // archivo a medio escribir.
    fs::write(dir.path().join(".a.md.tmp"), "de fuera").unwrap();
    fs::rename(dir.path().join(".a.md.tmp"), dir.path().join("a.md")).unwrap();
    wait_for_changes(&updates, |changes| {
        changes
            .iter()
            .any(|change| matches!(change, VaultChange::Modified { path } if *path == note))
    });
    // El cambio se registra después de publicarse; cerrar el vault espera a
    // que termine el lote que ya se publicó.
    drop(vault);
    let (vault, _updates) = open_vault(dir.path(), data.path());
    save(&vault, &note, "mía otra vez");
    assert_eq!(
        contents(vault.history().unwrap(), &note),
        ["mía", "de fuera", "mía otra vez"]
    );
}

#[test]
fn cerrar_el_vault_no_pierde_las_versiones_que_esperan() {
    let (dir, data) = (TempDir::new().unwrap(), TempDir::new().unwrap());
    let note = path("a.md");
    {
        let (vault, _updates) = open_vault(dir.path(), data.path());
        for text in ["uno", "dos", "tres"] {
            drop(
                vault
                    .save(&note, text.to_owned(), SaveKind::Explicit)
                    .unwrap(),
            );
        }
    }
    let (vault, _updates) = open_vault(dir.path(), data.path());
    assert_eq!(
        contents(vault.history().unwrap(), &note),
        ["uno", "dos", "tres"]
    );
}

#[test]
fn si_el_historial_falla_la_nota_igual_se_guarda() {
    let (dir, outer) = (TempDir::new().unwrap(), TempDir::new().unwrap());
    let data = outer.path().join("no es carpeta");
    fs::write(&data, "").unwrap();
    let (vault, _updates) = open_vault(dir.path(), &data);
    let pending = vault
        .save(&path("a.md"), "guardada".to_owned(), SaveKind::Explicit)
        .unwrap()
        .unwrap();
    assert_eq!(
        fs::read_to_string(dir.path().join("a.md")).unwrap(),
        "guardada"
    );
    assert!(matches!(pending.wait(), Err(VersionError::History(_))));
}

#[test]
#[cfg_attr(not(valgrind_available), ignore = "valgrind no está en el PATH")]
fn una_version_con_una_linea_cambiada_es_lineal() {
    let setup = |lines: usize| {
        let note = metalmind_perf::note(lines);
        let dir = TempDir::new().unwrap();
        let history = History::new(dir.path(), RETENTION);
        history.record(&path("a.md"), &note, epoch()).unwrap();
        let middle = note[..note.len() / 2].rfind('\n').unwrap() + 1;
        let edited = format!("{}cambio {}", &note[..middle], &note[middle..]);
        (dir, history, edited)
    };
    metalmind_perf::assert_linear(8_000, setup, |(dir, history, edited)| {
        history
            .record(&path("a.md"), &edited, epoch() + HOUR)
            .unwrap();
        dir
    });
}

/// Guarda cada nota desde un vault que se cierra enseguida.
fn save_closed(root: &Path, data: &Path, notes: &[(&str, &str)]) {
    let (vault, _updates) = open_vault(root, data);
    for (note, text) in notes {
        save(&vault, &path(note), text);
    }
}

fn deleted(history: &History) -> Vec<VaultPath> {
    history
        .deleted_notes()
        .unwrap()
        .into_iter()
        .map(|note| note.path)
        .collect()
}

#[test]
fn al_abrir_dos_notas_iguales_movidas_no_se_adivina_cual_fue_a_donde() {
    let (dir, data) = (TempDir::new().unwrap(), TempDir::new().unwrap());
    save_closed(
        dir.path(),
        data.path(),
        &[
            ("a.md", "de a"),
            ("a.md", "igual"),
            ("b.md", "de b"),
            ("b.md", "igual"),
        ],
    );
    // En orden contrario al de los nombres: emparejar por orden se equivocaría.
    fs::rename(dir.path().join("a.md"), dir.path().join("z.md")).unwrap();
    fs::rename(dir.path().join("b.md"), dir.path().join("y.md")).unwrap();

    let (vault, _updates) = open_vault(dir.path(), data.path());
    wait_until_open(&vault);
    let history = vault.history().unwrap();
    assert_eq!(deleted(history), [path("a.md"), path("b.md")]);
    assert_eq!(contents(history, &path("a.md")), ["de a", "igual"]);
    for note in ["y.md", "z.md"] {
        assert_eq!(
            history.versions(&path(note)).unwrap(),
            Vec::<Version>::new()
        );
    }
}

#[test]
fn al_abrir_un_contenido_vacio_o_repetido_no_se_toma_por_renombre() {
    let (dir, data) = (TempDir::new().unwrap(), TempDir::new().unwrap());
    save_closed(
        dir.path(),
        data.path(),
        &[("vacia.md", ""), ("plantilla.md", "# Reunión\n")],
    );
    fs::remove_file(dir.path().join("vacia.md")).unwrap();
    fs::rename(dir.path().join("plantilla.md"), dir.path().join("lunes.md")).unwrap();
    fs::write(dir.path().join("martes.md"), "# Reunión\n").unwrap();
    fs::write(dir.path().join("otra vacia.md"), "").unwrap();

    let (vault, _updates) = open_vault(dir.path(), data.path());
    wait_until_open(&vault);
    let history = vault.history().unwrap();
    assert_eq!(deleted(history), [path("plantilla.md"), path("vacia.md")]);
    for note in ["lunes.md", "martes.md", "otra vacia.md"] {
        assert_eq!(
            history.versions(&path(note)).unwrap(),
            Vec::<Version>::new()
        );
    }
}

#[test]
fn al_abrir_una_nota_movida_encima_de_una_borrada_la_deja_aparte() {
    let (dir, data) = (TempDir::new().unwrap(), TempDir::new().unwrap());
    // Sin un vault abierto: su watcher, al ver `b.md`, la volvería a dar por viva.
    let closed = History::new(data.path(), RETENTION);
    let now = SystemTime::now();
    closed.record(&path("b.md"), "b vieja", now).unwrap();
    closed.mark_deleted(&path("b.md"), now).unwrap();
    closed.record(&path("a.md"), "a", now).unwrap();
    fs::write(dir.path().join("a.md"), "a").unwrap();
    fs::rename(dir.path().join("a.md"), dir.path().join("b.md")).unwrap();

    let (vault, _updates) = open_vault(dir.path(), data.path());
    wait_until_open(&vault);
    let history = vault.history().unwrap();
    assert_eq!(contents(history, &path("b.md")), ["a"]);
    assert_eq!(
        history.versions(&path("a.md")).unwrap(),
        Vec::<Version>::new()
    );
    let [old] = history.deleted_notes().unwrap().try_into().unwrap();
    assert_eq!(old.path, path("b.md"));
    let [version] = history
        .deleted_versions(old.id)
        .unwrap()
        .try_into()
        .unwrap();
    assert_eq!(
        history.deleted_content(old.id, version.id).unwrap(),
        "b vieja"
    );
}

#[test]
fn dos_renombres_en_un_mismo_lote_siguen_la_historia() {
    let (dir, data) = (TempDir::new().unwrap(), TempDir::new().unwrap());
    let (vault, updates) = open_vault(dir.path(), data.path());
    let (a, c) = (path("a.md"), path("c.md"));
    save(&vault, &a, "uno");
    wait_for_changes(&updates, |changes| {
        changes
            .iter()
            .any(|change| matches!(change, VaultChange::Created { path, .. } if *path == a))
    });
    fs::rename(dir.path().join("a.md"), dir.path().join("b.md")).unwrap();
    fs::rename(dir.path().join("b.md"), dir.path().join("c.md")).unwrap();
    wait_for_changes(&updates, |changes| {
        changes.iter().any(|change| match change {
            VaultChange::Created { path, .. } | VaultChange::Renamed { to: path, .. } => *path == c,
            _ => false,
        })
    });
    save(&vault, &c, "dos");
    assert_eq!(contents(vault.history().unwrap(), &c), ["uno", "dos"]);
    assert_eq!(vault.history().unwrap().deleted_notes().unwrap(), []);
}

#[test]
fn muchas_notas_cambiadas_desde_fuera_quedan_cada_una_con_su_version() {
    let (dir, data) = (TempDir::new().unwrap(), TempDir::new().unwrap());
    let (vault, updates) = open_vault(dir.path(), data.path());
    let notes: Vec<VaultPath> = (0..50).map(|n| path(&format!("n{n}.md"))).collect();
    for (n, note) in notes.iter().enumerate() {
        let temp = dir.path().join(format!(".tmp{n}"));
        fs::write(&temp, format!("nota {n}")).unwrap();
        fs::rename(temp, dir.path().join(note.as_str())).unwrap();
    }
    wait_for_changes(&updates, |changes| {
        let created: HashSet<&VaultPath> = changes
            .iter()
            .filter_map(|change| match change {
                VaultChange::Created { path, .. } => Some(path),
                _ => None,
            })
            .collect();
        created.len() == notes.len()
    });
    // Se registran después de publicarse; cerrar el vault espera a que
    // terminen los lotes que ya se publicaron.
    drop(vault);
    let (vault, _updates) = open_vault(dir.path(), data.path());
    for (n, note) in notes.iter().enumerate() {
        assert_eq!(
            contents(vault.history().unwrap(), note),
            [format!("nota {n}")]
        );
    }
}

#[test]
fn sin_historial_en_las_opciones_guardar_igual_escribe() {
    let dir = TempDir::new().unwrap();
    let (vault, _updates) = Vault::open(dir.path()).unwrap();
    assert!(vault.history().is_none());
    assert!(
        vault
            .save(&path("a.md"), "uno".to_owned(), SaveKind::Explicit)
            .unwrap()
            .is_none()
    );
    assert_eq!(fs::read_to_string(dir.path().join("a.md")).unwrap(), "uno");
}

#[test]
fn al_abrir_se_aplica_la_retencion() {
    let (dir, data) = (TempDir::new().unwrap(), TempDir::new().unwrap());
    let history = History::new(data.path(), RETENTION);
    // Borrada hace mucho más de 90 días: la retención la olvida.
    history.record(&path("vieja.md"), "uno", epoch()).unwrap();
    history.mark_deleted(&path("vieja.md"), epoch()).unwrap();
    let (vault, _updates) = open_vault(dir.path(), data.path());
    // El hilo poda antes de atender este guardado.
    save(&vault, &path("otra.md"), "otra");
    assert_eq!(
        vault
            .history()
            .unwrap()
            .versions(&path("vieja.md"))
            .unwrap(),
        Vec::<Version>::new()
    );
}
