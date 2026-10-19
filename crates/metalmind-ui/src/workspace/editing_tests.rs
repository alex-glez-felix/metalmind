//! `ctrl-shift-c` y `ctrl-shift-v`, y lo que la búsqueda deja para pintar.

use metalmind_buffer::ByteOffset;
use metalmind_vim::{Key, Mode};
use pretty_assertions::assert_eq;

use crate::workspace::PaneId;
use crate::workspace::test_session::Session;

#[test]
fn copiar_en_v_incluye_el_ultimo_caracter() {
    let mut session = Session::new("hola mundo\nadiós\n");
    session.keys("wve");
    session.copy();
    assert_eq!(session.clipboard.0.as_deref(), Some("mundo"));
    assert_eq!(session.mode(), Mode::Normal, "como \"+y, sale de Visual");
    assert_eq!(session.cursor(), ByteOffset::new(5));
    session.keys("v");
    session.copy();
    assert_eq!(
        session.clipboard.0.as_deref(),
        Some("m"),
        "v solo copia un carácter"
    );
}

#[test]
fn copiar_en_mayuscula_v_copia_lineas_completas() {
    let mut session = Session::new("hola mundo\nadiós\nfin");
    session.keys("wVj");
    session.copy();
    assert_eq!(session.clipboard.0.as_deref(), Some("hola mundo\nadiós\n"));
    session.keys("GV");
    session.copy();
    assert_eq!(session.clipboard.0.as_deref(), Some("fin\n"));
}

#[test]
fn copiar_fuera_de_visual_copia_el_raton_o_la_linea() {
    let mut session = Session::new("hola mundo\nadiós\n");
    session.copy();
    assert_eq!(session.clipboard.0.as_deref(), Some("hola mundo\n"));
    session.select(ByteOffset::new(5), ByteOffset::new(10));
    session.copy();
    assert_eq!(session.clipboard.0.as_deref(), Some("mundo"));
}

#[test]
fn pegar_en_visual_reemplaza_la_seleccion_sin_tocar_los_registros() {
    let mut session = Session::new("hola mundo\n");
    session.keys("yiw");
    session.clipboard.0 = Some("gente".to_owned());
    session.keys("wve");
    session.paste_clipboard();
    assert_eq!(session.text(), "hola gente\n");
    assert_eq!(session.mode(), Mode::Normal);
    session.keys("0P");
    assert_eq!(
        session.text(),
        "holahola gente\n",
        "\"\" sigue con lo copiado"
    );
}

#[test]
fn pegar_lineas_en_mayuscula_v_reemplaza_las_lineas() {
    let mut session = Session::new("uno\ndos\ntres\n");
    session.clipboard.0 = Some("nuevo\n".to_owned());
    session.keys("jV");
    session.paste_clipboard();
    assert_eq!(session.text(), "uno\nnuevo\ntres\n");
}

#[test]
fn pegar_fuera_de_visual_pega_en_el_cursor() {
    let mut session = Session::new("ab\n");
    session.clipboard.0 = Some("X".to_owned());
    session.paste_clipboard();
    assert_eq!(session.text(), "Xab\n");
}

fn highlighted(session: &Session, pane: PaneId) -> Option<String> {
    session
        .view(pane)
        .search
        .map(|pattern| pattern.source().to_owned())
}

#[test]
fn noh_apaga_el_resaltado_y_una_busqueda_lo_enciende() {
    let mut session = Session::new("uno\ndos\nuno\n");
    let left = session.pane();
    assert_eq!(highlighted(&session, left), None);
    session.keys("/uno⏎⌃wv");
    let right = session.pane();
    assert_eq!(highlighted(&session, right).as_deref(), Some("uno"));
    assert_eq!(
        highlighted(&session, left).as_deref(),
        Some("uno"),
        "en todos los paneles"
    );
    session.keys(":noh⏎");
    assert_eq!(highlighted(&session, right), None);
    assert_eq!(highlighted(&session, left), None);
    session.keys("n");
    assert_eq!(highlighted(&session, right).as_deref(), Some("uno"));
    session.keys(":nohlsearch⏎?dos⏎");
    assert_eq!(highlighted(&session, right).as_deref(), Some("dos"));
}

#[test]
fn las_flechas_recorren_el_historial_de_la_busqueda() {
    let mut session = Session::new("uno\ndos\n");
    session.keys("/uno⏎/dos⏎/");
    let pane = session.pane();
    session
        .workspace
        .feed(pane, Key::Up, &mut session.clipboard)
        .unwrap();
    assert_eq!(session.command_line(), Some("dos"));
    session
        .workspace
        .feed(pane, Key::Up, &mut session.clipboard)
        .unwrap();
    assert_eq!(session.command_line(), Some("uno"));
    let line = session.view(pane).command_line.unwrap();
    assert_eq!(line.prompt.symbol(), '/');
}

