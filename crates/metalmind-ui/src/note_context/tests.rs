use metalmind_vault::EntryKind;
use pretty_assertions::assert_eq;

use super::*;
use crate::workspace::LinePosition;

fn path(path: &str) -> VaultPath {
    VaultPath::new(path).unwrap()
}

fn index(notes: &[(&str, &str)]) -> Arc<LinkIndex> {
    Arc::new(LinkIndex::from_texts(
        notes
            .iter()
            .map(|&(name, text)| (path(name), EntryKind::Note, text)),
    ))
}

/// `plan.md` recibe links de `diario.md` y `b.md`, y enlaza a `meta.md`.
fn context() -> NoteContext {
    let mut context = NoteContext::new(index(&[
        ("plan.md", "# Plan\nver la [[meta]]\n"),
        ("meta.md", "# Meta\n"),
        ("diario.md", "lunes: el [[plan]]\nmartes: el [[plan]]\n"),
        ("b.md", "el [[plan]]\n"),
    ]));
    context.show_note(Some(path("plan.md")));
    context
}

#[test]
fn cada_pestana_cuenta_lo_suyo() {
    let mut context = context();
    assert_eq!(context.counts(), (2, 1));
    assert_eq!(context.tab(), ContextTab::Backlinks);
    context.set_tab(context.tab().next());
    assert_eq!(context.tab(), ContextTab::Outgoing);
    assert_eq!(context.outline().selected_file(), Some(&path("meta.md")));
    context.set_tab(context.tab().next());
    assert_eq!(context.tab(), ContextTab::Backlinks);
}

/// `l` en una nota de los backlinks baja a su primer link sin abrir nada;
/// en el link, lo abre ahí. En salientes abre el destino.
#[test]
fn l_baja_a_los_links_y_luego_abre() {
    let mut context = context();
    assert_eq!(context.expand_or_open(), None);
    assert_eq!(context.outline().selected_row(), Some(1));
    assert_eq!(
        context.expand_or_open(),
        Some(RowOpen {
            path: path("b.md"),
            at: Some(LinePosition { line: 0, column: 3 }),
            focus_editor: true,
        })
    );
    context.set_tab(ContextTab::Outgoing);
    assert_eq!(
        context.expand_or_open().map(|open| open.path),
        Some(path("meta.md"))
    );
}

/// El clic simple abre y deja el foco en el panel; el doble se lo pasa al
/// editor. Una nota de los backlinks abre en su primer link.
#[test]
fn un_clic_abre_la_fila() {
    let mut context = context();
    let open = context.click(3, 1).unwrap();
    assert_eq!(open.path, path("diario.md"));
    assert!(!open.focus_editor);
    assert!(context.click(0, 2).unwrap().focus_editor);
}

/// Un renombre en un lote del watcher lo siguen las dos pestañas.
#[test]
fn un_lote_rehace_las_dos_pestanas() {
    let mut context = context();
    let renamed = VaultChange::Renamed {
        from: path("plan.md"),
        to: path("p/plan.md"),
        kind: EntryKind::Note,
    };
    let index = index(&[
        ("p/plan.md", "# Plan\nver la [[meta]] y [[otra]]\n"),
        ("meta.md", "# Meta\n"),
        ("diario.md", "lunes: el [[plan]]\n"),
    ]);
    context.apply(&[renamed], &index);
    assert_eq!(context.note(), Some(&path("p/plan.md")));
    assert_eq!(context.counts(), (1, 2));
}
