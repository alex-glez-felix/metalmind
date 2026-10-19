use std::fs;

use metalmind_buffer::ByteOffset;
use metalmind_vault::{EntryKind, VaultChange, VaultPath};
use pretty_assertions::assert_eq;

use super::super::Effect;
use super::super::links::tests::vault_session;
use super::super::test_session::Session;
use super::*;

fn at(session: &Session) -> (String, usize) {
    (session.file(), session.line(session.pane()))
}

fn error(session: &Session) -> Option<String> {
    match session.workspace.message() {
        Some(Message::Error(text)) => Some(text.clone()),
        _ => None,
    }
}

const A: &str = "uno\n[[b]]\ntres\n";
const B: &str = "b1\nb2\n[[c]]\n";
const C: &str = "c1\n";

#[test]
fn ctrl_o_vuelve_al_link_y_ctrl_i_a_la_nota() {
    let (_dir, _vault, mut session) = vault_session(&[("a.md", A), ("b.md", B)], "a.md");
    session.keys("jllgd");
    assert_eq!(at(&session), ("b.md".into(), 1));
    session.keys("j⌃o");
    assert_eq!(at(&session), ("a.md".into(), 2));
    assert_eq!(session.cursor(), ByteOffset::new(6));
    session.keys("⌃i");
    assert_eq!(at(&session), ("b.md".into(), 2));
    session.keys("⌃o⇥");
    assert_eq!(at(&session), ("b.md".into(), 2));
}

#[test]
fn la_jumplist_cruza_varios_archivos_con_count() {
    let notes = [("a.md", A), ("b.md", B), ("c.md", C)];
    let (_dir, _vault, mut session) = vault_session(&notes, "a.md");
    session.keys("jgdjjgd");
    assert_eq!(at(&session), ("c.md".into(), 1));
    session.keys("2⌃o");
    assert_eq!(at(&session), ("a.md".into(), 2));
    session.keys("2⌃i");
    assert_eq!(at(&session), ("c.md".into(), 1));
}

#[test]
fn abrir_otra_nota_con_e_cuenta_como_salto() {
    let (_dir, vault, mut session) = vault_session(&[("a.md", A), ("b.md", B)], "a.md");
    session.keys("jj");
    let b = vault.root().join("b.md");
    session.keys(&format!(":e {}⏎", b.display()));
    assert_eq!(session.file(), "b.md");
    session.keys("⌃o");
    assert_eq!(at(&session), ("a.md".into(), 3));
}

#[test]
fn cada_panel_tiene_su_jumplist_y_un_split_copia_la_suya() {
    let (_dir, _vault, mut session) = vault_session(&[("a.md", A), ("b.md", B)], "a.md");
    session.keys("jgd⌃wv");
    let right = session.pane();
    session.keys("⌃o");
    assert_eq!(at(&session), ("a.md".into(), 2));
    session.keys("⌃wh");
    assert_ne!(session.pane(), right);
    assert_eq!(session.file(), "b.md");
    session.keys("G⌃o");
    assert_eq!(at(&session), ("b.md".into(), 1));
}

#[test]
fn las_entradas_de_otro_panel_siguen_a_lo_editado() {
    let (_dir, _vault, mut session) = vault_session(&[("a.md", A), ("b.md", B)], "a.md");
    session.keys("jgd⌃wvgg⌃o");
    assert_eq!(at(&session), ("a.md".into(), 2));
    session.keys("ggOnueva⎋⌃wh⌃o");
    assert_eq!(at(&session), ("a.md".into(), 3));
}

#[test]
fn una_entrada_de_un_archivo_borrado_se_salta() {
    let notes = [("a.md", A), ("b.md", B), ("c.md", C)];
    let (_dir, vault, mut session) = vault_session(&notes, "a.md");
    session.keys("jgdjjgd");
    fs::remove_file(vault.root().join("b.md")).unwrap();
    let removed = VaultChange::Removed {
        path: VaultPath::new("b.md").unwrap(),
        kind: EntryKind::Note,
    };
    session.workspace.files_changed(vault.root(), &[removed]);
    session.keys("⌃o");
    assert_eq!(at(&session), ("a.md".into(), 2));
}

#[test]
fn mandar_una_nota_a_la_papelera_salta_sus_entradas() {
    let notes = [("a.md", A), ("b.md", B), ("c.md", C)];
    let (_dir, vault, mut session) = vault_session(&notes, "a.md");
    session.keys("jgdjjgd");
    let b = VaultPath::new("b.md").unwrap();
    session.workspace.trash_file(&vault, &b).unwrap();
    session.keys("⌃o");
    assert_eq!(at(&session), ("a.md".into(), 2));
}

#[test]
fn una_mark_de_archivo_vuelve_a_su_nota_desde_otra() {
    let (_dir, _vault, mut session) =
        vault_session(&[("a.md", "uno\n  dos x\n[[b]]\n"), ("b.md", B)], "a.md");
    session.keys("j$mAjgd");
    assert_eq!(session.file(), "b.md");
    session.keys("'A");
    assert_eq!(at(&session), ("a.md".into(), 2));
    assert_eq!(session.cursor(), ByteOffset::new(6));
    session.keys("⌃o");
    assert_eq!(session.file(), "b.md");
    session.keys("`A");
    assert_eq!(session.cursor(), ByteOffset::new(10));
}

