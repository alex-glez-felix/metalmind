//! `space /` de punta a punta con su hilo de búsqueda: la vista previa sigue
//! a la fila marcada (#77).

use std::fs;
use std::time::{Duration, Instant};

use gpui_kit::{Entity, TestAppContext, VisualTestContext};
use metalmind_config::Config;
use metalmind_keymap::Keymap;

use super::*;
use crate::actions::bind_keys;
use crate::picker::PickerOptions;
use crate::theme::Theme;

fn open<'a>(
    cx: &'a mut TestAppContext,
    root: &Path,
) -> (Entity<PickerView<TextSearch>>, &'a mut VisualTestContext) {
    cx.update(|cx| {
        bind_keys(Keymap::default(), cx);
        cx.set_global(Theme::with_font(
            &Config::defaults().unwrap(),
            "Mono".into(),
        ));
    });
    let options = PickerOptions {
        placeholder: "Buscar".into(),
        splits: true,
        focus: None,
    };
    let root = root.to_owned();
    cx.add_window_view(move |_, cx| {
        let search = TextSearch::new(&root, Vec::new(), None, cx);
        PickerView::new(search, options, cx)
    })
}

/// La línea marcada de la vista previa, como `número:texto`.
fn focused(view: &Entity<PickerView<TextSearch>>, cx: &mut VisualTestContext) -> Option<String> {
    view.read_with(cx, |view, _| match view.delegate().preview()? {
        Preview::Lines { lines, focus } => {
            let line = &lines[(*focus)?];
            Some(format!("{}:{}", line.number, line.text))
        }
        Preview::Message(_) => None,
    })
}

/// Espera a que la vista previa marque `expected`: la búsqueda corre en su
/// propio hilo, fuera del executor del test.
fn wait_for(view: &Entity<PickerView<TextSearch>>, cx: &mut VisualTestContext, expected: &str) {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        cx.run_until_parked();
        let shown = focused(view, cx);
        if shown.as_deref() == Some(expected) {
            return;
        }
        assert!(Instant::now() < deadline, "la vista previa marca {shown:?}");
        std::thread::sleep(Duration::from_millis(5));
    }
}

/// Llega el resultado y su vista previa; al marcar otra fila, la de esa.
#[test]
fn la_vista_previa_sigue_a_la_fila_marcada() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("a.md"), "uno\nuna idea\ntres\n").unwrap();
    fs::write(dir.path().join("b.md"), "otra idea más\n").unwrap();
    let mut cx = TestAppContext::single();
    let (view, cx) = open(&mut cx, dir.path());
    view.update(cx, |view, cx| {
        view.update_delegate(cx, |search, cx| search.edit(LineEdit::Insert("idea"), cx));
    });
    let deadline = Instant::now() + Duration::from_secs(10);
    while view.read_with(cx, |view, _| view.delegate().len()) < 2 {
        assert!(Instant::now() < deadline, "no llegaron los dos resultados");
        cx.run_until_parked();
        std::thread::sleep(Duration::from_millis(5));
    }
    let expected = |row: usize, cx: &mut VisualTestContext| {
        view.read_with(cx, |view, _| {
            let hit = view.delegate().hit(row).unwrap();
            format!("{}:{}", hit.line.line + 1, hit.line.text)
        })
    };
    let first = expected(0, cx);
    wait_for(&view, cx, &first);
    view.update(cx, |view, cx| {
        view.update_delegate(cx, |search, _| search.select_next());
    });
    let second = expected(1, cx);
    assert_ne!(first, second);
    wait_for(&view, cx, &second);
}
