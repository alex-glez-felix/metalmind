use std::fs;

use nota_vim::{Command, Key, KeyOutcome};
use pretty_assertions::assert_eq;
use tempfile::TempDir;

use super::test_session::Session;
use super::*;
use crate::document::FileStats;

#[test]
fn el_texto_compuesto_entra_por_el_motor() {
    let mut session = Session::new("hola\n");
    session.keys("A");
    assert_eq!(session.type_text("ñá"), Some(KeyOutcome::Inserted));
    session.keys("⎋");
    assert_eq!(session.text(), "holañá\n");
    assert_eq!(session.mode(), Mode::Normal);
}

#[test]
fn en_normal_el_texto_son_comandos() {
    let mut session = Session::new("uno dos\n");
    assert!(matches!(
        session.type_text("w"),
        Some(KeyOutcome::Executed(_))
    ));
    assert_eq!(session.cursor(), ByteOffset::new(4));
}

#[test]
fn pegar_en_normal_es_un_paso_de_undo() {
    let mut session = Session::new("ab\n");
    session.paste("XY");
    assert_eq!(session.text(), "XYab\n");
    assert_eq!(session.cursor(), ByteOffset::new(1));
    session.keys("u");
    assert_eq!(session.text(), "ab\n");
}

#[test]
fn pegar_en_normal_cancela_un_f_pendiente() {
    let mut session = Session::new("ab\n");
    session.keys("f");
    session.paste("XY");
    session.keys("x");
    assert_eq!(session.text(), "Xab\n");
}

#[test]
fn un_clic_cancela_un_f_pendiente() {
    let mut session = Session::new("abc def\n");
    session.keys("f");
    session.select(ByteOffset::new(4), ByteOffset::new(4));
    session.keys("x");
    assert_eq!(session.text(), "abc ef\n");
}

#[test]
fn pegar_en_insert_se_deshace_con_lo_escrito() {
    let mut session = Session::new("ab\n");
    session.keys("iz");
    session.paste("ñ");
    session.keys("q⎋");
    assert_eq!(session.text(), "zñqab\n");
    session.keys("u");
    assert_eq!(session.text(), "ab\n");
}

#[test]
fn lo_pegado_en_insert_se_repite_con_el_count() {
    let mut session = Session::new("bc\n");
    session.keys("3ia");
    session.paste("XY");
    session.keys("⎋");
    assert_eq!(session.text(), "aXYaXYaXYbc\n");
    assert_eq!(session.cursor(), ByteOffset::new(8));
}

#[test]
fn un_clic_en_insert_descarta_la_repeticion_del_count() {
    let mut session = Session::new("abc def\n");
    session.keys("3ihola");
    session.select(ByteOffset::new(10), ByteOffset::new(10));
    session.keys("x⎋");
    assert_eq!(session.text(), "holaabc dexf\n");
    assert_eq!(session.cursor(), ByteOffset::new(10));
}

#[test]
fn pegar_en_la_linea_de_comandos_no_toca_el_texto() {
    let mut session = Session::new("ab\n");
    session.keys(":");
    session.paste("wq");
    assert_eq!(session.command_line(), Some("wq"));
    assert_eq!(session.text(), "ab\n");
    assert!(!session.is_modified());
}

#[test]
fn seleccion_con_raton_y_copia() {
    let mut session = Session::new("hola mundo\nadiós\n");
    assert_eq!(session.selected_text(), None);
    session.select(ByteOffset::new(5), ByteOffset::new(10));
    assert_eq!(session.selected_text().as_deref(), Some("mundo"));
    session.select(ByteOffset::new(11), ByteOffset::new(11));
    assert_eq!(session.current_line(), "adiós\n");
}

#[test]
fn en_normal_el_cursor_no_queda_sobre_el_salto() {
    let mut session = Session::new("abc\n\nxyz\n");
    session.select(ByteOffset::new(3), ByteOffset::new(3));
    assert_eq!(session.cursor(), ByteOffset::new(2));
    session.select(ByteOffset::new(4), ByteOffset::new(4));
    assert_eq!(session.cursor(), ByteOffset::new(4));
    session.keys("A");
    session.select(ByteOffset::new(3), ByteOffset::new(3));
    assert_eq!(session.cursor(), ByteOffset::new(3));
}

#[test]
fn dos_puntos_w_pide_guardar() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("nota.md");
    let mut session = Session::open(&path);
    assert_eq!(
        session.keys("iab⎋:w⏎"),
        Some(KeyOutcome::Command(Command::Save.into()))
    );
    assert_eq!(session.mode(), Mode::Normal);
    assert_eq!(fs::read_to_string(&path).unwrap(), "ab");
    assert_eq!(
        session.keys(":wq⏎"),
        Some(KeyOutcome::Command(Command::SaveAndQuit.into()))
    );
    assert_eq!(session.effect, Effect::CloseWindow);
    assert_eq!(
        session.keys(":qx⌫!⏎"),
        Some(KeyOutcome::Command(Command::ForceQuit.into()))
    );
}

