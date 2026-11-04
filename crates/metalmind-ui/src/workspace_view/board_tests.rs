//! La pizarra en una ventana de verdad, con sus teclas por el keymap.

use std::fs;
use std::path::Path;

use gpui_kit::{
    AppContext as _, Entity, Modifiers, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent,
    Pixels, Point, ScrollDelta, ScrollWheelEvent, TestAppContext, VisualTestContext, point, px,
};
use metalmind_config::Config;
use metalmind_keymap::{BoardAction, Keymap, WindowCommand};
use metalmind_vault::Vault;
use metalmind_vim::Markdown;
use pretty_assertions::assert_eq;
use tempfile::TempDir;

use super::WorkspaceView;
use crate::actions::{Dispatch, bind_keys};
use crate::board::BoardView;
use crate::document::Document;
use crate::finder::{Prepared, VaultSession};
use crate::new_note::NoteSettings;
use crate::session::VaultState;
use crate::theme::Theme;
use crate::workspace::{LineNumbers, Options, PaneOptions, Workspace};

pub(super) const BOARD: &str = r#"{
	"nodes":[
		{"id":"caja","type":"text","x":0,"y":0,"width":200,"height":80,"text":"una caja"},
		{"id":"tarjeta","type":"file","x":400,"y":0,"width":200,"height":80,"file":"carpeta con espacio/la nota.md"}
	],
	"edges":[
		{"id":"e","fromNode":"caja","toNode":"tarjeta"}
	]
}
"#;

pub(super) struct Window<'a> {
    _dir: TempDir,
    pub(super) root: std::path::PathBuf,
    pub(super) view: Entity<WorkspaceView>,
    pub(super) cx: &'a mut VisualTestContext,
}

pub(super) fn open<'a>(cx: &'a mut TestAppContext, file: &str) -> Window<'a> {
    let dir = tempfile::tempdir().unwrap();
    fs::create_dir(dir.path().join("carpeta con espacio")).unwrap();
    fs::write(
        dir.path().join("carpeta con espacio/la nota.md"),
        "la nota\n",
    )
    .unwrap();
    fs::write(dir.path().join("idea.canvas"), BOARD).unwrap();
    fs::write(dir.path().join("a.md"), "uno\n").unwrap();
    let (vault, updates) = Vault::open(dir.path()).unwrap();
    let root = vault.root().to_owned();
    let path = root.join(file);
    cx.update(|cx| {
        cx.set_global(Theme::with_font(
            &Config::defaults().unwrap(),
            "Mono".into(),
        ));
        bind_keys(Keymap::default(), cx);
    });
    let prepared = Prepared::new(vault, updates, None).unwrap();
    let session = cx.update(|cx| cx.new(|cx| VaultSession::new(prepared, cx)));
    let options = Options {
        scrolloff: 0,
        tabstop: 8,
        expandtab: false,
        gutter_marks: false,
        markdown: Markdown::default(),
    };
    let workspace = cx.update(|cx| {
        cx.new(|_| {
            let pane = PaneOptions {
                numbers: LineNumbers::HYBRID,
                wrap: true,
            };
            let document = Document::open(&path).unwrap();
            Workspace::new(document, pane, options, &Keymap::default())
        })
    });
    let notes = NoteSettings {
        templates_folder: String::new(),
        daily: metalmind_config::Daily::default(),
    };
    let (view, cx) = cx.add_window_view(|window, cx| {
        WorkspaceView::new(
            workspace,
            session,
            &VaultState::default(),
            None,
            notes,
            window,
            cx,
        )
    });
    view.update_in(cx, |view, window, cx| view.focus(window, cx));
    cx.run_until_parked();
    Window {
        _dir: dir,
        root,
        view,
        cx,
    }
}

