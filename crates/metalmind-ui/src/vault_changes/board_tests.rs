//! Las pizarras en los cambios del vault: una tarjeta sigue a su nota y un
//! tag se renombra en el texto de una caja (09 §5).

use gpui_kit::TestAppContext;
use pretty_assertions::assert_eq;

use super::{Fixture, Seen, fixture, path, rename_tag};
use crate::change_preview::ChangePreviewEvent;

/// Una caja con un link y un tag, y la tarjeta de `b.md`.
const BOARD: &str = "{\n\t\"nodes\":[\n\t\t{\"id\":\"a\",\"type\":\"text\",\"x\":0,\"y\":0,\"width\":200,\"height\":80,\"text\":\"ver [[b]] #rust\"},\n\t\t{\"id\":\"t\",\"type\":\"file\",\"x\":300,\"y\":0,\"width\":200,\"height\":80,\"file\":\"b.md\"}\n\t],\n\t\"edges\":[]\n}\n";

fn rows(vault: &mut Fixture<'_>) -> Vec<(String, Option<usize>, String, String)> {
    vault.view().read_with(vault.cx, |view, _| {
        let preview = view.preview();
        (0..preview.len())
            .filter_map(|row| preview.row(row))
            .map(|row| {
                let path = row.path.to_string();
                (path, row.line, row.before.to_owned(), row.after.to_owned())
            })
            .collect()
    })
}

/// El criterio de #271: la tarjeta tiene su fila en la vista previa del
/// renombre, aplicar cambia su `file` y `:vaultundo` lo devuelve.
#[gpui_kit::test]
fn renombrar_una_nota_muestra_su_tarjeta_la_cambia_y_se_deshace(cx: &mut TestAppContext) {
    let notes = [("idea.canvas", BOARD), ("b.md", "# B\n"), ("c.md", "")];
    let mut vault = fixture(cx, &notes, "c.md");
    vault.plan_rename("b.md", "notas/d.md");
    vault.finish_rename(false);
    assert_eq!(vault.seen(), [Seen::Show]);
    let row = |path: &str, line, before: &str, after: &str| {
        (path.to_owned(), line, before.to_owned(), after.to_owned())
    };
    assert_eq!(
        rows(&mut vault),
        [
            row("b.md", None, "b.md", "notas/d.md"),
            row("idea.canvas", Some(1), "ver [[b]] #rust", "ver [[d]] #rust"),
            row("idea.canvas", Some(2), "b.md", "notas/d.md"),
        ]
    );
    assert_eq!(vault.read("idea.canvas"), BOARD);

    vault.press(ChangePreviewEvent::Apply);
    assert_eq!(
        vault.read("idea.canvas"),
        BOARD
            .replace("[[b]]", "[[d]]")
            .replace("b.md", "notas/d.md")
    );
    assert!(vault.exists("notas/d.md") && !vault.exists("b.md"));
    vault.undo();
    assert_eq!(vault.read("idea.canvas"), BOARD);
    assert!(vault.exists("b.md") && !vault.exists("notas/d.md"));
}

/// Quitar la fila de la tarjeta la deja apuntando a la ruta vieja.
#[gpui_kit::test]
fn la_fila_de_una_tarjeta_se_puede_quitar(cx: &mut TestAppContext) {
    let notes = [("idea.canvas", BOARD), ("b.md", "# B\n"), ("c.md", "")];
    let mut vault = fixture(cx, &notes, "c.md");
    vault.plan_rename("b.md", "d.md");
    vault.finish_rename(false);
    vault.view().update(vault.cx, |view, _| {
        view.preview_mut().select(2);
        view.preview_mut().toggle_selected();
    });
    vault.press(ChangePreviewEvent::Apply);
    assert_eq!(vault.read("idea.canvas"), BOARD.replace("[[b]]", "[[d]]"));
}

/// Un tag de una caja se renombra con los de las notas; con la pizarra
/// abierta y sin guardar, en su buffer.
#[gpui_kit::test]
fn un_tag_se_renombra_en_el_texto_de_una_caja(cx: &mut TestAppContext) {
    let notes = [("idea.canvas", BOARD), ("b.md", "#rust\n")];
    let mut vault = fixture(cx, &notes, "b.md");
    vault.change_tags(&rename_tag("rust", "lang"));
    assert_eq!(
        rows(&mut vault)[1],
        (
            "idea.canvas".to_owned(),
            Some(1),
            "rust".to_owned(),
            "lang".to_owned()
        )
    );
    vault.press(ChangePreviewEvent::Apply);
    assert_eq!(vault.read("idea.canvas"), BOARD.replace("#rust", "#lang"));
    assert_eq!(vault.read("b.md"), "#lang\n");
    vault.undo();
    assert_eq!(vault.read("idea.canvas"), BOARD);
}

/// `t` en la lista escribe en el frontmatter, que una pizarra no tiene.
#[gpui_kit::test]
fn una_pizarra_no_se_etiqueta_desde_la_lista(cx: &mut TestAppContext) {
    let mut vault = fixture(cx, &[("idea.canvas", BOARD), ("b.md", "")], "b.md");
    vault.changes.update(vault.cx, |changes, cx| {
        changes.tag_note(&path("idea.canvas"), &["gpui".to_owned()], cx);
    });
    assert_eq!(
        vault.toast(),
        "no se etiquetó idea.canvas: en una pizarra el #tag se escribe en una caja"
    );
    assert_eq!(vault.read("idea.canvas"), BOARD);
}
