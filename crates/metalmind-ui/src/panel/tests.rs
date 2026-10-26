use std::sync::Arc;

use metalmind_vault::tags::Tags;
use metalmind_vault::{EntryKind, VaultChange, VaultIndex, VaultPath};
use pretty_assertions::assert_eq;

use super::*;
use crate::tag_change::TagRequest;
use crate::tag_outline::RowKind;

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
    let tags = Tags::build(path("tags"), &index, |_: &VaultPath| None);
    let tree = FileTree::with_expanded(Arc::new(index), [path("diario")]);
    Panel::new(
        tree,
        TagOutline::new(Arc::new(tags)),
        true,
        PanelView::Folders,
    )
}

fn type_text(panel: &mut Panel, text: &str) {
    panel.input_mut().unwrap().insert(text);
    panel.input_changed();
}

#[test]
fn mostrar_una_vista_o_esconder_si_ya_se_ve_con_el_foco() {
    let mut panel = panel();
    assert!(!panel.toggle(PanelView::Folders, true).visible);
    assert!(panel.toggle(PanelView::Folders, false).visible);
    assert!(panel.toggle(PanelView::Tags, true).visible);
    assert_eq!(panel.view, PanelView::Tags);
    assert_eq!(panel.view.other(), PanelView::Folders);
}