impl Window<'_> {
    pub(super) fn keys(&mut self, keys: &str) {
        self.cx.simulate_keystrokes(keys);
        self.cx.run_until_parked();
    }

    pub(super) fn board(&mut self) -> Entity<BoardView> {
        self.view.read_with(self.cx, |view, cx| {
            let pane = view.workspace.read(cx).focused();
            let editor = view.editors[&pane].read(cx);
            editor.board().cloned().unwrap()
        })
    }

    fn zoom(&mut self) -> f32 {
        let board = self.board();
        board.read_with(self.cx, |board, _| board.camera().unwrap().zoom)
    }

    fn file(&mut self) -> String {
        let root = self.root.clone();
        self.view.read_with(self.cx, |view, cx| {
            let workspace = view.workspace.read(cx);
            let path = workspace.focused_document().unwrap().path().unwrap();
            let relative = path.strip_prefix(&root).unwrap_or(path);
            relative.display().to_string()
        })
    }

    pub(super) fn text(&mut self) -> String {
        self.view.read_with(self.cx, |view, cx| {
            let workspace = view.workspace.read(cx);
            workspace.focused_document().unwrap().buffer().to_string()
        })
    }

    pub(super) fn modified(&mut self) -> bool {
        self.view.read_with(self.cx, |view, cx| {
            let workspace = view.workspace.read(cx);
            workspace.focused_document().unwrap().is_modified()
        })
    }

    pub(super) fn on_disk(&self, file: &str) -> String {
        fs::read_to_string(Path::new(&self.root).join(file)).unwrap()
    }

    pub(super) fn dispatch(&mut self, action: impl Into<metalmind_keymap::Invocation>) {
        let action = Dispatch(action.into());
        self.cx.dispatch_action(action);
        self.cx.run_until_parked();
    }
}

#[gpui_kit::test]
fn al_abrir_se_ve_todo_sin_agrandarla(cx: &mut TestAppContext) {
    let mut window = open(cx, "idea.canvas");
    assert!(window.zoom() <= 1.0);
}

#[gpui_kit::test]
fn el_zoom_responde_a_sus_teclas(cx: &mut TestAppContext) {
    let mut window = open(cx, "idea.canvas");
    window.keys("shift-0");
    assert!((window.zoom() - 1.0).abs() < 1e-3);
    window.keys("+");
    assert!((window.zoom() - 1.25).abs() < 1e-3);
    window.keys("-");
    assert!((window.zoom() - 1.0).abs() < 1e-3);
    for keys in ["shift-0", ")", "="] {
        window.keys("+");
        window.keys(keys);
        assert!((window.zoom() - 1.0).abs() < 1e-3, "{keys}");
    }
    window.keys("shift-1");
    assert!(window.zoom() <= 1.0);
}

#[gpui_kit::test]
fn h_j_k_l_mueven_la_vista(cx: &mut TestAppContext) {
    let mut window = open(cx, "idea.canvas");
    window.keys("shift-0");
    let board = window.board();
    let at = |window: &mut Window<'_>| {
        let camera = board.read_with(window.cx, |board, _| board.camera().unwrap());
        (camera.x, camera.y)
    };
    let start = at(&mut window);
    window.keys("l");
    assert!(at(&mut window).0 > start.0);
    window.keys("h");
    assert_eq!(at(&mut window), start);
    window.keys("j");
    assert!(at(&mut window).1 > start.1);
    window.keys("k");
    assert_eq!(at(&mut window), start);
}

#[gpui_kit::test]
fn shift_h_j_k_l_eligen_un_nodo(cx: &mut TestAppContext) {
    let mut window = open(cx, "idea.canvas");
    let board = window.board();
    window.keys("shift-l");
    let first = board.read_with(window.cx, |board, _| board.selected().map(str::to_owned));
    assert!(first.is_some());
    window.keys("shift-h");
    let selected = board.read_with(window.cx, |board, _| board.selected().map(str::to_owned));
    assert_eq!(selected.as_deref(), Some("caja"));
    window.keys("shift-l");
    let selected = board.read_with(window.cx, |board, _| board.selected().map(str::to_owned));
    assert_eq!(selected.as_deref(), Some("tarjeta"));
}

#[gpui_kit::test]
fn enter_y_gd_abren_la_nota_de_la_tarjeta(cx: &mut TestAppContext) {
    for keys in ["enter", "g d"] {
        let mut window = open(cx, "idea.canvas");
        window.keys(keys);
        assert_eq!(window.file(), "carpeta con espacio/la nota.md", "{keys}");
    }
}

#[gpui_kit::test]
fn sobre_una_caja_elegida_enter_no_abre_nada(cx: &mut TestAppContext) {
    let mut window = open(cx, "idea.canvas");
    window.keys("shift-l");
    window.keys("shift-h");
    window.keys("enter");
    assert_eq!(window.file(), "idea.canvas");
}

/// 09 §4.2: el motor no recibe teclas, y lo que se ejecuta en `:` no alcanza
/// el JSON del archivo.
#[gpui_kit::test]
fn ni_mirar_ni_la_linea_de_comandos_tocan_el_archivo(cx: &mut TestAppContext) {
    let mut window = open(cx, "idea.canvas");
    let before = window.text();
    window.keys("h j k l + - ! ) escape");
    assert_eq!(window.text(), before);
    window.keys(":");
    window.cx.simulate_input("%d");
    window.keys("enter");
    window.keys(":");
    window.cx.simulate_input("s/a/b/");
    window.keys("enter");
    assert_eq!(window.text(), before);
    assert!(!window.modified());
    assert_eq!(window.on_disk("idea.canvas"), BOARD);
}

