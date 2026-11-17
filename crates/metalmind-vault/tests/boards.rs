//! Las pizarras en los índices (09 §5): sus tarjetas y el texto de sus cajas
//! son links y tags como los de una nota, y renombrar una nota o un tag las
//! reescribe.

#![allow(
    clippy::unwrap_used,
    reason = "en tests, un unwrap que falla es un test que falla"
)]

use std::fs;
use std::path::Path;
use std::sync::mpsc::Receiver;
use std::time::{Duration, Instant};

use metalmind_vault::tags::TagKey;
use metalmind_vault::{
    ApplyError, LinkIndex, LinkSubpath, LinkTarget, Mismatch, Summary, Vault, VaultOptions,
    VaultPath, VaultSnapshot, VaultUpdate,
};
use pretty_assertions::assert_eq;
use tempfile::TempDir;

/// Una caja unida a la tarjeta de `notas/plan.md`, una tarjeta a una nota
/// que no existe y un grupo, escrita como la escribe Metalmind.
const BOARD: &str = r##"{
	"nodes":[
		{"id":"g","type":"group","x":-20,"y":-20,"width":900,"height":300,"label":"[[plan]]"},
		{"id":"a","type":"text","x":0,"y":0,"width":200,"height":80,"text":"# Idea\nver [[plan|el plan]] #idea"},
		{"id":"b","type":"file","x":300,"y":0,"width":200,"height":80,"file":"notas/plan.md","subpath":"#Meta"},
		{"id":"c","type":"file","x":600,"y":0,"width":200,"height":80,"file":"falta.md"}
	],
	"edges":[
		{"id":"ab","fromNode":"a","toNode":"b"}
	]
}
"##;

fn path(path: &str) -> VaultPath {
    VaultPath::new(path).unwrap()
}

fn write(root: &Path, file: &str, text: &str) {
    let full = root.join(file);
    fs::create_dir_all(full.parent().unwrap()).unwrap();
    fs::write(full, text).unwrap();
}

fn read(root: &Path, file: &str) -> String {
    fs::read_to_string(root.join(file)).unwrap()
}

/// Un vault con la pizarra y la nota de su tarjeta.
fn vault_with_board() -> (TempDir, Vault, Receiver<VaultUpdate>) {
    let dir = TempDir::new().unwrap();
    let root = dir.path().canonicalize().unwrap();
    write(&root, "idea.canvas", BOARD);
    write(&root, "notas/plan.md", "# Meta\n#idea\n");
    let (vault, updates) = Vault::open(&root).unwrap();
    (dir, vault, updates)
}

/// Espera a que los links y los tags del vault abierto sean los de abrirlo
/// de nuevo.
fn settle(vault: &Vault, updates: &Receiver<VaultUpdate>) {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        let rebuilt = Vault::open(vault.root()).unwrap().0.snapshot();
        let current = vault.snapshot();
        if current.links == rebuilt.links && current.tags == rebuilt.tags {
            return;
        }
        let left = deadline.saturating_duration_since(Instant::now());
        if updates.recv_timeout(left).is_err() {
            assert_eq!(
                current.links, rebuilt.links,
                "los links no alcanzaron al disco"
            );
            assert_eq!(
                current.tags, rebuilt.tags,
                "los tags no alcanzaron al disco"
            );
        }
    }
}

/// `origen:línea contexto` de cada backlink de `file`.
fn backlinks(index: &LinkIndex, file: &str) -> Vec<String> {
    index
        .backlinks(&path(file))
        .map(|found| {
            let link = found.link;
            format!("{}:{} {}", found.source, link.line, link.context.text)
        })
        .collect()
}

fn tagged(snapshot: &VaultSnapshot, tag: &str) -> Vec<String> {
    let entry = snapshot.tags.index().tag(&TagKey::new(tag));
    let mut notes: Vec<String> = entry
        .into_iter()
        .flat_map(|entry| &entry.notes)
        .map(ToString::to_string)
        .collect();
    notes.sort();
    notes
}

