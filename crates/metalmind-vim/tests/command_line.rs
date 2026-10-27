#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "en tests, un unwrap que falla es un test que falla"
)]

//! Lo que devuelve la línea de comandos al enviarla. Lo que pasa con el
//! texto y el modo mientras se escribe está en la tabla de `vim.rs`, que
//! también corre contra nvim.

#[allow(
    dead_code,
    reason = "cada binario de test usa solo una parte del soporte compartido"
)]
mod support;

use metalmind_keymap::Command;
use metalmind_vim::{
    Action, ActionName, CommandError, Invocation, KeyOutcome, Mode, Vim, VisualKind,
};
use pretty_assertions::assert_eq;
use rstest::rstest;

/// Corre `keys` y devuelve lo que pasó con la última tecla.
fn submit(keys: &str) -> (KeyOutcome, Vim, String) {
    let mut editor = support::Editor::app("ab|c");
    let last = editor.keys(keys);
    let text = support::render(&editor.buffer, [editor.selections.primary().head]);
    (last.unwrap(), editor.vim, text)
}

#[rstest]
#[case(":w<CR>", Command::Save)]
#[case(":write<CR>", Command::Save)]
#[case(":q<CR>", Command::Quit)]
#[case(":q!<CR>", Command::ForceQuit)]
#[case(":wq<CR>", Command::SaveAndQuit)]
#[case(":x<BS>wq<CR>", Command::SaveAndQuit)]
#[case("3: w <CR>", Command::Save)]
#[case("ihola<Esc>:w<CR>", Command::Save)]
#[case(":e<CR>", Command::Edit)]
#[case(":e!<CR>", Command::ForceEdit)]
#[case(":vs<CR>", Command::SplitRight)]
#[case(":sp<CR>", Command::SplitDown)]
#[case(":on<CR>", Command::Only)]
#[case(":set<CR>", Command::Set)]
#[case(":tagundo<CR>", Command::UndoTagChange)]
#[case(":template<CR>", Command::NewNoteFromTemplate)]
#[case(":today<CR>", Command::OpenDailyNote)]
#[case(":tagpage<CR>", Command::OpenTagPage)]
#[case(":focus<CR>", Command::Focus)]
#[case(":unfocus<CR>", Command::ClearFocus)]
#[case("<C-w>v", Command::SplitRight)]
#[case("<C-w><C-v>", Command::SplitRight)]
#[case("<C-w>s", Command::SplitDown)]
#[case("<C-w>h", Command::FocusLeft)]
#[case("<C-w>j", Command::FocusDown)]
#[case("<C-w>k", Command::FocusUp)]
#[case("<C-w>l", Command::FocusRight)]
#[case("<C-w>w", Command::FocusNext)]
#[case("<C-w><C-w>", Command::FocusNext)]
#[case("<C-w>q", Command::Quit)]
#[case("<C-w>o", Command::Only)]
#[case("3<C-w>v", Command::SplitRight)]
#[case(" f", Command::OpenFinder)]
#[case(" /", Command::OpenTextSearch)]
#[case(" p", Command::OpenCommandPalette)]
#[case(" e", Command::ToggleFolders)]
#[case(" t", Command::ToggleTags)]
#[case(" b", Command::ToggleBacklinks)]
#[case(" u", Command::OpenUnresolvedLinks)]
#[case(" n", Command::NewNoteFromTemplate)]
#[case(" d", Command::OpenDailyNote)]
#[case("gd", Command::FollowLink)]
#[case("<CR>", Command::FollowLink)]
fn envia_el_comando(#[case] keys: &str, #[case] command: Command) {
    let (outcome, vim, _) = submit(keys);
    assert_eq!(outcome, support::dispatched(command));
    assert_eq!(vim.mode(), Mode::Normal);
    assert_eq!(vim.command_line(), None);
}

#[test]
fn e_lleva_la_ruta() {
    let (outcome, _, text) = submit(":e notas/otra nota.md<CR>");
    assert_eq!(
        outcome,
        KeyOutcome::Dispatch(Invocation {
            action: ActionName::new(Command::Edit.name()),
            argument: Some("notas/otra nota.md".to_owned()),
        })
    );
    assert_eq!(text, "ab|c");
}

