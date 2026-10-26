//! Los campos de la página de un tag desde la vista de tags (05 §3).

use std::sync::Arc;

use metalmind_vault::tags::{PageChange, TagIcon, Tags};
use metalmind_vault::{EntryKind, VaultIndex, VaultPath};
use pretty_assertions::assert_eq;

use super::*;
use crate::tag_change::{TagField, TagRequest};

fn path(path: &str) -> VaultPath {
    VaultPath::new(path).unwrap()
}

/// `a.md` con `#rust` y la página de `#rust` con un alias, un padre y un
/// ícono, en la vista de tags con `#rust` elegido.
fn panel() -> Panel {
    let notes = [
        ("a.md", "#rust"),
        (
            "tags/rust.md",
            "---\naliases: [rs]\nparents: [lenguajes]\nicon: 🦀\n---\n",
        ),
    ];
    let index = VaultIndex::from_entries(
        notes
            .iter()
            .map(|(name, _)| (path(name), EntryKind::Note))
            .chain([(path("tags"), EntryKind::Folder)]),
    );
    let read = |note: &VaultPath| {
        notes
            .iter()
            .find(|(name, _)| *name == note.as_str())
            .map(|(_, text)| (*text).to_owned())
    };
    let tags = Tags::build(path("tags"), &index, read);
    let tree = FileTree::new(Arc::new(index));
    let mut panel = Panel::new(tree, TagOutline::new(Arc::new(tags)), true, PanelView::Tags);
    // `#rust` está dentro de su padre.
    for tag in ["lenguajes", "rust"] {
        let row = panel
            .tags
            .rows()
            .iter()
            .position(|row| row.tag_name() == Some(tag))
            .unwrap();
        panel.click(row, 1);
    }
    panel
}

fn asked(panel: &Panel) -> (TagField, String) {
    let Some(Prompt::TagField { tag, field, input }) = panel.prompt() else {
        panic!("{:?}", panel.prompt());
    };
    assert_eq!(tag, "rust");
    (*field, input.text().to_owned())
}

/// Cada campo se pregunta escrito ya con lo que tiene la página.
#[test]
fn la_pregunta_empieza_con_lo_que_tiene_la_pagina() {
    let cases = [
        (TagField::Aliases, "rs"),
        (TagField::Parents, "lenguajes"),
        (TagField::Icon, "🦀"),
        (TagField::Template, ""),
    ];
    for (field, current) in cases {
        let mut panel = panel();
        panel.start_tag_field(field);
        assert_eq!(asked(&panel), (field, current.to_owned()));
    }
}

/// Confirmar pide la vista previa del cambio; los nombres se separan con
/// coma o espacio y el `#` es opcional.
#[test]
fn confirmar_pide_la_vista_previa_del_cambio() {
    let mut panel = panel();
    panel.start_tag_field(TagField::Aliases);
    panel.input_mut().unwrap().insert(", #rustlang  ferris");
    assert_eq!(
        panel.confirm(),
        Some(Request::ChangeTags(TagRequest::Page {
            tag: "rust".to_owned(),
            field: TagField::Aliases,
            change: PageChange::Aliases(vec![
                "rs".to_owned(),
                "#rustlang".to_owned(),
                "ferris".to_owned(),
            ]),
        }))
    );
    panel.start_tag_field(TagField::Icon);
    panel.input_mut().unwrap().clear_before();
    panel.input_mut().unwrap().insert("lucide:book");
    let Some(Request::ChangeTags(TagRequest::Page { change, .. })) = panel.confirm() else {
        panic!("tenía que pedir el cambio");
    };
    assert_eq!(
        change,
        PageChange::Icon(Some(TagIcon::Set {
            set: "lucide".to_owned(),
            name: "book".to_owned(),
        }))
    );
}

/// Vaciar la pregunta quita el campo.
#[test]
fn vacio_quita_el_campo() {
    let mut panel = panel();
    panel.start_tag_field(TagField::Parents);
    panel.input_mut().unwrap().clear_before();
    let Some(Request::ChangeTags(TagRequest::Page { change, .. })) = panel.confirm() else {
        panic!("tenía que pedir el cambio");
    };
    assert_eq!(change, PageChange::Parents(Vec::new()));
}

/// Un ícono o una plantilla que no valen dejan la pregunta abierta con el
/// error.
#[test]
fn un_valor_invalido_deja_la_pregunta_abierta() {
    let cases = [
        (
            TagField::Icon,
            "libro",
            "`libro` no es un ícono: va un emoji o `set:nombre`",
        ),
        (
            TagField::Template,
            "../fuera",
            "`../fuera` no es el nombre de una plantilla",
        ),
    ];
    for (field, text, error) in cases {
        let mut panel = panel();
        panel.start_tag_field(field);
        panel.input_mut().unwrap().clear_before();
        panel.input_mut().unwrap().insert(text);
        assert_eq!(panel.confirm(), None);
        assert_eq!(asked(&panel).0, field);
        assert_eq!(panel.error(), Some(error));
    }
}

/// En la vista de carpetas no hay tag elegido: no se pregunta nada.
#[test]
fn en_la_vista_de_carpetas_no_se_pregunta() {
    let mut panel = panel();
    panel.switch_view();
    panel.start_tag_field(TagField::Aliases);
    assert_eq!(panel.prompt(), None);
}

/// Cerrar la pregunta con `esc` se lleva su error.
#[test]
fn cerrar_la_pregunta_quita_su_error() {
    let mut panel = panel();
    panel.start_tag_field(TagField::Icon);
    panel.input_mut().unwrap().clear_before();
    panel.input_mut().unwrap().insert("libro");
    assert_eq!(panel.confirm(), None);
    assert!(panel.error().is_some());
    panel.escape();
    assert_eq!(panel.prompt(), None);
    assert_eq!(panel.error(), None);
}