#[test]
fn las_tarjetas_y_el_texto_de_las_cajas_son_links_y_tags() {
    let (_dir, vault, _updates) = vault_with_board();
    let snapshot = vault.snapshot();
    let board = path("idea.canvas");
    assert_eq!(
        backlinks(&snapshot.links, "notas/plan.md"),
        [
            "idea.canvas:1 ver [[plan|el plan]] #idea",
            "idea.canvas:2 notas/plan.md — # Idea",
        ]
    );
    let card = &snapshot.links.outgoing(&board)[1];
    assert_eq!(card.target, LinkTarget::Resolved(path("notas/plan.md")));
    assert_eq!(card.subpath, Some(LinkSubpath::Heading("Meta".to_owned())));
    assert_eq!(card.written_target(), "notas/plan.md");
    let unresolved: Vec<(String, &str)> = snapshot
        .links
        .unresolved()
        .into_iter()
        .map(|found| (found.source.to_string(), found.link.target_text.as_str()))
        .collect();
    assert_eq!(unresolved, [("idea.canvas".to_owned(), "/falta.md")]);
    assert_eq!(tagged(&snapshot, "idea"), ["idea.canvas", "notas/plan.md"]);
    assert_eq!(snapshot.properties.of(&board), []);
    assert_eq!(snapshot.summaries.of(&board), Some(&Summary::default()));
}

#[test]
fn una_tarjeta_apunta_a_su_ruta_exacta_y_no_a_otra_nota_con_ese_nombre() {
    let dir = TempDir::new().unwrap();
    let root = dir.path().canonicalize().unwrap();
    let card = |file: &str| {
        format!(
            r#"{{"nodes":[{{"id":"a","type":"file","x":0,"y":0,"width":9,"height":9,"file":"{file}"}}],"edges":[]}}"#
        )
    };
    write(&root, "plan.md", "");
    write(&root, "otra/plan.md", "");
    write(&root, "otra/foto.png", "");
    write(&root, "otra/a.canvas", &card("plan.md"));
    write(&root, "otra/b.canvas", &card("otra/plan.md"));
    write(&root, "otra/c.canvas", &card("foto.png"));
    write(&root, "otra/d.canvas", &card("otra/foto.png"));
    let (vault, _updates) = Vault::open(&root).unwrap();
    let target = |board: &str| {
        vault.snapshot().links.outgoing(&path(board))[0]
            .target
            .clone()
    };
    assert_eq!(
        target("otra/a.canvas"),
        LinkTarget::Resolved(path("plan.md"))
    );
    assert_eq!(
        target("otra/b.canvas"),
        LinkTarget::Resolved(path("otra/plan.md"))
    );
    assert_eq!(target("otra/c.canvas"), LinkTarget::Unresolved);
    assert_eq!(
        target("otra/d.canvas"),
        LinkTarget::Resolved(path("otra/foto.png"))
    );
}

