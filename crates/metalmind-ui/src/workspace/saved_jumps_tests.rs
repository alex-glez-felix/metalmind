use metalmind_vault::{EntryKind, SavedJump, SavedJumps, VaultChange, VaultPath};
use pretty_assertions::assert_eq;

use super::super::links::tests::vault_session;
use super::super::test_session::Session;

const A: &str = "uno\n[[b]]\ntres\n";
const B: &str = "b1\nb2\nb3\n";

fn at(session: &Session) -> (String, usize) {
    (session.file(), session.line(session.pane()))
}

fn saved(path: &str, line: usize, column: usize) -> SavedJump {
    SavedJump {
        path: VaultPath::new(path).unwrap(),
        line,
        column,
    }
}

/// Cerrar y volver a abrir el vault: la jumplist del panel y las marks
/// `A`–`Z` siguen ahí, como con el shada de nvim.
#[test]
fn la_jumplist_y_las_marks_vuelven_en_otra_sesion() {
    let notes = [("a.md", A), ("b.md", B)];
    let (dir, vault, mut session) = vault_session(&notes, "a.md");
    session.keys("jlgdjjmB");
    assert_eq!(at(&session), ("b.md".into(), 3));
    let file = dir.path().join("estado/saltos");
    let jumps = session.workspace.saved_jumps(vault.root()).unwrap();
    assert_eq!(jumps.jumps, [saved("a.md", 1, 1)]);
    assert_eq!(jumps.marks, [('B', saved("b.md", 2, 0))]);
    jumps.save(vault.root(), &file).unwrap();
    drop(session);

    let mut session = Session::open(&vault.root().join("b.md"));
    let loaded = SavedJumps::load(vault.root(), &file, &vault.snapshot());
    session
        .workspace
        .restore_jumps(vault.root(), &loaded)
        .unwrap();
    session.keys("⌃o");
    assert_eq!(at(&session), ("a.md".into(), 2));
    assert_eq!(session.cursor().get(), 5);
    session.keys("'B");
    assert_eq!(at(&session), ("b.md".into(), 3));
    session.keys("⌃o");
    assert_eq!(at(&session), ("a.md".into(), 2), "`'B` fue un salto");
}

/// Lo editado y todavía sin guardar también lo siguen lo que se guarda.
#[test]
fn se_guarda_donde_quedaron_tras_lo_editado() {
    let (_dir, vault, mut session) = vault_session(&[("a.md", A), ("b.md", B)], "a.md");
    session.keys("jjmAggOnueva⎋");
    let jumps = session.workspace.saved_jumps(vault.root()).unwrap();
    assert_eq!(jumps.marks, [('A', saved("a.md", 3, 0))]);
    assert_eq!(jumps.jumps, [saved("a.md", 3, 0)], "el gg");
}

/// Un archivo fuera del vault o sin nombre no tiene ruta del vault.
#[test]
fn lo_de_fuera_del_vault_no_se_guarda() {
    let (_dir, vault, mut session) = vault_session(&[("a.md", A)], "a.md");
    let outside = tempfile::tempdir().unwrap();
    let other = outside.path().join("fuera.md");
    std::fs::write(&other, "fuera\n").unwrap();
    session.keys("mA");
    session.keys(&format!(":e {}⏎", other.display()));
    session.keys("mF");
    let jumps = session.workspace.saved_jumps(vault.root()).unwrap();
    assert_eq!(jumps.marks, [('A', saved("a.md", 0, 0))]);
    assert_eq!(jumps.jumps, [saved("a.md", 0, 0)]);
}

/// Un renombre con Metalmind abierto ya lo siguen los `FileId`: se guarda
/// la ruta de ahora.
#[test]
fn se_guarda_la_ruta_de_una_nota_renombrada_con_la_app_abierta() {
    let (_dir, vault, mut session) = vault_session(&[("a.md", A), ("b.md", B)], "a.md");
    session.keys("jlgdjmB");
    std::fs::rename(vault.root().join("b.md"), vault.root().join("c.md")).unwrap();
    let change = VaultChange::Renamed {
        from: VaultPath::new("b.md").unwrap(),
        to: VaultPath::new("c.md").unwrap(),
        kind: EntryKind::Note,
    };
    session.workspace.files_changed(vault.root(), &[change]);
    let jumps = session.workspace.saved_jumps(vault.root()).unwrap();
    assert_eq!(jumps.marks, [('B', saved("c.md", 1, 0))]);
}