#[test]
fn editar_marca_modificado_y_guardar_lo_limpia() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("nota.md");
    fs::write(&path, "uno\n").unwrap();
    let mut session = Session::open(&path);
    session.keys("Adós⎋");
    assert!(session.is_modified());
    assert_eq!(fs::read_to_string(&path).unwrap(), "uno\n");
    let stats = session.save().unwrap();
    assert!(!session.is_modified());
    assert_eq!(stats, FileStats { lines: 2, bytes: 8 });
    assert_eq!(fs::read_to_string(&path).unwrap(), "unodós\n");
}

#[test]
fn guardar_crea_el_archivo_nuevo() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("nueva.md");
    let mut session = Session::open(&path);
    session.keys("ihola⎋");
    session.save().unwrap();
    assert!(session.exists_on_disk());
    assert_eq!(fs::read_to_string(&path).unwrap(), "hola");
}

#[test]
fn deshacer_tras_guardar_vuelve_a_marcar_modificado() {
    let dir = TempDir::new().unwrap();
    let mut session = Session::open(&dir.path().join("nota.md"));
    session.keys("ix⎋");
    session.save().unwrap();
    session.keys("u");
    assert!(session.is_modified());
}

#[test]
fn si_guardar_falla_sigue_modificado() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("no-existe").join("nota.md");
    let mut session = Session::open(&path);
    session.keys("ix⎋");
    assert!(session.save().is_err());
    assert!(session.is_modified());
    assert!(!session.exists_on_disk());
}

#[test]
fn deshacer_hasta_lo_guardado_quita_el_modificado() {
    let mut session = Session::new("abc\n");
    session.keys("x");
    assert!(session.is_modified());
    session.keys("xuu");
    assert_eq!(session.text(), "abc\n");
    assert!(!session.is_modified());
}

#[test]
fn escribir_en_insert_marca_modificado_antes_de_salir() {
    let mut session = Session::new("abc\n");
    session.keys("i");
    assert!(!session.is_modified());
    session.keys("x");
    assert!(session.is_modified());
    session.keys("⎋u");
    assert!(!session.is_modified());
}

#[test]
fn g_menos_y_g_mas_cruzan_ramas_hasta_lo_guardado() {
    let dir = TempDir::new().unwrap();
    let mut session = Session::open(&dir.path().join("nota.md"));
    session.keys("iabc⎋0x");
    session.save().unwrap();
    session.keys("u$x");
    assert_eq!(session.text(), "ab");
    assert!(session.is_modified());
    session.keys("g-");
    assert_eq!(session.text(), "bc");
    assert!(!session.is_modified());
    session.keys("g+");
    assert!(session.is_modified());
    session.keys("g-");
    assert!(!session.is_modified());
}

#[test]
fn el_mismo_texto_por_otra_edicion_sigue_modificado() {
    let mut session = Session::new("abc\n");
    session.keys("xia⎋");
    assert_eq!(session.text(), "abc\n");
    assert!(session.is_modified());
}

#[test]
fn guardar_a_mitad_del_insert_no_coincide_con_ningun_estado() {
    let dir = TempDir::new().unwrap();
    let mut session = Session::open(&dir.path().join("nota.md"));
    session.keys("ix");
    session.save().unwrap();
    assert!(session.is_modified());
    session.keys("⎋u");
    assert_eq!(session.text(), "");
    assert!(session.is_modified());
}

#[test]
fn dos_paneles_del_mismo_documento_ven_las_mismas_ediciones() {
    let mut session = Session::new("uno\ndos\n");
    let left = session.pane();
    session.keys("⌃wv");
    let right = session.pane();
    assert_ne!(left, right);
    session.keys("jAy⎋");
    assert_eq!(session.view(left).buffer().to_string(), "uno\ndosy\n");
    assert_eq!(session.view(left).cursor(), ByteOffset::ZERO);
    assert_eq!(session.view(right).cursor(), ByteOffset::new(7));
}

#[test]
fn el_cursor_del_otro_panel_sigue_a_su_texto() {
    let mut session = Session::new("uno\ndos\n");
    let left = session.pane();
    session.keys("j⌃wv");
    session.keys("ggOcero⎋");
    assert_eq!(session.text(), "cero\nuno\ndos\n");
    assert_eq!(session.view(left).cursor(), ByteOffset::new(9));
    session.keys("u");
    assert_eq!(session.view(left).cursor(), ByteOffset::new(4));
    session.keys("jdd");
    assert_eq!(session.text(), "uno\n");
    assert_eq!(session.view(left).cursor(), ByteOffset::new(4));
}