#[test]
fn el_watcher_sigue_a_las_pizarras() {
    let (_dir, vault, updates) = vault_with_board();
    let root = vault.root().to_owned();
    write(&root, "falta.md", "");
    settle(&vault, &updates);
    assert_eq!(
        backlinks(&vault.snapshot().links, "falta.md"),
        ["idea.canvas:3 falta.md"]
    );

    write(
        &root,
        "idea.canvas",
        &BOARD.replace("#idea", "#otra [[falta]]"),
    );
    settle(&vault, &updates);
    let snapshot = vault.snapshot();
    assert_eq!(tagged(&snapshot, "idea"), ["notas/plan.md"]);
    assert_eq!(tagged(&snapshot, "otra"), ["idea.canvas"]);
    assert_eq!(backlinks(&snapshot.links, "falta.md").len(), 2);

    fs::rename(root.join("idea.canvas"), root.join("notas/mapa.canvas")).unwrap();
    settle(&vault, &updates);
    assert_eq!(tagged(&vault.snapshot(), "otra"), ["notas/mapa.canvas"]);

    // Cambiar la extensión es quitar un archivo y crear otro: se vuelve a leer.
    write(&root, "suelta.md", "[[plan]] #nota");
    settle(&vault, &updates);
    fs::rename(root.join("suelta.md"), root.join("suelta.canvas")).unwrap();
    settle(&vault, &updates);
    assert_eq!(tagged(&vault.snapshot(), "nota"), Vec::<String>::new());
    fs::rename(root.join("notas/mapa.canvas"), root.join("mapa.md")).unwrap();
    settle(&vault, &updates);
    fs::rename(root.join("mapa.md"), root.join("notas/mapa.canvas")).unwrap();
    settle(&vault, &updates);
    assert_eq!(tagged(&vault.snapshot(), "otra"), ["notas/mapa.canvas"]);

    write(&root, "notas/mapa.canvas", "{ esto no es una pizarra");
    settle(&vault, &updates);
    let snapshot = vault.snapshot();
    assert_eq!(snapshot.links.outgoing(&path("notas/mapa.canvas")), []);
    assert_eq!(tagged(&snapshot, "otra"), Vec::<String>::new());

    fs::remove_file(root.join("notas/mapa.canvas")).unwrap();
    settle(&vault, &updates);
    assert_eq!(
        backlinks(&vault.snapshot().links, "notas/plan.md"),
        Vec::<String>::new()
    );
}

/// El criterio de #271: la tarjeta sigue a su nota, el plan lo muestra como
/// una línea por nodo y deshacer devuelve los bytes de antes.
#[test]
fn renombrar_una_nota_reescribe_sus_tarjetas_y_se_deshace() {
    let (_dir, vault, updates) = vault_with_board();
    let root = vault.root().to_owned();
    let plan = vault
        .snapshot()
        .links
        .rename_plan(&path("notas/plan.md"), &path("archivo/Meta 2026.md"))
        .unwrap();
    assert_eq!(plan.conflicts, []);
    let [board] = plan.files.as_slice() else {
        panic!("solo la pizarra apunta a la nota: {:?}", plan.files);
    };
    let lines = board.whole_lines(BOARD).unwrap();
    let rows: Vec<(usize, &str, &str)> = lines
        .edits
        .iter()
        .map(|edit| (edit.line, edit.old.as_str(), edit.new.as_str()))
        .collect();
    assert_eq!(
        rows,
        [
            (
                1,
                "ver [[plan|el plan]] #idea",
                "ver [[Meta 2026|el plan]] #idea"
            ),
            (2, "notas/plan.md", "archivo/Meta 2026.md"),
        ]
    );

    let applied = vault.apply(&plan.changes()).unwrap();
    settle(&vault, &updates);
    let renamed = BOARD
        .replace("[[plan|el plan]]", "[[Meta 2026|el plan]]")
        .replace("notas/plan.md", "archivo/Meta 2026.md");
    assert_eq!(
        read(&root, "idea.canvas"),
        renamed,
        "solo cambian sus dos líneas"
    );
    assert_eq!(
        backlinks(&vault.snapshot().links, "archivo/Meta 2026.md"),
        [
            "idea.canvas:1 ver [[Meta 2026|el plan]] #idea",
            "idea.canvas:2 archivo/Meta 2026.md — # Idea",
        ]
    );

    let report = vault.undo(&applied).unwrap();
    assert_eq!(report.restored, [path("idea.canvas")]);
    assert_eq!(report.moved_back.len(), 1);
    assert_eq!(read(&root, "idea.canvas"), BOARD);
    settle(&vault, &updates);
    assert_eq!(backlinks(&vault.snapshot().links, "notas/plan.md").len(), 2);
}

