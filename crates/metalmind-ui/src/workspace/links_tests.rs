use std::fs;
use std::path::Path;

use metalmind_buffer::ByteOffset;
use metalmind_vault::Vault;
use pretty_assertions::assert_eq;
use tempfile::TempDir;

use super::super::test_session::Session;
use super::*;

/// Un vault con esas notas y una sesión en `open`, con sus links.
pub(in crate::workspace) fn vault_session(
    notes: &[(&str, &str)],
    open: &str,
) -> (TempDir, Vault, Session) {
    let dir = TempDir::new().unwrap();
    for (name, text) in notes {
        let path = dir.path().join(name);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, text).unwrap();
    }
    let (vault, _updates) = Vault::open(dir.path()).unwrap();
    let mut session = Session::open(&vault.root().join(open));
    session.workspace.set_links(vault.root(), vault.links());
    (dir, vault, session)
}

fn error(session: &Session) -> Option<String> {
    match session.workspace.message() {
        Some(Message::Error(text)) => Some(text.clone()),
        _ => None,
    }
}

#[test]
fn gd_abre_la_nota_del_link_en_el_panel() {
    let (_dir, _vault, mut session) = vault_session(
        &[("a.md", "ver [[b]] hoy\n"), ("b.md", "uno\ndos\n")],
        "a.md",
    );
    session.keys("wwgd");
    assert_eq!(session.file(), "b.md");
    assert_eq!(session.cursor(), ByteOffset::ZERO);
    assert_eq!(session.workspace.tree().len(), 1);
}

#[test]
fn enter_tambien_sigue_el_link() {
    let (_dir, _vault, mut session) =
        vault_session(&[("a.md", "[[b]]\n"), ("b.md", "b\n")], "a.md");
    session.keys("⏎");
    assert_eq!(session.file(), "b.md");
}

#[test]
fn va_al_heading_y_al_bloque_que_nombra_el_link() {
    let b = "uno\n## Entrenamiento\nfuerza\nun párrafo ^bloque\n";
    let notes = [
        ("a.md", "[[b#Entrenamiento]] y [[b#^bloque]]\n"),
        ("b.md", b),
    ];
    let (_dir, _vault, mut session) = vault_session(&notes, "a.md");
    session.keys("gd");
    assert_eq!(
        (session.file(), session.line(session.pane())),
        ("b.md".into(), 2)
    );
    session.keys("⌃o$gd");
    assert_eq!(
        (session.file(), session.line(session.pane())),
        ("b.md".into(), 4)
    );
}

#[test]
fn embeds_y_links_markdown_tambien_se_siguen() {
    let notes = [
        ("a.md", "![[b]]\n[la c](c.md)\n"),
        ("b.md", "b\n"),
        ("c.md", "c\n"),
    ];
    let (_dir, _vault, mut session) = vault_session(&notes, "a.md");
    session.keys("gd");
    assert_eq!(session.file(), "b.md");
    session.keys("⌃ojgd");
    assert_eq!(session.file(), "c.md");
}

#[test]
fn un_link_a_un_heading_de_la_misma_nota_es_un_salto() {
    let text = "[[#Fin]]\nx\n# Fin\n";
    let (_dir, _vault, mut session) = vault_session(&[("a.md", text)], "a.md");
    session.keys("gd");
    assert_eq!(session.line(session.pane()), 3);
    session.keys("⌃o");
    assert_eq!(session.line(session.pane()), 1);
}

#[test]
fn sin_link_o_sin_destino_solo_avisa() {
    let (_dir, _vault, mut session) = vault_session(&[("a.md", "texto [[nada]]\n")], "a.md");
    session.keys("gd");
    assert_eq!(
        error(&session).as_deref(),
        Some("no hay un link en el cursor")
    );
    session.keys("wwgd");
    assert_eq!(
        error(&session).as_deref(),
        Some("el link no lleva a ninguna nota: nada")
    );
    assert_eq!(session.file(), "a.md");
    // Un link que no lleva a nada no cuenta como salto.
    session.keys("⌃o");
    assert_eq!(session.cursor(), ByteOffset::new(8));
}

