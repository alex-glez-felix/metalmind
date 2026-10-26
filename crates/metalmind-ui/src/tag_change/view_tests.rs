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
