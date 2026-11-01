//! El panel en focus (05 §3): `F` enfoca la carpeta o el tag elegido, y lo
//! que se crea en focus de carpeta queda dentro de ella.

use std::sync::Arc;

use metalmind_vault::tags::Tags;
use metalmind_vault::{EntryKind, VaultIndex, VaultPath};
use pretty_assertions::assert_eq;

use super::*;
use metalmind_config::SavedView;

use crate::focus::{Focus, FocusError};
use crate::picker::LineEdit;

fn path(path: &str) -> VaultPath {
    VaultPath::new(path).unwrap()
}

/// `a/b/nota.md` con `#rust`, y `c.md`.
fn panel(view: PanelView) -> Panel {
    let index = Arc::new(VaultIndex::from_entries([
        (path("a"), EntryKind::Folder),
        (path("a/b"), EntryKind::Folder),
        (path("a/b/nota.md"), EntryKind::Note),
        (path("c.md"), EntryKind::Note),
    ]));
    let tags = Tags::build(path("tags"), &index, |note: &VaultPath| {
        (note.as_str() == "a/b/nota.md").then(|| "#rust".to_owned())
    });
    let tree = FileTree::new(Arc::clone(&index));
    let mut panel = Panel::new(
        tree,
        TagOutline::new(Arc::new(tags), Arc::default()),
        true,
        view,
    );
    panel.tags.refresh();
    panel
}

fn focus_on(panel: &mut Panel, text: &str) {
    let focus = Focus::parse(text, &[]).unwrap().unwrap();
    let in_scope = focus.resolve(panel.tree.index(), panel.tags.tags());
    panel.set_focus(focus.scope().clone(), Some(Arc::new(in_scope)));
}

#[test]
fn f_enfoca_la_carpeta_o_el_tag_elegido_pero_no_un_archivo() {
    let mut folders = panel(PanelView::Folders);
    assert_eq!(folders.selected_focus(), Some(Ok(Focus::folder(path("a")))));
    folders.outline_mut().select_row(1);
    assert_eq!(folders.selected_focus(), None, "c.md no es un alcance");
    let tags = panel(PanelView::Tags);
    assert_eq!(tags.selected_focus(), Some(Ok(Focus::tag("rust"))));
}

/// `F` sobre una vista es su consulta; una con texto no se enfoca y lo
/// dice. Sin la vista de tags a la vista, no hay vista elegida.
#[test]
fn f_sobre_una_vista_enfoca_su_consulta() {
    let gs = SavedView {
        name: "gs".to_owned(),
        query: "#rust".to_owned(),
    };
    let with_text = SavedView {
        name: "texto".to_owned(),
        query: "#rust parser".to_owned(),
    };
    let mut tags = panel(PanelView::Tags);
    tags.set_views(Arc::from([gs.clone(), with_text.clone()]));
    tags.outline_mut().select_first();
    assert_eq!(tags.selected_view(), Some(&gs));
    assert_eq!(tags.selected_focus(), Some(Focus::view(&gs)));
    tags.outline_mut().select_next();
    assert_eq!(
        tags.selected_focus(),
        Some(Err(FocusError::NotTags {
            name: "texto".to_owned(),
            word: "parser".to_owned(),
        }))
    );
    tags.outline_mut().select_next();
    assert_eq!(tags.selected_view(), None, "un tag no es una vista");
    tags.outline_mut().select_first();
    tags.switch_view();
    assert_eq!(tags.selected_view(), None);
}

/// Al abrir con el panel en tags, la primera fila elegida es la primera
/// vista: `enter` o `F` justo al abrir actúan sobre ella, no sobre un tag.
#[test]
fn al_abrir_en_tags_la_elegida_es_la_primera_vista() {
    let index = Arc::new(VaultIndex::from_entries([(path("a.md"), EntryKind::Note)]));
    let tags = Tags::build(path("tags"), &index, |_: &VaultPath| {
        Some("#rust".to_owned())
    });
    let gs = SavedView {
        name: "gs".to_owned(),
        query: "#rust".to_owned(),
    };
    let outline = TagOutline::new(Arc::new(tags), Arc::from([gs.clone()]));
    let panel = Panel::new(FileTree::new(index), outline, true, PanelView::Tags);
    assert_eq!(panel.selected_view(), Some(&gs));
}

/// Con `a/b/` de focus el árbol muestra `a` y su `b`: crear sobre `a`
/// pondría la nota fuera del focus, así que va a `a/b`.
#[test]
fn en_focus_de_carpeta_se_crea_dentro_de_ella() {
    let mut panel = panel(PanelView::Folders);
    focus_on(&mut panel, "a/b/");
    assert_eq!(panel.tree.selected(), Some(&path("a")));
    panel.start_create(NewEntry::Note);
    panel.input_mut().unwrap().apply(LineEdit::Insert("x"));
    assert_eq!(panel.confirm(), Some(Request::CreateNote(path("a/b/x.md"))));
}

#[test]
fn quitar_el_focus_devuelve_todo_el_vault() {
    let mut panel = panel(PanelView::Folders);
    focus_on(&mut panel, "#rust");
    let rows = |panel: &Panel| -> Vec<String> {
        panel
            .tree
            .rows()
            .iter()
            .map(|row| row.path.to_string())
            .collect()
    };
    assert_eq!(rows(&panel), ["a"]);
    panel.set_focus(Scope::default(), None);
    assert_eq!(rows(&panel), ["a", "c.md"]);
    panel.start_create(NewEntry::Note);
    panel.input_mut().unwrap().apply(LineEdit::Insert("x"));
    assert_eq!(panel.confirm(), Some(Request::CreateNote(path("a/x.md"))));
}
