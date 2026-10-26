use gpui_kit::TestAppContext;
use metalmind_config::Config;
use metalmind_keymap::Keymap;
use metalmind_vault::tags::Tags;
use metalmind_vault::{EntryKind, VaultIndex, VaultPath};

use super::*;
use crate::actions::bind_keys;
use crate::tag_change::TagRequest;

fn path(path: &str) -> VaultPath {
    VaultPath::new(path).unwrap()
}

/// Fusionar dos tags con página: el aviso que bloquea es más largo que la
/// vista previa.
fn blocked_merge() -> TagChangePreview {
    let notes = [
        ("a.md", "#rust #go\n"),
        ("tags/rust.md", "R\n"),
        ("tags/go.md", "G\n"),
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
    let request = TagRequest::Merge {
        from: "rust".to_owned(),
        into: "go".to_owned(),
    };
    let plan = request.plan(&tags).unwrap();
    TagChangePreview::new(request.title(), plan, |_| false).unwrap()
}

/// Un aviso que no cabe en una línea se parte: lo que dice es por qué no se
/// aplica y tiene que leerse entero.
#[test]
fn un_aviso_largo_se_parte_en_lineas() {
    let mut cx = TestAppContext::single();
    cx.update(|cx| {
        bind_keys(Keymap::default(), cx);
        cx.set_global(Theme::with_font(
            &Config::defaults().unwrap(),
            "Mono".into(),
        ));
    });
    let preview = blocked_merge();
    let (_, cx) = cx.add_window_view(|_, cx| TagChangeView::new(preview, cx));
    cx.run_until_parked();
    let theme = cx.update(|_, cx| Theme::get(cx).clone());
    let line = theme.ui_small_font_size * theme.text.ui_sm.line_height;
    let blocked = cx.debug_bounds("tag-change-blocked").unwrap();
    assert!(
        blocked.size.height > line * 1.5,
        "{blocked:?} en una línea de {line:?}"
    );
}

/// La vista previa de `request` sobre `a.md` con `#rust`, `notes` notas
/// más con él y la página de `#rust` con `page`.
fn preview_of(request: &TagRequest, notes: usize, page: &str) -> TagChangePreview {
    let names: Vec<String> = (0..=notes).map(|n| format!("n{n:03}.md")).collect();
    let index = VaultIndex::from_entries(
        names
            .iter()
            .map(|name| (path(name), EntryKind::Note))
            .chain([
                (path("tags"), EntryKind::Folder),
                (path("tags/rust.md"), EntryKind::Note),
            ]),
    );
    let read = |note: &VaultPath| {
        let text = if note.as_str() == "tags/rust.md" {
            page
        } else {
            "#rust\n"
        };
        Some(text.to_owned())
    };
    let tags = Tags::build(path("tags"), &index, read);
    let plan = request.plan(&tags).unwrap();
    TagChangePreview::new(request.title(), plan, |_| false).unwrap()
}

fn window(cx: &mut TestAppContext, preview: TagChangePreview) -> &mut gpui_kit::VisualTestContext {
    cx.update(|cx| {
        bind_keys(Keymap::default(), cx);
        cx.set_global(Theme::with_font(
            &Config::defaults().unwrap(),
            "Mono".into(),
        ));
    });
    let (_, cx) = cx.add_window_view(|_, cx| TagChangeView::new(preview, cx));
    cx
}

/// Un valor de antes largo no tapa el nuevo: el nuevo va primero en el
/// detalle y se ve dentro de su alto.
#[test]
fn el_valor_nuevo_se_ve_aunque_el_de_antes_sea_largo() {
    let long: Vec<String> = (0..40).map(|n| format!("alias-largo-{n}")).collect();
    let page = format!("---\naliases: [{}]\n---\n", long.join(", "));
    let request = TagRequest::Page {
        tag: "rust".to_owned(),
        field: crate::tag_change::TagField::Aliases,
        change: metalmind_vault::tags::PageChange::Aliases(vec!["rs".to_owned()]),
    };
    let mut cx = TestAppContext::single();
    let cx = window(&mut cx, preview_of(&request, 0, &page));
    cx.run_until_parked();
    let detail = cx.debug_bounds("tag-change-detail").unwrap();
    let after = cx.debug_bounds("tag-change-detail-after").unwrap();
    assert!(
        after.top() >= detail.top() && after.bottom() <= detail.bottom(),
        "{after:?} en {detail:?}"
    );
}

/// En una ventana baja la lista se encoge y los botones siguen dentro.
#[test]
fn en_una_ventana_baja_los_botones_se_ven() {
    let request = TagRequest::Rename {
        from: "rust".to_owned(),
        to: "rustlang".to_owned(),
    };
    let mut cx = TestAppContext::single();
    let cx = window(&mut cx, preview_of(&request, 30, "R\n"));
    cx.simulate_resize(gpui_kit::size(gpui_kit::px(800.), gpui_kit::px(360.)));
    cx.run_until_parked();
    let apply = cx.debug_bounds("tag-change-Aplicar").unwrap();
    let detail = cx.debug_bounds("tag-change-detail").unwrap();
    let theme = cx.update(|_, cx| Theme::get(cx).clone());
    assert!(apply.bottom() <= gpui_kit::px(360.), "{apply:?}");
    assert_eq!(detail.size.height, theme.tag_change_detail_height());
}