#[test]
fn una_mark_de_archivo_sigue_al_renombre() {
    let (_dir, vault, mut session) =
        vault_session(&[("a.md", "uno\ndos\n[[b]]\n"), ("b.md", B)], "a.md");
    session.keys("jmAjgd");
    let from = VaultPath::new("a.md").unwrap();
    let to = VaultPath::new("notas/renombrada.md").unwrap();
    fs::create_dir(vault.root().join("notas")).unwrap();
    session.workspace.rename_file(&vault, &from, &to).unwrap();
    session.keys("'A");
    assert_eq!(at(&session), ("renombrada.md".into(), 2));
}

#[test]
fn una_mark_de_archivo_sigue_a_un_renombre_desde_fuera() {
    let (_dir, vault, mut session) = vault_session(
        &[("a.md", "uno\ndos\n"), ("b.md", B), ("c.md", "[[b]]\n")],
        "a.md",
    );
    session.keys("jmA");
    session.keys(&format!(":e {}⏎", vault.root().join("c.md").display()));
    fs::rename(vault.root().join("a.md"), vault.root().join("z.md")).unwrap();
    let renamed = VaultChange::Renamed {
        from: VaultPath::new("a.md").unwrap(),
        to: VaultPath::new("z.md").unwrap(),
        kind: EntryKind::Note,
    };
    session.workspace.files_changed(vault.root(), &[renamed]);
    session.keys("'A");
    assert_eq!(at(&session), ("z.md".into(), 2));
}

#[test]
fn la_mark_de_un_archivo_borrado_es_e20() {
    let (_dir, vault, mut session) =
        vault_session(&[("a.md", "uno\n"), ("b.md", B), ("c.md", "c\n")], "a.md");
    session.keys("mA");
    session.keys(&format!(":e {}⏎", vault.root().join("c.md").display()));
    let removed = VaultChange::Removed {
        path: VaultPath::new("a.md").unwrap(),
        kind: EntryKind::Note,
    };
    session.workspace.files_changed(vault.root(), &[removed]);
    session.keys("'A");
    assert_eq!(session.file(), "c.md");
    assert_eq!(error(&session).as_deref(), Some("E20: Mark not set"));
}

#[test]
fn las_marks_de_cada_nota_sobreviven_a_cambiar_y_volver() {
    let (_dir, vault, mut session) =
        vault_session(&[("a.md", "uno\ndos\n[[b]]\n"), ("b.md", B)], "a.md");
    session.keys("jmaGgd");
    assert_eq!(session.file(), "b.md");
    session.keys("jma");
    session.keys(&format!(":e {}⏎", vault.root().join("a.md").display()));
    session.keys("gg'a");
    assert_eq!(at(&session), ("a.md".into(), 2));
    session.keys(&format!(":e {}⏎", vault.root().join("b.md").display()));
    session.keys("G'a");
    assert_eq!(at(&session), ("b.md".into(), 2));
}

#[test]
fn una_macro_con_ctrl_o_cruza_de_archivo_y_sigue() {
    let (_dir, _vault, mut session) = vault_session(&[("a.md", A), ("b.md", B)], "a.md");
    session.keys("jgd");
    session.keys("qq⌃ojq");
    assert_eq!(at(&session), ("a.md".into(), 3));
    session.keys("⌃i@q");
    assert_eq!(at(&session), ("a.md".into(), 3));
}

/// Guardar un `[No Name]` en la ruta de una nota que se mandó a la papelera
/// le da esa ruta: el documento y su archivo son el mismo para la jumplist.
#[test]
fn un_buffer_guardado_en_la_ruta_de_una_nota_borrada_se_queda_con_ella() {
    let (_dir, vault, mut session) = vault_session(&[("a.md", "uno\n"), ("b.md", B)], "a.md");
    session.keys("mA");
    let a = VaultPath::new("a.md").unwrap();
    session.workspace.trash_file(&vault, &a).unwrap();
    let path = vault.root().join("a.md");
    session.keys(&format!("inueva⎋:w {}⏎", path.display()));
    session.keys(&format!(":e {}⏎", vault.root().join("b.md").display()));
    session.keys("⌃o");
    assert_eq!(
        (session.file(), session.text()),
        ("a.md".into(), "nueva".into())
    );
    session.keys("'A");
    assert_eq!(error(&session).as_deref(), Some("E20: Mark not set"));
}

/// Una nota borrada que vuelve a existir y se abre antes de que el watcher
/// avise vuelve a valer para la jumplist.
#[test]
fn reabrir_una_nota_borrada_la_devuelve_a_la_jumplist() {
    let notes = [("a.md", A), ("b.md", B), ("c.md", C)];
    let (_dir, vault, mut session) = vault_session(&notes, "a.md");
    let b = VaultPath::new("b.md").unwrap();
    session.workspace.trash_file(&vault, &b).unwrap();
    fs::write(vault.root().join("b.md"), B).unwrap();
    session.keys(&format!(":e {}⏎", vault.root().join("b.md").display()));
    session.keys("jjgd");
    assert_eq!(session.file(), "c.md");
    session.keys("⌃o");
    assert_eq!(at(&session), ("b.md".into(), 3));
}