/// Una pizarra de otra herramienta queda en su forma canónica al aplicar, y
/// deshacer la devuelve byte a byte.
#[test]
fn una_pizarra_con_otro_formato_se_reescribe_entera_y_vuelve_igual() {
    let dir = TempDir::new().unwrap();
    let root = dir.path().canonicalize().unwrap();
    let foreign = r#"{"nodes":[{"id":"a","type":"file","file":"plan.md","x":0,"y":0,"width":9,"height":9,"de_otro":true}],"edges":[]}"#;
    write(&root, "idea.canvas", foreign);
    write(&root, "plan.md", "");
    let (vault, _updates) = Vault::open(&root).unwrap();
    let plan = vault
        .snapshot()
        .links
        .rename_plan(&path("plan.md"), &path("meta.md"))
        .unwrap();
    let applied = vault.apply(&plan.changes()).unwrap();
    assert_eq!(
        read(&root, "idea.canvas"),
        "{\n\t\"nodes\":[\n\t\t{\"id\":\"a\",\"type\":\"file\",\"x\":0,\"y\":0,\"width\":9,\"height\":9,\"file\":\"meta.md\",\"de_otro\":true}\n\t],\n\t\"edges\":[]\n}\n"
    );
    vault.undo(&applied).unwrap();
    assert_eq!(read(&root, "idea.canvas"), foreign);
}

#[test]
fn mover_la_pizarra_no_toca_sus_tarjetas() {
    let (_dir, vault, _updates) = vault_with_board();
    let plan = vault
        .snapshot()
        .links
        .rename_plan(&path("idea.canvas"), &path("notas/idea.canvas"))
        .unwrap();
    assert_eq!(plan.files, [], "una tarjeta lleva la ruta entera");
}

#[test]
fn renombrar_un_tag_lo_cambia_en_el_texto_de_la_caja() {
    let (_dir, vault, updates) = vault_with_board();
    let root = vault.root().to_owned();
    let plan = vault
        .snapshot()
        .tags
        .plan_rename("idea", "concepto")
        .unwrap();
    let applied = vault.apply(&plan.changes).unwrap();
    settle(&vault, &updates);
    assert_eq!(
        read(&root, "idea.canvas"),
        BOARD.replace("#idea", "#concepto")
    );
    assert_eq!(read(&root, "notas/plan.md"), "# Meta\n#concepto\n");
    assert_eq!(
        tagged(&vault.snapshot(), "concepto"),
        ["idea.canvas", "notas/plan.md"]
    );
    vault.undo(&applied).unwrap();
    assert_eq!(read(&root, "idea.canvas"), BOARD);
}

/// Si la pizarra cambió desde que se indexó, no se escribe nada.
#[test]
fn una_pizarra_que_cambio_frena_el_renombre() {
    let (_dir, vault, _updates) = vault_with_board();
    let root = vault.root().to_owned();
    let plan = vault
        .snapshot()
        .links
        .rename_plan(&path("notas/plan.md"), &path("meta.md"))
        .unwrap();
    for (changed, unreadable) in [
        (BOARD.replace("notas/plan.md", "notas/otra.md"), false),
        ("{".to_owned(), true),
    ] {
        write(&root, "idea.canvas", &changed);
        let Err(ApplyError::Stale(stale)) = vault.apply(&plan.changes()) else {
            panic!("la pizarra ya no dice lo que el plan espera");
        };
        assert_eq!(stale.path, path("idea.canvas"));
        assert_eq!(matches!(stale.mismatch, Mismatch::NotABoard(_)), unreadable);
        assert_eq!(read(&root, "idea.canvas"), changed);
        assert!(root.join("notas/plan.md").exists());
    }
}

#[test]
fn la_cache_guarda_lo_escaneado_de_una_pizarra() {
    let (dir, vault, _updates) = vault_with_board();
    let fresh = vault.snapshot();
    drop(vault);
    let data = TempDir::new().unwrap();
    let options = VaultOptions {
        cache: Some(data.path().join("notas")),
        ..VaultOptions::default()
    };
    // Al soltarlo, la caché ya está escrita.
    drop(Vault::open_with(dir.path(), options.clone()).unwrap());
    let (cached, _updates) = Vault::open_with(dir.path(), options).unwrap();
    assert_eq!(cached.snapshot().links, fresh.links);
    assert_eq!(cached.snapshot().tags, fresh.tags);
}
