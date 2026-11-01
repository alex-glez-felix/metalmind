use gpui_kit::TestAppContext;
use metalmind_config::Config;
use metalmind_keymap::Keymap;
use metalmind_vault::tags::Tags;
use metalmind_vault::{EntryKind, VaultIndex, VaultPath};

use super::*;
use crate::actions::bind_keys;
use crate::tag_change::{TagRequest, VaultConfigText};

fn path(path: &str) -> VaultPath {
    VaultPath::new(path).unwrap()
}

/// Fusionar dos tags con página: el aviso que bloquea es más largo que la
/// vista previa.
fn blocked_merge() -> ChangePreview {
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
    request
        .preview(&tags, &VaultConfigText::Missing, |_| false)
        .unwrap()
        .unwrap()
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
    let (_, cx) = cx.add_window_view(|_, cx| ChangePreviewView::new(preview, cx));
    cx.run_until_parked();
    let theme = cx.update(|_, cx| Theme::get(cx).clone());
    let line = theme.ui_small_font_size * theme.text.ui_sm.line_height;
    let blocked = cx.debug_bounds("change-preview-blocked").unwrap();
    assert!(
        blocked.size.height > line * 1.5,
        "{blocked:?} en una línea de {line:?}"
    );
}

/// La vista previa de `request` sobre `a.md` con `#rust`, `notes` notas
/// más con él y la página de `#rust` con `page`.
fn preview_of(request: &TagRequest, notes: usize, page: &str) -> ChangePreview {
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
    request
        .preview(&tags, &VaultConfigText::Missing, |_| false)
        .unwrap()
        .unwrap()
}

fn window(cx: &mut TestAppContext, preview: ChangePreview) -> &mut gpui_kit::VisualTestContext {
    cx.update(|cx| {
        bind_keys(Keymap::default(), cx);
        cx.set_global(Theme::with_font(
            &Config::defaults().unwrap(),
            "Mono".into(),
        ));
    });
    let (_, cx) = cx.add_window_view(|_, cx| ChangePreviewView::new(preview, cx));
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
    let detail = cx.debug_bounds("change-preview-detail").unwrap();
    let after = cx.debug_bounds("change-preview-detail-after").unwrap();
    assert!(
        after.top() >= detail.top() && after.bottom() <= detail.bottom(),
        "{after:?} en {detail:?}"
    );
}

/// Con lo de antes y lo nuevo largos, el detalle muestra renglones enteros:
/// lo nuevo hasta todos menos uno y lo de antes en uno, con `…`, y nada
/// pasa de su borde de abajo, que es el de arriba del pie.
#[test]
fn el_detalle_muestra_renglones_enteros() {
    let old: Vec<String> = (0..40).map(|n| format!("alias-viejo-{n}")).collect();
    let new: Vec<String> = (0..40).map(|n| format!("alias-nuevo-{n}")).collect();
    let page = format!("---\naliases: [{}]\n---\n", old.join(", "));
    let request = TagRequest::Page {
        tag: "rust".to_owned(),
        field: crate::tag_change::TagField::Aliases,
        change: metalmind_vault::tags::PageChange::Aliases(new),
    };
    let mut cx = TestAppContext::single();
    let cx = window(&mut cx, preview_of(&request, 0, &page));
    cx.run_until_parked();
    let theme = cx.update(|_, cx| Theme::get(cx).clone());
    let line = theme.ui_line();
    let lines = f32::from(theme.layout.change_preview_detail_lines);
    let detail = cx.debug_bounds("change-preview-detail").unwrap();
    let after = cx.debug_bounds("change-preview-detail-after").unwrap();
    let before = cx.debug_bounds("change-preview-detail-before").unwrap();
    assert!(after.size.height <= line * (lines - 1.), "{after:?}");
    assert_eq!(before.size.height, line, "un renglón");
    assert!(after.bottom() <= before.top(), "{after:?} {before:?}");
    let inner_bottom = detail.bottom() - theme.layout.picker_input_padding_y;
    assert!(before.bottom() <= inner_bottom, "{before:?} en {detail:?}");
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
    let apply = cx.debug_bounds("change-preview-Aplicar").unwrap();
    let detail = cx.debug_bounds("change-preview-detail").unwrap();
    let theme = cx.update(|_, cx| Theme::get(cx).clone());
    assert!(apply.bottom() <= gpui_kit::px(360.), "{apply:?}");
    assert_eq!(detail.size.height, theme.change_preview_detail_height());
}

/// La vista previa de renombrar `#rust`, que tiene página, con `o`, el
/// foco y lo que emite.
fn openable(
    cx: &mut TestAppContext,
    opens_lines: bool,
) -> (
    &mut gpui_kit::VisualTestContext,
    std::rc::Rc<std::cell::RefCell<Vec<ChangePreviewEvent>>>,
) {
    let request = TagRequest::Rename {
        from: "rust".to_owned(),
        to: "lang".to_owned(),
    };
    let preview = preview_of(&request, 1, "R\n");
    cx.update(|cx| {
        bind_keys(Keymap::default(), cx);
        cx.set_global(Theme::with_font(
            &Config::defaults().unwrap(),
            "Mono".into(),
        ));
    });
    let (view, cx) = cx.add_window_view(|_, cx| {
        let view = ChangePreviewView::new(preview, cx);
        if opens_lines {
            view.with_open_line()
        } else {
            view
        }
    });
    let events = std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
    let seen = std::rc::Rc::clone(&events);
    cx.update(|window, cx| {
        cx.subscribe(&view, move |_, event: &ChangePreviewEvent, _| {
            seen.borrow_mut().push(*event);
        })
        .detach();
        let handle = view.read(cx).focus_handle(cx);
        window.focus(&handle, cx);
    });
    cx.run_until_parked();
    (cx, events)
}

/// `o` abre la línea de una edición; el renombre de la página, la última
/// fila, no tiene línea, y su botón se apaga.
#[test]
fn o_abre_la_linea_de_una_edicion() {
    let mut cx = TestAppContext::single();
    let (cx, events) = openable(&mut cx, true);
    assert!(cx.debug_bounds("change-preview-Abrir línea").is_some());
    cx.simulate_keystrokes("o");
    assert_eq!(*events.borrow(), [ChangePreviewEvent::OpenLine]);
    cx.simulate_keystrokes("k o");
    assert_eq!(events.borrow().len(), 1, "un renombre no tiene línea");
}

/// Sin quien sepa volver a ella, `o` no hace nada ni hay botón.
#[test]
fn sin_abrir_lineas_o_no_hace_nada() {
    let mut cx = TestAppContext::single();
    let (cx, events) = openable(&mut cx, false);
    assert!(cx.debug_bounds("change-preview-Abrir línea").is_none());
    cx.simulate_keystrokes("o");
    assert!(events.borrow().is_empty());
}