/// Como `autoread`: una nota oculta sin cambios que cambió en disco se
/// relee; una con cambios se queda como estaba.
#[test]
fn una_nota_oculta_se_relee_si_cambia_en_disco() {
    let (_dir, vault, mut session) =
        vault_session(&[("a.md", "uno\n"), ("b.md", B), ("c.md", C)], "a.md");
    let path = |name: &str| vault.root().join(name);
    session.keys(&format!(
        ":e {}⏎Ax⎋:e! {}⏎",
        path("c.md").display(),
        path("b.md").display()
    ));
    fs::write(path("a.md"), "de fuera\n").unwrap();
    fs::write(path("c.md"), "de fuera\n").unwrap();
    let modified = |name: &str| VaultChange::Modified {
        path: VaultPath::new(name).unwrap(),
    };
    session
        .workspace
        .files_changed(vault.root(), &[modified("a.md"), modified("c.md")]);
    session.keys(&format!(":e {}⏎", path("a.md").display()));
    assert_eq!(session.text(), "de fuera");
    session.keys(&format!(":e {}⏎", path("c.md").display()));
    assert_eq!(session.text(), "c1x");
}

/// Releer una nota oculta que cambió en disco conserva sus marks en su
/// línea, como vim; también `:e!`.
#[test]
fn releer_una_nota_conserva_sus_marks() {
    let (_dir, vault, mut session) =
        vault_session(&[("a.md", "uno\ndos\ntres\n"), ("b.md", B)], "a.md");
    let path = |name: &str| vault.root().join(name);
    session.keys(&format!("jjma:e {}⏎", path("b.md").display()));
    fs::write(path("a.md"), "UNO\nDOS\nTRES\n").unwrap();
    let modified = VaultChange::Modified {
        path: VaultPath::new("a.md").unwrap(),
    };
    session.workspace.files_changed(vault.root(), &[modified]);
    session.keys(&format!(":e {}⏎gg'a", path("a.md").display()));
    assert_eq!(
        (session.text(), session.line(session.pane())),
        ("UNO\nDOS\nTRES".into(), 3)
    );
    fs::write(path("a.md"), "uno\n").unwrap();
    // vim da E19 y la guarda; aquí se pierde con su línea.
    session.keys(":e!⏎gg'a");
    assert_eq!(session.line(session.pane()), 1);
    assert_eq!(error(&session).as_deref(), Some("E20: Mark not set"));
}

/// Con `hidden`, todos los caminos dejan oculta la nota con cambios: `gd`,
/// `ctrl-o`, `'A` y `:e`. Vuelve con su texto y `:q` no sale mientras la
/// tenga; al mostrarla, como nvim, deja un salto que `ctrl-o` deshace.
#[test]
fn los_saltos_dejan_oculta_la_nota_con_cambios_y_q_la_muestra() {
    let (_dir, vault, mut session) = vault_session(&[("a.md", A), ("b.md", B)], "a.md");
    session.keys("mAx");
    session.keys("jgd");
    assert_eq!(session.file(), "b.md");
    assert_eq!(error(&session), None);
    session.keys("⌃o");
    assert_eq!(session.file(), "a.md");
    assert_eq!(session.text(), "no\n[[b]]\ntres");
    session.keys("⌃i'A");
    assert_eq!(session.file(), "a.md");
    assert!(session.is_modified());
    let b = vault.root().join("b.md");
    session.keys(&format!(":e {}⏎", b.display()));
    assert_eq!(session.file(), "b.md");
    session.keys(":q⏎");
    assert_eq!(session.effect, Effect::Stay);
    assert_eq!(session.file(), "a.md");
    assert!(error(&session).is_some_and(|text| text.starts_with("E162")));
    session.keys("⌃o");
    assert_eq!(session.file(), "b.md");
}

/// Si el watcher relee una nota oculta que se acortó fuera, el cursor
/// guardado sigue a su línea: no queda en un offset del texto viejo.
#[test]
fn releer_una_nota_oculta_reubica_su_cursor() {
    let (_dir, vault, mut session) =
        vault_session(&[("a.md", "uno\ndos\ntres\ncuatro\n"), ("b.md", B)], "a.md");
    let path = |name: &str| vault.root().join(name);
    session.keys(&format!("3jll:e {}⏎", path("b.md").display()));
    fs::write(path("a.md"), "x\nyy\nz\nw\n").unwrap();
    let modified = VaultChange::Modified {
        path: VaultPath::new("a.md").unwrap(),
    };
    session.workspace.files_changed(vault.root(), &[modified]);
    session.keys(&format!(":e {}⏎", path("a.md").display()));
    assert_eq!(session.text(), "x\nyy\nz\nw");
    assert_eq!(session.cursor(), ByteOffset::new(7), "línea 4, columna 0");
}
