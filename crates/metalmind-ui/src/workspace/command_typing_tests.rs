//! Lo que se escribe tras `:`, el nombre de un comando o la palabra de su
//! argumento (#156), y aceptar su autocompletado.

use pretty_assertions::assert_eq;

use crate::completion::CommandCompletions;

use crate::workspace::test_session::Session;

fn typed(session: &Session) -> Option<String> {
    session
        .workspace
        .typed_argument(session.pane())
        .map(|typed| typed.word.text)
}

#[test]
fn solo_en_la_linea_de_comandos_y_en_el_panel_con_el_foco() {
    let mut session = Session::new("hola\n");
    session.keys(":focus #r");
    assert_eq!(typed(&session), Some("#r".to_owned()));
    session.keys("⎋");
    assert_eq!(typed(&session), None);
    session.keys("A:focus #r");
    assert_eq!(typed(&session), None, "en Insert es texto");
}

#[test]
fn aceptar_cambia_la_palabra_y_se_puede_seguir_escribiendo() {
    let mut session = Session::new("hola\n");
    session.keys(r":focus Mis\ n");
    let pane = session.pane();
    session.workspace.complete_argument(pane, r"Mis\ notas/");
    session.keys(" #r");
    assert_eq!(session.command_line(), Some(r"focus Mis\ notas/ #r"));
    session.workspace.complete_argument(pane, "#Rust");
    assert_eq!(session.command_line(), Some(r"focus Mis\ notas/ #Rust"));
}

#[test]
fn el_nombre_del_comando_solo_en_el_panel_con_el_foco() {
    let mut session = Session::new("hola\n");
    session.keys(":foc");
    let pane = session.pane();
    assert!(session.workspace.typed_command(pane).is_some());
    session.workspace.complete_command(pane, "focus ");
    assert_eq!(session.command_line(), Some("focus "));
    assert_eq!(
        session.workspace.typed_command(pane),
        None,
        "ya es el argumento"
    );
    session.keys("⎋A:foc");
    assert_eq!(
        session.workspace.typed_command(pane),
        None,
        "en Insert es texto"
    );
}

fn command_list(session: &Session) -> Option<CommandCompletions> {
    let typed = session.workspace.typed_command(session.pane())?;
    Some(CommandCompletions::open(typed, None))
}

fn rows(list: &CommandCompletions) -> Vec<(String, Option<&'static str>)> {
    (0..list.len())
        .filter_map(|row| list.row(row))
        .map(|row| (row.text.to_owned(), row.action))
        .collect()
}

#[test]
fn la_lista_del_nombre_trae_la_accion_y_aceptar_deja_el_espacio_del_argumento() {
    let mut session = Session::new("hola\n");
    session.keys(":foc");
    let list = command_list(&session).unwrap();
    assert_eq!(
        rows(&list),
        [("focus".to_owned(), Some("workspace::focus"))]
    );
    let pane = session.pane();
    session.workspace.complete_command(pane, &list.accept());
    assert_eq!(session.command_line(), Some("focus "));
}

#[test]
fn con_la_linea_vacia_la_lista_es_el_historial_y_aceptar_escribe_la_linea() {
    let mut session = Session::new("hola\n");
    session.keys(":noh⏎:2⏎:");
    let mut list = command_list(&session).unwrap();
    assert_eq!(
        rows(&list),
        [("2".to_owned(), None), ("noh".to_owned(), None)]
    );
    list.selection_mut().select(1);
    let pane = session.pane();
    session.workspace.complete_command(pane, &list.accept());
    assert_eq!(session.command_line(), Some("noh"));
}