#[test]
fn un_heading_que_no_esta_abre_la_nota_y_avisa() {
    let notes = [("a.md", "[[b#Otro]]\n"), ("b.md", "# Uno\n")];
    let (_dir, _vault, mut session) = vault_session(&notes, "a.md");
    session.keys("gd");
    assert_eq!(session.file(), "b.md");
    assert_eq!(error(&session).as_deref(), Some("la nota no tiene #Otro"));
}

#[test]
fn si_empata_pide_elegir_y_abre_la_elegida() {
    let notes = [
        ("a.md", "[[c#Dos]]\n"),
        ("x/c.md", "uno\n"),
        ("y/c.md", "uno\n# Dos\n"),
    ];
    let (_dir, _vault, mut session) = vault_session(&notes, "a.md");
    session.keys("gd");
    assert_eq!(session.effect, Effect::ChooseLink);
    let candidates: Vec<&str> = session
        .workspace
        .link_choice()
        .unwrap()
        .candidates
        .iter()
        .map(VaultPath::as_str)
        .collect();
    assert_eq!(candidates, ["x/c.md", "y/c.md"]);
    session.workspace.choose_link(Some(1)).unwrap();
    assert_eq!(
        session
            .view(session.pane())
            .document
            .path()
            .unwrap()
            .parent()
            .unwrap()
            .file_name()
            .unwrap(),
        "y"
    );
    assert_eq!(session.line(session.pane()), 2);
    assert_eq!(session.workspace.link_choice(), None);
}

#[test]
fn descartar_la_eleccion_no_abre_nada() {
    let notes = [("a.md", "[[c]]\n"), ("x/c.md", ""), ("y/c.md", "")];
    let (_dir, _vault, mut session) = vault_session(&notes, "a.md");
    session.keys("gd");
    session.workspace.choose_link(None).unwrap();
    assert_eq!(session.file(), "a.md");
    assert_eq!(session.workspace.link_choice(), None);
}

/// Como vim con `hidden`: la nota con cambios queda abierta y modificada.
#[test]
fn con_cambios_sin_guardar_la_nota_queda_oculta() {
    let (_dir, vault, mut session) = vault_session(&[("a.md", "[[b]]\n"), ("b.md", "b\n")], "a.md");
    session.keys("Ax⎋0gd");
    assert_eq!(session.file(), "b.md");
    session.keys("⌃o");
    assert_eq!(session.file(), "a.md");
    assert_eq!(session.text(), "[[b]]x");
    assert!(session.is_modified());
    assert_eq!(
        fs::read_to_string(vault.root().join("a.md")).unwrap(),
        "[[b]]\n"
    );
}

#[test]
fn una_macro_sigue_despues_de_gd() {
    let notes = [("a.md", "[[b]]\n[[b]]\n"), ("b.md", "uno\ndos\n")];
    let (_dir, _vault, mut session) = vault_session(&notes, "a.md");
    session.keys("qqgdjq⌃o⌃oj@q");
    assert_eq!(
        (session.file(), session.line(session.pane())),
        ("b.md".into(), 2)
    );
}

#[test]
fn un_buffer_fuera_del_vault_no_sigue_links() {
    let (_dir, _vault, _) = vault_session(&[("a.md", "")], "a.md");
    let other = TempDir::new().unwrap();
    let path = other.path().join("fuera.md");
    fs::write(&path, "[[a]]\n").unwrap();
    let mut session = Session::open(&path);
    session.keys("gd");
    assert_eq!(
        error(&session).as_deref(),
        Some("no hay un vault abierto: los links no llevan a ningún lado")
    );
    let (_dir, vault, _) = vault_session(&[("a.md", "")], "a.md");
    session.workspace.set_links(vault.root(), vault.links());
    session.keys("gd");
    assert!(
        error(&session)
            .unwrap()
            .starts_with("la nota no está en el vault")
    );
    assert_eq!(
        session.workspace.focused_document().unwrap().path(),
        Some(Path::new(&path))
    );
}