#[test]
fn cambiar_de_panel_cierra_el_insert() {
    let mut session = Session::new("abc\n");
    let left = session.pane();
    session.keys("⌃wv");
    session.keys("Axy");
    assert_eq!(session.mode(), Mode::Insert);
    session.workspace.focus(left).unwrap();
    assert_eq!(session.mode(), Mode::Normal);
    session.keys("u");
    assert_eq!(session.text(), "abc\n");
}

#[test]
fn escribir_en_otro_panel_le_da_el_foco() {
    let mut session = Session::new("abc\n");
    let left = session.pane();
    session.keys("⌃wv");
    session.keys("i");
    session
        .workspace
        .feed(left, Key::Char('x'), &mut session.clipboard)
        .unwrap();
    assert_eq!(session.pane(), left);
    assert_eq!(session.mode(), Mode::Normal);
    assert_eq!(session.text(), "bc\n");
}

#[test]
fn solo_el_panel_enfocado_muestra_el_modo() {
    let mut session = Session::new("abc\n");
    let left = session.pane();
    session.keys("⌃wv:");
    assert_eq!(session.command_line(), Some(""));
    assert_eq!(session.view(left).mode, Mode::Normal);
    assert_eq!(session.view(left).command_line, None);
    assert!(!session.view(left).focused);
}

#[test]
fn lo_copiado_en_un_documento_se_pega_en_otro() {
    let dir = TempDir::new().unwrap();
    fs::write(dir.path().join("b.md"), "b\n").unwrap();
    let mut session = Session::open(&dir.path().join("a.md"));
    session.paste("hola");
    session.keys("yiw");
    session.keys(&format!(":e! {}⏎", dir.path().join("b.md").display()));
    session.keys("P");
    assert_eq!(session.text(), "holab\n");
}

#[test]
fn los_registros_del_portapapeles_usan_el_que_le_pasan() {
    let mut session = Session::new("uno\ndos");
    session.keys("\"+yyj");
    assert_eq!(session.clipboard.0.as_deref(), Some("uno\n"));
    session.clipboard.0 = Some("tres".to_owned());
    session.keys("\"+P");
    assert_eq!(session.text(), "uno\ntresdos");
}

#[test]
fn solo_el_panel_con_el_foco_dice_que_se_graba_una_macro() {
    let mut session = Session::new("uno");
    let left = session.pane();
    session.keys("qb");
    assert_eq!(session.view(left).recording, Some('b'));
    session.keys("⌃wv");
    let right = session.pane();
    assert_eq!(session.view(right).recording, Some('b'));
    assert_eq!(session.view(left).recording, None);
    session.keys("q");
    assert_eq!(session.view(right).recording, None);
}

#[test]
fn la_busqueda_se_escribe_tras_su_simbolo_y_avisa_en_la_barra() {
    let mut session = Session::new("uno\ndos\nuno\n");
    session.keys("?un");
    let line = session.view(session.pane()).command_line.unwrap();
    assert_eq!((line.prompt.symbol(), line.typed), ('?', "un"));
    session.keys("⎋/nada⏎");
    assert_eq!(
        session.workspace.message(),
        Some(&Message::Error(
            "E486: no se encontró el patrón: nada".to_owned()
        ))
    );
    session.keys("G/uno⏎");
    assert!(matches!(
        session.workspace.message(),
        Some(Message::Info(_))
    ));
    assert_eq!(session.cursor(), ByteOffset::ZERO);
    session.keys("n");
    assert_eq!(session.workspace.message(), None);
    assert_eq!(session.cursor(), ByteOffset::new(8));
}

#[test]
fn una_macro_que_cambia_de_panel_edita_en_el_nuevo() {
    let mut session = Session::new("abc\n");
    let left = session.pane();
    session.keys("⌃wv");
    let right = session.pane();
    session.keys("qa⌃whxq");
    assert_eq!((session.pane(), session.text()), (left, "bc\n".to_owned()));
    session.keys("⌃wl$");
    assert_eq!(session.pane(), right);
    session.keys("@a");
    assert_eq!(session.pane(), left);
    assert_eq!(session.text(), "c\n");
    assert_eq!(session.view(left).cursor(), ByteOffset::ZERO);
}

#[test]
fn un_comando_que_falla_corta_la_macro() {
    let mut session = Session::new("abc\n");
    session.keys("xqa:e otro.md⏎xq");
    assert_eq!(session.text(), "c\n");
    session.keys("@a");
    assert!(matches!(
        session.workspace.message(),
        Some(Message::Error(_))
    ));
    assert_eq!(session.text(), "c\n");
}

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