#[rstest]
#[case(":tagrename #rust #Rust<CR>", Command::RenameTag, "#rust #Rust")]
#[case(":tagmerge #rs #rust<CR>", Command::MergeTag, "#rs #rust")]
#[case(
    ":template entrevista Backend senior<CR>",
    Command::NewNoteFromTemplate,
    "entrevista Backend senior"
)]
#[case(":focus proyectos/ #rust<CR>", Command::Focus, "proyectos/ #rust")]
fn llevan_lo_escrito_tras_el_nombre(
    #[case] keys: &str,
    #[case] command: Command,
    #[case] argument: &str,
) {
    let (outcome, _, text) = submit(keys);
    assert_eq!(
        outcome,
        KeyOutcome::Dispatch(Invocation {
            action: ActionName::new(command.name()),
            argument: Some(argument.to_owned()),
        })
    );
    assert_eq!(text, "ab|c");
}

/// Como nvim: `d<C-w>` cancela el `d` y la `v` que sigue ya es Visual.
#[test]
fn ctrl_w_cancela_un_operador_pendiente() {
    let (outcome, vim, text) = submit("d<C-w>");
    assert_eq!(outcome, KeyOutcome::Unbound);
    assert_eq!(vim.mode(), Mode::Normal);
    assert_eq!(text, "ab|c");
    let (_, vim, _) = submit("d<C-w>v");
    assert_eq!(vim.mode(), Mode::Visual(VisualKind::Char));
}

#[test]
fn el_leader_solo_es_de_normal() {
    let (outcome, vim, _) = submit("v ");
    assert_eq!(outcome, KeyOutcome::Unbound);
    assert_eq!(vim.mode(), Mode::Visual(VisualKind::Char));
}

#[test]
fn ctrl_w_seguido_de_otra_tecla_no_hace_nada() {
    let (outcome, vim, _) = submit("<C-w>x");
    assert_eq!(outcome, KeyOutcome::Unbound);
    assert_eq!(vim.mode(), Mode::Normal);
    let (_, _, text) = submit("<C-w>xh");
    assert_eq!(text, "a|bc");
}

#[test]
fn un_comando_desconocido_no_toca_el_texto() {
    let (outcome, vim, text) = submit(":dd<CR>");
    assert_eq!(
        outcome,
        KeyOutcome::InvalidCommand(CommandError::Unknown("dd".to_owned()))
    );
    assert_eq!(vim.mode(), Mode::Normal);
    assert_eq!(text, "ab|c");
}

#[test]
fn un_argumento_de_mas_no_toca_el_texto() {
    let (outcome, vim, text) = submit(":q otra.md<CR>");
    assert_eq!(
        outcome,
        KeyOutcome::InvalidCommand(CommandError::TrailingCharacters("otra.md".to_owned()))
    );
    assert_eq!(vim.mode(), Mode::Normal);
    assert_eq!(text, "ab|c");
}

#[test]
fn enviar_la_linea_vacia_no_hace_nada() {
    let (outcome, vim, text) = submit(":<CR>");
    assert_eq!(outcome, KeyOutcome::Executed(Action::SubmitCommand));
    assert_eq!(vim.mode(), Mode::Normal);
    assert_eq!(text, "ab|c");
}

#[test]
fn muestra_lo_escrito_mientras_se_escribe() {
    let (outcome, vim, _) = submit(":wq<BS>");
    assert_eq!(outcome, KeyOutcome::Executed(Action::Backspace));
    assert_eq!(vim.mode(), Mode::CommandLine);
    assert_eq!(vim.command_line(), Some("w"));
    let (outcome, vim, _) = submit(":w");
    assert_eq!(outcome, KeyOutcome::Inserted);
    assert_eq!(vim.command_line(), Some("w"));
}

#[test]
fn volver_a_abrirla_empieza_vacia() {
    let (_, vim, _) = submit(":wq<Esc>:");
    assert_eq!(vim.command_line(), Some(""));
}

#[test]
fn los_digitos_son_texto_y_no_un_count() {
    let (_, vim, _) = submit(":12");
    assert_eq!(vim.command_line(), Some("12"));
}

#[test]
fn tab_no_se_escribe_en_la_linea_de_comandos() {
    let (outcome, vim, _) = submit(":w<Tab>");
    assert_eq!(outcome, KeyOutcome::Unbound);
    assert_eq!(vim.command_line(), Some("w"));
}
