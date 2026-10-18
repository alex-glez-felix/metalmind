use std::sync::Arc;

use nota_vault::{EntryKind, VaultIndex, VaultPath};
use pretty_assertions::assert_eq;

use super::*;

fn path(path: &str) -> VaultPath {
    VaultPath::new(path).unwrap()
}

/// `diario/`, `diario/hoy.md` y `a.md`, con la carpeta abierta.
fn panel() -> Panel {
    let index = VaultIndex::from_entries([
        (path("diario"), EntryKind::Folder),
        (path("diario/hoy.md"), EntryKind::Note),
        (path("a.md"), EntryKind::Note),
    ]);
    let tree = FileTree::with_expanded(Arc::new(index), [path("diario")]);
    Panel::new(tree, true, PanelView::Folders)
}

fn type_text(panel: &mut Panel, text: &str) {
    panel.input_mut().unwrap().insert(text);
    panel.input_changed();
}

#[test]
fn mostrar_una_vista_o_esconder_si_ya_se_ve() {
    let mut panel = panel();
    assert!(!panel.toggle(PanelView::Folders));
    assert!(panel.toggle(PanelView::Folders));
    assert!(panel.toggle(PanelView::Tags));
    assert_eq!(panel.view, PanelView::Tags);
    assert_eq!(panel.view.other(), PanelView::Folders);
}

#[test]
fn crear_dentro_de_la_carpeta_o_junto_al_archivo() {
    let cases = [
        (
            "diario",
            "idea",
            NewEntry::Note,
            Request::CreateNote(path("diario/idea.md")),
        ),
        (
            "diario/hoy.md",
            "Idea.MD",
            NewEntry::Note,
            Request::CreateNote(path("diario/Idea.MD")),
        ),
        (
            "a.md",
            "  b  ",
            NewEntry::Note,
            Request::CreateNote(path("b.md")),
        ),
        (
            "a.md",
            "x/y",
            NewEntry::Note,
            Request::CreateNote(path("x/y.md")),
        ),
        (
            "a.md",
            "nueva/",
            NewEntry::Note,
            Request::CreateFolder(path("nueva")),
        ),
        (
            "diario",
            "2026",
            NewEntry::Folder,
            Request::CreateFolder(path("diario/2026")),
        ),
    ];
    for (selected, name, kind, expected) in cases {
        let mut panel = panel();
        panel.tree.select(&path(selected));
        panel.start_create(kind);
        type_text(&mut panel, name);
        assert_eq!(panel.confirm(), Some(expected), "{selected} {name:?}");
        assert_eq!(panel.prompt(), None);
    }
}

#[test]
fn un_nombre_invalido_deja_la_pregunta_abierta_con_el_error() {
    let mut panel = panel();
    panel.start_create(NewEntry::Note);
    type_text(&mut panel, ".oculta");
    assert_eq!(panel.confirm(), None);
    assert!(matches!(panel.prompt(), Some(Prompt::Create { .. })));
    assert!(
        panel.error().unwrap().contains("oculto"),
        "{:?}",
        panel.error()
    );
    panel.cancel();
    assert_eq!(panel.prompt(), None);
    panel.start_create(NewEntry::Note);
    assert_eq!(panel.error(), None, "una pregunta nueva borra el error");
    assert_eq!(panel.confirm(), None, "un nombre vacío no hace nada");
    assert_eq!(panel.prompt(), None);
}

#[test]
fn renombrar_empieza_con_el_cursor_antes_de_la_extension() {
    let mut panel = panel();
    panel.tree.select(&path("diario/hoy.md"));
    panel.start_rename();
    let input = panel.input_mut().unwrap();
    assert_eq!((input.text(), input.cursor()), ("hoy.md", 3));
    input.clear_before();
    input.insert("ayer");
    assert_eq!(
        panel.confirm(),
        Some(Request::Rename {
            from: path("diario/hoy.md"),
            to: path("diario/ayer.md"),
        })
    );
}

#[test]
fn renombrar_no_acepta_rutas_ni_el_mismo_nombre() {
    let mut panel = panel();
    panel.tree.select(&path("a.md"));
    panel.start_rename();
    assert_eq!(panel.confirm(), None);
    assert_eq!(panel.prompt(), None);
    panel.start_rename();
    type_text(&mut panel, "/b");
    assert_eq!(panel.confirm(), None);
    assert!(panel.error().is_some());
}

#[test]
fn mover_escribe_la_ruta_entera() {
    let mut panel = panel();
    panel.tree.select(&path("a.md"));
    panel.start_move();
    let input = panel.input_mut().unwrap();
    assert_eq!(input.text(), "a.md");
    input.home();
    input.insert("diario/");
    assert_eq!(
        panel.confirm(),
        Some(Request::Rename {
            from: path("a.md"),
            to: path("diario/a.md"),
        })
    );
}

#[test]
fn borrar_pide_confirmacion() {
    let mut panel = panel();
    panel.start_delete(path("diario"), true);
    assert_eq!(
        panel.prompt(),
        Some(&Prompt::ConfirmDelete {
            path: path("diario"),
            unsaved: true,
        })
    );
    assert!(panel.input_mut().is_none());
    assert_eq!(panel.confirm(), Some(Request::Trash(path("diario"))));
    panel.start_delete(path("a.md"), false);
    panel.cancel();
    assert_eq!(panel.prompt(), None);
}

#[test]
fn el_filtro_se_aplica_al_escribir_y_escape_lo_quita() {
    let mut panel = panel();
    panel.start_filter();
    type_text(&mut panel, "hoy");
    assert_eq!(panel.tree.rows().len(), 2);
    assert_eq!(panel.confirm(), None);
    assert_eq!(panel.tree.filter(), "hoy", "Enter lo deja puesto");
    panel.start_filter();
    assert_eq!(panel.input_mut().unwrap().text(), "hoy");
    panel.cancel();
    assert_eq!(panel.tree.filter(), "");
    assert_eq!(panel.tree.rows().len(), 3);
}

#[test]
fn un_clic_abre_o_cierra_la_carpeta_y_el_doble_abre_el_archivo() {
    let mut panel = panel();
    assert_eq!(panel.click(0, 1), None);
    assert_eq!(panel.tree.rows().len(), 2, "diario se cerró");
    assert_eq!(panel.click(0, 2), None);
    assert_eq!(
        panel.tree.rows().len(),
        2,
        "el segundo clic no la vuelve a abrir"
    );
    assert_eq!(panel.click(1, 1), None);
    assert_eq!(panel.tree.selected(), Some(&path("a.md")));
    assert_eq!(panel.click(1, 2), Some(path("a.md")));
    assert_eq!(panel.click(1, 3), Some(path("a.md")));
}
