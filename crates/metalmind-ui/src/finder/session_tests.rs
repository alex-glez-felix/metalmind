use std::cell::Cell;

use gpui_kit::{AppContext as _, TestAppContext};

use super::*;

/// Las vistas se leen del vault al abrir; cuando cambian, el focus de
/// una sigue a su nombre y su consulta nuevos, y se avisa de las dos
/// cosas.
#[test]
fn el_focus_sigue_a_su_vista() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("a.md"), "#rust").unwrap();
    fs::write(dir.path().join("b.md"), "#gs").unwrap();
    fs::create_dir(dir.path().join(".metalmind")).unwrap();
    fs::write(
        dir.path().join(".metalmind/config.yaml"),
        "views:\n  gs: \"#gs\"\n",
    )
    .unwrap();
    let (vault, updates) = Vault::open(dir.path()).unwrap();
    let prepared = Prepared::new(vault, updates, None).unwrap();
    let mut cx = TestAppContext::single();
    let session = cx.update(|cx| cx.new(|cx| VaultSession::new(prepared, cx)));
    let views = session.read_with(&cx, |session, _| session.views());
    assert_eq!(views.len(), 1);
    let focus = Focus::parse("@gs", &views).unwrap();
    session.update(&mut cx, |session, cx| session.set_focus(focus, cx));
    let events = Rc::new(Cell::new((0, 0)));
    let (on_focus, on_views) = (Rc::clone(&events), Rc::clone(&events));
    cx.update(|cx| {
        cx.subscribe(&session, move |_, _: &FocusChanged, _| {
            let (focus, views) = on_focus.get();
            on_focus.set((focus + 1, views));
        })
        .detach();
        cx.subscribe(&session, move |_, _: &ViewsChanged, _| {
            let (focus, views) = on_views.get();
            on_views.set((focus, views + 1));
        })
        .detach();
    });
    let estudio = SavedView {
        name: "estudio".to_owned(),
        query: "#rust".to_owned(),
    };
    session.update(&mut cx, |session, cx| {
        session.set_views(vec![estudio], Some(("gs", "estudio")), cx);
    });
    cx.run_until_parked();
    assert_eq!(events.get(), (1, 1));
    session.read_with(&cx, |session, _| {
        assert_eq!(session.focus().unwrap().label(), "focus: @estudio");
        let in_scope = session.in_scope().unwrap();
        assert!(in_scope.contains_file(&VaultPath::new("a.md").unwrap()));
        assert!(!in_scope.contains_file(&VaultPath::new("b.md").unwrap()));
    });
    let same = session.read_with(&cx, |session, _| session.views().to_vec());
    session.update(&mut cx, |session, cx| session.set_views(same, None, cx));
    cx.run_until_parked();
    assert_eq!(events.get(), (1, 1), "las mismas vistas no avisan");
    let gone = Vec::new();
    session.update(&mut cx, |session, cx| session.set_views(gone, None, cx));
    cx.run_until_parked();
    assert_eq!(events.get(), (2, 2));
    session.read_with(&cx, |session, _| {
        assert_eq!(session.focus().unwrap().label(), "focus: #rust");
    });
}

#[test]
fn los_recientes_se_leen_de_lo_guardado() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("state/vaults/x/recientes");
    assert_eq!(read_recents(&file), Recents::default());
    let recents = Recents::parse("b.md\na.md\n");
    super::super::writer::write_recents(&file, &recents.to_text()).unwrap();
    assert_eq!(read_recents(&file), recents);
}

#[test]
fn solo_lo_de_dentro_del_vault_tiene_ruta_del_vault() {
    let root = Path::new("/v");
    let path = |text: &str| relative_to(root, Path::new(text));
    assert_eq!(path("/v/d/a.md"), Some(VaultPath::new("d/a.md").unwrap()));
    assert_eq!(path("/otra/a.md"), None);
    assert_eq!(path("/v"), None);
    assert_eq!(path("/v/.oculta/a.md"), None);
}

#[test]
fn una_nota_sin_guardar_se_ubica_por_lo_que_existe() {
    let dir = tempfile::tempdir().unwrap();
    let root = fs::canonicalize(dir.path()).unwrap();
    assert_eq!(
        canonical(&root.join("nueva.md")),
        Some(root.join("nueva.md"))
    );
    assert_eq!(
        canonical(&root.join("no/existe.md")),
        Some(root.join("no/existe.md")),
        "su carpeta se borró desde fuera"
    );
    assert_eq!(canonical(&root.join("no/../existe.md")), None);
}