#[test]
fn copiar_y_pegar_en_visual_no_completan_lo_pendiente() {
    let mut session = Session::new("hola mundo\n");
    session.keys("wver");
    session.copy();
    assert_eq!(session.clipboard.0.as_deref(), Some("mundo"));
    assert_eq!(session.text(), "hola mundo\n", "r no reemplazó por \"");
    session.clipboard.0 = Some("gente".to_owned());
    session.keys("ve3");
    session.paste_clipboard();
    assert_eq!(
        session.text(),
        "hola gente\n",
        "el count no repitió el pegado"
    );
}

#[test]
fn copiar_fuera_de_visual_no_mueve_el_cursor() {
    let mut session = Session::new("uno\ndos\ntres\n");
    session.keys("jl");
    session.copy();
    assert_eq!(session.cursor(), ByteOffset::new(5));
    assert_eq!(session.mode(), Mode::Normal);
}

/// Lo que hace la paleta con una acción del motor: lo mismo que su atajo.
#[test]
fn execute_corre_una_accion_del_motor_como_su_atajo() {
    use metalmind_vim::{Action, KeyOutcome, Motion};

    let mut session = Session::new("uno\ndos\ntres");
    let pane = session.pane();
    let outcome = session
        .workspace
        .execute(
            pane,
            Action::Motion(Motion::DocumentEnd),
            &mut session.clipboard,
        )
        .unwrap();
    assert_eq!(
        outcome,
        KeyOutcome::Executed(Action::Motion(Motion::DocumentEnd))
    );
    assert_eq!(session.line(pane), 3);
    let outcome = session
        .workspace
        .execute(
            pane,
            Action::Command(metalmind_vim::Command::SplitRight),
            &mut session.clipboard,
        )
        .unwrap();
    assert!(matches!(outcome, KeyOutcome::Command(_)));
}

#[test]
fn el_tabstop_de_la_config_mide_la_columna_de_j() {
    let text = "a\tbc\nxyzwvutsrqp";
    let mut cuatro = Session::with_tabstop(text, 4);
    cuatro.keys("fcj");
    let mut ocho = Session::with_tabstop(text, 8);
    ocho.keys("fcj");
    let column = |session: &Session| session.cursor().get() - "a\tbc\n".len();
    assert_eq!((column(&cuatro), column(&ocho)), (5, 9));
}

#[test]
fn un_clic_en_la_casilla_la_marca_y_desmarca() {
    let text = "- [ ] uno\n- [x] dos\nfin";
    let mut session = Session::new(text);
    session.keys("0");
    session.click(ByteOffset::new(3));
    assert_eq!(session.text(), "- [x] uno\n- [x] dos\nfin");
    session.click(ByteOffset::new(12));
    assert_eq!(session.text(), "- [x] uno\n- [ ] dos\nfin");
    session.keys("u");
    assert_eq!(session.text(), "- [x] uno\n- [x] dos\nfin");
}

#[test]
fn un_clic_fuera_de_la_casilla_solo_mueve_el_cursor() {
    let text = "- [ ] uno\n```\n- [ ] código\n```";
    let mut session = Session::new(text);
    session.keys("0");
    for offset in [7, 18] {
        session.click(ByteOffset::new(offset));
        assert_eq!(session.text(), text);
        assert_eq!(session.cursor(), ByteOffset::new(offset));
    }
}

#[test]
fn en_insert_un_clic_en_la_casilla_no_la_cambia() {
    let mut session = Session::new("- [ ] uno");
    session.keys("A");
    session.click(ByteOffset::new(3));
    assert_eq!(session.text(), "- [ ] uno");
    assert_eq!(session.mode(), Mode::Insert);
}

#[test]
fn en_visual_un_clic_en_la_casilla_solo_sale_de_visual() {
    let mut session = Session::new("- [ ] uno\ndos");
    session.keys("jv");
    session.click(ByteOffset::new(3));
    assert_eq!(session.text(), "- [ ] uno\ndos");
    assert_eq!(session.mode(), Mode::Normal);
}

#[test]
fn la_casilla_de_una_cita_se_marca() {
    let mut session = Session::new("> - [ ] uno");
    session.keys("0");
    session.click(ByteOffset::new(5));
    assert_eq!(session.text(), "> - [x] uno");
}
