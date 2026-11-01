//! Los mensajes de vim que muestra Metalmind llevan el texto de nvim 0.12.6,
//! sin traducir: cada caso teclea lo mismo en los dos y compara el último.

use std::fs;

use pretty_assertions::assert_eq;
use tempfile::TempDir;

use super::Message;
use super::options::OptionError;
use super::panes_oracle;
use super::test_session::Session;

/// Teclas en la notación de nvim; las rutas van sin carpeta, y para
/// Metalmind se escriben enteras. `E162` lo compara `como_nvim_con_hidden`
/// con `v:errmsg`: con la UI que recibe los mensajes, nvim pide confirmar.
#[rstest::rstest]
#[case::abrir("")]
#[case::abrir_sin_salto_final(":e b.md<CR>")]
#[case::abrir_uno_nuevo(":e nuevo.md<CR>")]
#[case::w("ix<Esc>:w<CR>")]
#[case::busqueda_que_sigue_desde_arriba("/dos<CR>n")]
#[case::busqueda_que_sigue_desde_abajo("?uno<CR>")]
#[case::set_desconocida(":set foo<CR>")]
#[case::e_con_cambios("ix<Esc>:e<CR>")]
#[case::w_a_un_archivo_que_existe(":w b.md<CR>")]
#[case::w_a_un_buffer_abierto(":e nuevo.md<CR><C-w>v:e! a.md<CR>ix<Esc>:w nuevo.md<CR>")]
#[cfg_attr(not(nvim_available), ignore = "nvim no está en el PATH")]
fn el_mensaje_es_el_de_nvim(#[case] keys: &str) {
    let nvim = panes_oracle::shown(folder().path(), "a.md", keys);
    let dir = folder();
    let mut session = Session::open(&dir.path().join("a.md"));
    let typed = keys
        .replace("<Esc>", "⎋")
        .replace("<CR>", "⏎")
        .replace("<C-w>", "⌃w");
    let typed = ["a.md", "b.md", "nuevo.md"]
        .iter()
        .fold(typed, |typed, name| {
            typed.replace(
                &format!(" {name}"),
                &format!(" {}", dir.path().join(name).display()),
            )
        });
    session.keys(&typed);
    let message = session
        .workspace
        .messages()
        .last()
        .map(Message::text)
        .unwrap_or_default()
        .replace(&format!("{}/", dir.path().display()), "");
    assert_eq!(message, nvim.message);
}

/// Una para cada uno: los dos escriben en ella.
fn folder() -> TempDir {
    let dir = TempDir::new().unwrap();
    fs::write(dir.path().join("a.md"), "uno\ndos\n").unwrap();
    fs::write(dir.path().join("b.md"), "sin").unwrap();
    dir
}

/// nvim lista las opciones con `:set` a secas; Metalmind no tiene esa
/// lista y lo trata como un comando sin su argumento, con el texto que el
/// oráculo de `metalmind-vim` comprueba con `:norm`.
#[test]
fn set_sin_argumento_es_el_e471_de_vim() {
    assert_eq!(
        OptionError::MissingArgument.to_string(),
        metalmind_vim::CommandError::ArgumentRequired.to_string()
    );
}