#[gpui_kit::test]
fn dos_puntos_abre_la_linea_y_e_cambia_de_archivo(cx: &mut TestAppContext) {
    let mut window = open(cx, "idea.canvas");
    window.keys(":");
    window.cx.simulate_input("e a.md");
    window.keys("enter");
    assert_eq!(window.file(), "a.md");
}

#[gpui_kit::test]
fn el_lider_y_ctrl_w_siguen_siendo_de_la_ventana(cx: &mut TestAppContext) {
    let mut window = open(cx, "idea.canvas");
    window.keys("ctrl-w v");
    let panes = window.view.read_with(window.cx, |view, cx| {
        view.workspace.read(cx).tree().panes().len()
    });
    assert_eq!(panes, 2);
    window.keys("space f");
    let overlay = window
        .view
        .read_with(window.cx, |view, _| view.overlay.is_some());
    assert!(overlay);
}

#[gpui_kit::test]
fn un_archivo_que_cambia_en_disco_se_vuelve_a_leer(cx: &mut TestAppContext) {
    let mut window = open(cx, "idea.canvas");
    fs::write(window.root.join("idea.canvas"), "esto no es json").unwrap();
    window.keys(":");
    window.cx.simulate_input("e");
    window.keys("enter");
    let board = window.board();
    let notice = board.read_with(window.cx, |board, _| board.notice_text());
    assert!(notice.unwrap().starts_with("No se lee como pizarra"));
    fs::write(window.root.join("idea.canvas"), "").unwrap();
    window.keys(":");
    window.cx.simulate_input("e");
    window.keys("enter");
    let notice = board.read_with(window.cx, |board, _| board.notice_text());
    assert_eq!(notice.as_deref(), Some("Pizarra vacía"));
}

#[gpui_kit::test]
fn board_new_con_nombre_crea_y_abre_una_pizarra_vacia(cx: &mut TestAppContext) {
    let mut window = open(cx, "a.md");
    window.keys(":");
    window.cx.simulate_input("board nueva");
    window.keys("enter");
    assert_eq!(window.file(), "nueva.canvas");
    assert_eq!(
        window.on_disk("nueva.canvas"),
        "{\n\t\"nodes\":[],\n\t\"edges\":[]\n}\n"
    );
    window.board();
}

#[gpui_kit::test]
fn board_new_sin_nombre_lo_pregunta_el_panel(cx: &mut TestAppContext) {
    let mut window = open(cx, "a.md");
    window.dispatch(WindowCommand::NewBoard);
    window.cx.simulate_input("desde panel");
    window.keys("enter");
    // Junto a lo elegido en el árbol, como una nota.
    let file = window.file();
    assert!(file.ends_with("desde panel.canvas"), "{file}");
    assert!(window.root.join(file).exists());
}

#[gpui_kit::test]
fn una_accion_de_pizarra_en_una_nota_lo_dice(cx: &mut TestAppContext) {
    let mut window = open(cx, "a.md");
    window.dispatch(BoardAction::ZoomIn);
    let message = window.view.read_with(window.cx, |view, cx| {
        view.workspace
            .read(cx)
            .messages()
            .last()
            .map(|m| m.text().to_owned())
    });
    assert_eq!(
        message.as_deref(),
        Some("Este panel no muestra una pizarra")
    );
}

fn press(window: &mut Window<'_>, position: Point<Pixels>, click_count: usize) {
    window.cx.simulate_event(MouseDownEvent {
        button: MouseButton::Left,
        position,
        modifiers: Modifiers::none(),
        click_count,
        first_mouse: false,
    });
    window.cx.simulate_event(MouseUpEvent {
        button: MouseButton::Left,
        position,
        modifiers: Modifiers::none(),
        click_count,
    });
    window.cx.run_until_parked();
}

fn node_at(window: &mut Window<'_>, id: &str) -> Point<Pixels> {
    let board = window.board();
    board.read_with(window.cx, |board, _| board.window_position(id).unwrap())
}

#[gpui_kit::test]
fn un_clic_elige_el_nodo_y_en_el_fondo_lo_suelta(cx: &mut TestAppContext) {
    let mut window = open(cx, "idea.canvas");
    let board = window.board();
    let at = node_at(&mut window, "caja");
    press(&mut window, at, 1);
    let selected = board.read_with(window.cx, |board, _| board.selected().map(str::to_owned));
    assert_eq!(selected.as_deref(), Some("caja"));
    press(&mut window, at + point(px(0.), px(300.)), 1);
    assert!(board.read_with(window.cx, |board, _| board.selected().is_none()));
}

