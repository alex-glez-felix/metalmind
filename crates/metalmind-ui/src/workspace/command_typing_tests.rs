//! La palabra del argumento de `:` que se escribe y aceptar su
//! autocompletado (#156).

use pretty_assertions::assert_eq;

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