/// Desde el editor, `space e` con el árbol ya a la vista no lo esconde: le
/// da el foco. Si no, `j` y `enter` irían al editor.
#[test]
fn la_vista_que_se_ve_sin_foco_sigue_visible() {
    let mut panel = panel();
    assert!(panel.toggle(PanelView::Folders, false).visible);
    assert!(panel.visible);
    assert_eq!(panel.view, PanelView::Folders);
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
fn una_extension_sin_nombre_no_es_una_nota() {
    for name in [".md", "x/.MD"] {
        let mut panel = panel();
        panel.start_create(NewEntry::Note);
        type_text(&mut panel, name);
        assert_eq!(panel.confirm(), None, "{name}");
        assert!(panel.error().unwrap().contains("oculto"), "{name}");
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
fn un_clic_abre_o_cierra_la_carpeta_y_abre_el_archivo() {
    let mut panel = panel();
    assert_eq!(panel.click(0, 1), None);
    assert_eq!(panel.tree.rows().len(), 2, "diario se cerró");
    assert_eq!(panel.click(0, 2), None);
    assert_eq!(
        panel.tree.rows().len(),
        2,
        "el segundo clic no la vuelve a abrir"
    );
    let open = |focus_editor| {
        Some(ClickOpen {
            path: path("a.md"),
            focus_editor,
        })
    };
    assert_eq!(panel.click(1, 1), open(false), "como en Zed");
    assert_eq!(panel.tree.selected(), Some(&path("a.md")));
    assert_eq!(panel.click(1, 2), open(true), "el doble pasa al editor");
    assert_eq!(panel.click(1, 3), open(true));
}

fn renamed(from: &str, to: &str) -> VaultChange {
    VaultChange::Renamed {
        from: path(from),
        to: path(to),
        kind: EntryKind::Folder,
    }
}

fn removed(at: &str) -> VaultChange {
    VaultChange::Removed {
        path: path(at),
        kind: EntryKind::Note,
    }
}

/// Si el watcher mueve la entrada de una pregunta abierta, la pregunta la
/// sigue: confirmar actúa sobre la entrada, no sobre su ruta vieja.
#[test]
fn una_pregunta_abierta_sigue_a_su_entrada() {
    let mut panel = panel();
    panel.tree.select(&path("diario/hoy.md"));
    panel.start_rename();
    panel.follow(&[renamed("diario", "viejo"), removed("otra.md")]);
    panel.input_mut().unwrap().clear_before();
    panel.input_mut().unwrap().insert("ayer");
    assert_eq!(
        panel.confirm(),
        Some(Request::Rename {
            from: path("viejo/hoy.md"),
            to: path("viejo/ayer.md"),
        })
    );

    panel.start_delete(path("diario/hoy.md"), false);
    panel.follow(&[renamed("diario", "viejo")]);
    assert_eq!(panel.confirm(), Some(Request::Trash(path("viejo/hoy.md"))));

    panel.tree.select(&path("diario"));
    panel.start_create(NewEntry::Note);
    panel.follow(&[renamed("diario", "viejo")]);
    type_text(&mut panel, "idea");
    assert_eq!(
        panel.confirm(),
        Some(Request::CreateNote(path("viejo/idea.md")))
    );
}

/// Si la entrada desaparece, la pregunta se cierra con un aviso: otra que
/// ocupe su ruta después no es la que se eligió.
#[test]
fn una_pregunta_abierta_se_cierra_si_su_entrada_desaparece() {
    let cases: [fn(&mut Panel); 4] = [
        |panel| {
            panel.tree.select(&path("diario/hoy.md"));
            panel.start_rename();
        },
        |panel| {
            panel.tree.select(&path("diario/hoy.md"));
            panel.start_move();
        },
        |panel| panel.start_delete(path("diario/hoy.md"), false),
        |panel| {
            panel.tree.select(&path("diario"));
            panel.start_create(NewEntry::Folder);
        },
    ];
    for start in cases {
        let mut panel = panel();
        start(&mut panel);
        panel.follow(&[removed("diario")]);
        assert_eq!(panel.prompt(), None);
        assert!(
            panel
                .error()
                .is_some_and(|error| error.contains("ya no existe"))
        );
        assert_eq!(panel.confirm(), None);
    }
    let mut panel = panel();
    panel.start_filter();
    panel.follow(&[removed("diario")]);
    assert!(
        matches!(panel.prompt(), Some(Prompt::Filter(_))),
        "el filtro no tiene entrada"
    );
}

/// `a.md` con `#rust` y `b.md` sin tags, en la vista de tags.
fn tags_panel() -> Panel {
    let index = VaultIndex::from_entries([
        (path("a.md"), EntryKind::Note),
        (path("b.md"), EntryKind::Note),
    ]);
    let read = |note: &VaultPath| (note.as_str() == "a.md").then(|| "#rust".to_owned());
    let tags = Tags::build(path("tags"), &index, read);
    let tree = FileTree::new(Arc::new(index));
    Panel::new(tree, TagOutline::new(Arc::new(tags)), true, PanelView::Tags)
}

/// Como en el árbol de carpetas: el clic abre o cierra un tag y abre una
/// nota, y el doble clic le pasa el foco al editor.
#[test]
fn en_la_vista_de_tags_el_clic_abre_el_tag_y_la_nota() {
    let mut panel = tags_panel();
    assert_eq!(panel.click(0, 1), None);
    assert_eq!(panel.tags.rows().len(), 3, "#rust, a.md y el buzón");
    let open = |focus_editor| {
        Some(ClickOpen {
            path: path("a.md"),
            focus_editor,
        })
    };
    assert_eq!(panel.click(1, 1), open(false));
    assert_eq!(panel.click(1, 2), open(true));
    assert_eq!(panel.click(0, 1), None);
    assert_eq!(panel.tags.rows().len(), 2, "se cerró");
}

/// Crear, borrar y filtrar son de archivos: en la vista de tags no
/// preguntan nada, ni actúan sobre la selección escondida del árbol de
/// carpetas. Renombrar y mover sobre una nota tampoco.
#[test]
fn en_la_vista_de_tags_no_se_pregunta_por_archivos() {
    let mut panel = tags_panel();
    panel.start_create(NewEntry::Note);
    panel.start_filter();
    panel.start_delete(path("a.md"), false);
    panel.click(0, 1);
    panel.click(1, 1);
    panel.start_rename();
    panel.start_move();
    assert_eq!(panel.prompt(), None);
    assert_eq!(panel.confirm(), None);
}

/// `r` sobre un tag pregunta su nombre nuevo, escrito ya con el de ahora, y
/// `m` con qué tag fusionarlo; confirmar pide la vista previa (05 §5).
#[test]
fn en_la_vista_de_tags_se_renombra_y_se_fusiona_el_tag() {
    let mut panel = tags_panel();
    panel.click(0, 1);
    panel.start_rename();
    let Some(Prompt::RenameTag { tag, input }) = panel.prompt() else {
        panic!("{:?}", panel.prompt());
    };
    assert_eq!((tag.as_str(), input.text()), ("rust", "rust"));
    panel.input_mut().unwrap().insert("lang");
    let rename = TagRequest::Rename {
        from: "rust".to_owned(),
        to: "rustlang".to_owned(),
    };
    assert_eq!(panel.confirm(), Some(Request::ChangeTags(rename)));
    panel.start_move();
    panel.input_mut().unwrap().insert("#go");
    let merge = TagRequest::Merge {
        from: "rust".to_owned(),
        into: "go".to_owned(),
    };
    assert_eq!(panel.confirm(), Some(Request::ChangeTags(merge)));
}

/// El mismo nombre, o nada, no cambia nada.
#[test]
fn renombrar_un_tag_a_su_mismo_nombre_no_pide_nada() {
    let mut panel = tags_panel();
    panel.click(0, 1);
    panel.start_rename();
    assert_eq!(panel.confirm(), None);
    panel.start_move();
    assert_eq!(panel.confirm(), None);
}

/// Un renombre o un borrado empezado en carpetas no sigue abierto en la
/// vista de tags, donde `Enter` lo confirmaría sin que se vea su archivo.
#[test]
fn cambiar_de_vista_cierra_la_pregunta() {
    let mut panel = panel();
    panel.tree.select(&path("a.md"));
    panel.start_rename();
    panel.switch_view();
    assert_eq!(panel.prompt(), None);
    assert_eq!(panel.confirm(), None);
    panel.switch_view();
    panel.start_delete(path("a.md"), false);
    panel.toggle(PanelView::Tags, true);
    assert_eq!(panel.prompt(), None);
}

/// Un filtro a medio escribir sobrevive al cambio de vista, como uno
/// confirmado con `Enter`.
#[test]
fn cambiar_de_vista_deja_el_filtro_que_se_escribia() {
    let mut panel = panel();
    panel.start_filter();
    type_text(&mut panel, "hoy");
    panel.switch_view();
    assert_eq!(panel.prompt(), None);
    panel.switch_view();
    assert_eq!(panel.tree.filter(), "hoy");
}

/// Armar el árbol de tags cuesta lo que el vault: con otra vista a la
/// vista, un lote nuevo espera a que se muestre la de tags.
#[test]
fn los_tags_nuevos_se_arman_al_mostrar_su_vista() {
    let mut panel = tags_panel();
    panel.switch_view();
    let index = VaultIndex::from_entries([(path("c.md"), EntryKind::Note)]);
    let tags = Tags::build(path("tags"), &index, |_: &VaultPath| {
        Some("#gpui".to_owned())
    });
    panel.set_tags(Arc::new(tags));
    assert_eq!(
        first_tag(&panel),
        "rust",
        "con carpetas a la vista no cambia"
    );
    panel.switch_view();
    assert_eq!(first_tag(&panel), "gpui");
}

fn first_tag(panel: &Panel) -> &str {
    match &panel.tags.rows()[0].kind {
        RowKind::Tag { name, .. } => name,
        other => panic!("la primera fila no es un tag: {other:?}"),
    }
}

/// Abrir con la vista de carpetas no arma el árbol de tags, que cuesta lo
/// que el vault; mostrarla sí.
#[test]
fn el_arbol_de_tags_se_arma_al_mostrarse() {
    let panel = tags_panel();
    assert_eq!(first_tag(&panel), "rust", "abrió en tags");
    let index = VaultIndex::from_entries([(path("a.md"), EntryKind::Note)]);
    let tags = Tags::build(path("tags"), &index, |_: &VaultPath| {
        Some("#rust".to_owned())
    });
    let tree = FileTree::new(Arc::new(index));
    let mut panel = Panel::new(
        tree,
        TagOutline::new(Arc::new(tags)),
        true,
        PanelView::Folders,
    );
    assert_eq!(panel.tags.rows(), []);
    panel.switch_view();
    assert_eq!(first_tag(&panel), "rust");
}

/// `Escape` sin pregunta quita el filtro de carpetas solo si esa vista se
/// ve: en la de tags no se toca lo que no está a la vista.
#[test]
fn escape_en_la_vista_de_tags_no_quita_el_filtro_de_carpetas() {
    let mut panel = panel();
    panel.start_filter();
    type_text(&mut panel, "hoy");
    panel.confirm();
    panel.switch_view();
    panel.escape();
    assert_eq!(panel.tree.filter(), "hoy");
    panel.switch_view();
    panel.escape();
    assert_eq!(panel.tree.filter(), "");
}

/// Al volver a tags, si los lotes que llegaron mientras se veía otra vista
/// movieron la fila elegida, se pide llevarla a la vista; si no, no.
#[test]
fn volver_a_tags_lleva_a_la_vista_la_fila_que_se_movio() {
    let mut panel = tags_panel();
    panel.tags.toggle_selected();
    panel.tags.select_row(1);
    assert_eq!(panel.switch_view(), None);
    assert_eq!(panel.switch_view(), None, "nada llegó: queda donde estaba");
    panel.switch_view();
    let index = VaultIndex::from_entries([
        (path("0.md"), EntryKind::Note),
        (path("a.md"), EntryKind::Note),
    ]);
    let tags = Tags::build(path("tags"), &index, |_: &VaultPath| {
        Some("#rust".to_owned())
    });
    panel.set_tags(Arc::new(tags));
    assert_eq!(panel.toggle(PanelView::Tags, false).moved, Some(2));
}