#[gpui_kit::test]
fn doble_clic_en_una_tarjeta_abre_la_nota_y_en_una_caja_no(cx: &mut TestAppContext) {
    let mut window = open(cx, "idea.canvas");
    let caja = node_at(&mut window, "caja");
    press(&mut window, caja, 1);
    press(&mut window, caja, 2);
    assert_eq!(window.file(), "idea.canvas");
    let tarjeta = node_at(&mut window, "tarjeta");
    press(&mut window, tarjeta, 1);
    press(&mut window, tarjeta, 2);
    assert_eq!(window.file(), "carpeta con espacio/la nota.md");
}

#[gpui_kit::test]
fn la_rueda_mueve_y_con_ctrl_acerca_bajo_el_raton(cx: &mut TestAppContext) {
    let mut window = open(cx, "idea.canvas");
    window.keys("shift-0");
    let board = window.board();
    let at = node_at(&mut window, "caja");
    let camera =
        |window: &mut Window<'_>| board.read_with(window.cx, |board, _| board.camera().unwrap());
    let before = camera(&mut window);
    let wheel = |modifiers| ScrollWheelEvent {
        position: at,
        delta: ScrollDelta::Pixels(point(px(0.), px(100.))),
        modifiers,
        ..ScrollWheelEvent::default()
    };
    window.cx.simulate_event(wheel(Modifiers::none()));
    window.cx.run_until_parked();
    let moved = camera(&mut window);
    assert!(moved.y < before.y);
    assert!((moved.zoom - before.zoom).abs() < 1e-3);
    window.cx.simulate_event(wheel(Modifiers::control()));
    window.cx.run_until_parked();
    assert!(camera(&mut window).zoom > before.zoom);
}

#[gpui_kit::test]
fn arrastrar_en_el_vacio_mueve_la_vista(cx: &mut TestAppContext) {
    let mut window = open(cx, "idea.canvas");
    window.keys("shift-0");
    let board = window.board();
    let at = node_at(&mut window, "caja") + point(px(0.), px(200.));
    let before = board.read_with(window.cx, |board, _| board.camera().unwrap());
    window.cx.simulate_event(MouseDownEvent {
        button: MouseButton::Left,
        position: at,
        modifiers: Modifiers::none(),
        click_count: 1,
        first_mouse: false,
    });
    window.cx.simulate_event(MouseMoveEvent {
        position: at + point(px(30.), px(10.)),
        pressed_button: Some(MouseButton::Left),
        modifiers: Modifiers::none(),
    });
    window.cx.run_until_parked();
    let after = board.read_with(window.cx, |board, _| board.camera().unwrap());
    assert!((after.x - (before.x - 30.0)).abs() < 1e-3);
    assert!((after.y - (before.y - 10.0)).abs() < 1e-3);
}

#[gpui_kit::test]
fn un_clic_en_la_barra_no_le_llega_a_la_pizarra(cx: &mut TestAppContext) {
    let mut window = open(cx, "idea.canvas");
    let board = window.board();
    let caja = node_at(&mut window, "caja");
    press(&mut window, caja, 1);
    let at = board.read_with(window.cx, |board, cx| {
        board.toolbar_position(Theme::get(cx)).unwrap()
    });
    press(&mut window, at, 1);
    let selected = board.read_with(window.cx, |board, _| board.selected().map(str::to_owned));
    assert_eq!(selected.as_deref(), Some("caja"));
}

/// El botón despacha desde el panel de la pizarra, aunque el foco esté en otro.
#[gpui_kit::test]
fn un_boton_de_la_barra_actua_sobre_su_pizarra_con_el_foco_en_otro_panel(cx: &mut TestAppContext) {
    let mut window = open(cx, "idea.canvas");
    window.keys("+");
    let board = window.board();
    let zoomed = window.zoom();
    window.keys("ctrl-w v");
    window.keys(":");
    window.cx.simulate_input("e a.md");
    window.keys("enter");
    assert_eq!(window.file(), "a.md");
    let at = board.read_with(window.cx, |board, cx| {
        board.toolbar_position(Theme::get(cx)).unwrap()
    });
    press(&mut window, at, 1);
    let camera = board.read_with(window.cx, |board, _| board.camera().unwrap());
    assert!((camera.zoom - zoomed).abs() > 1e-3, "{}", camera.zoom);
    assert_eq!(window.file(), "idea.canvas");
}
