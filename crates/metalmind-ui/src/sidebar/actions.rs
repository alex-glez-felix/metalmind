//! Las acciones del panel lateral y sus atajos. Las teclas, los botones, el
//! menú contextual y la paleta solo las invocan por su nombre.

use gpui_kit::{Action, App, KeyBinding};

use metalmind_keymap::Command;

use crate::actions::Dispatch;

/// Contexto de teclas del árbol con el foco.
pub const TREE_CONTEXT: &str = "FileTree";
/// Escribiendo un nombre o el filtro.
pub const INPUT_CONTEXT: &str = "FileTreeInput";
/// Confirmando un borrado.
pub const CONFIRM_CONTEXT: &str = "FileTreeConfirm";
/// Con el menú contextual abierto.
pub const MENU_CONTEXT: &str = "FileTreeMenu";

macro_rules! panel_actions {
    ($($(#[doc = $doc:literal])* $action:ident = $namespace:ident :: $name:literal;)*) => {
        $(
            $(#[doc = $doc])*
            #[derive(Clone, PartialEq, Eq, Default, Debug, Action)]
            #[action(namespace = $namespace, name = $name)]
            pub struct $action;
        )*
    };
}

panel_actions! {
    /// Cambia entre las vistas de carpetas y de tags.
    ToggleView = panel::"toggle_view";
    Hide = panel::"hide";
    /// Le devuelve el foco al panel de edición.
    FocusEditor = panel::"focus_editor";
    SelectNext = file_tree::"select_next";
    SelectPrevious = file_tree::"select_previous";
    SelectFirst = file_tree::"select_first";
    SelectLast = file_tree::"select_last";
    /// Cierra la carpeta o sube a la que la contiene.
    CollapseOrParent = file_tree::"collapse_or_parent";
    /// Abre la carpeta, baja a su primer hijo o abre el archivo.
    ExpandOrOpen = file_tree::"expand_or_open";
    /// Abre el archivo en el panel de edición, o abre y cierra la carpeta.
    Open = file_tree::"open";
    OpenSplitRight = file_tree::"open_split_right";
    OpenSplitDown = file_tree::"open_split_down";
    Filter = file_tree::"filter";
    NewNote = file_tree::"new_note";
    NewFolder = file_tree::"new_folder";
    Rename = file_tree::"rename";
    Move = file_tree::"move";
    /// Manda a la papelera del vault, tras confirmar.
    Delete = file_tree::"delete";
    /// Acepta lo escrito o el borrado.
    Confirm = file_tree::"confirm";
    /// Cierra la pregunta o el menú; en el árbol, quita el filtro.
    Cancel = file_tree::"cancel";
    MenuNext = menu::"select_next";
    MenuPrevious = menu::"select_previous";
}

pub fn bind_keys(cx: &mut App) {
    let tree = Some(TREE_CONTEXT);
    let input = Some(INPUT_CONTEXT);
    let confirm = Some(CONFIRM_CONTEXT);
    let menu = Some(MENU_CONTEXT);
    cx.bind_keys([
        KeyBinding::new("j", SelectNext, tree),
        KeyBinding::new("down", SelectNext, tree),
        KeyBinding::new("k", SelectPrevious, tree),
        KeyBinding::new("up", SelectPrevious, tree),
        KeyBinding::new("g g", SelectFirst, tree),
        KeyBinding::new("shift-g", SelectLast, tree),
        KeyBinding::new("h", CollapseOrParent, tree),
        KeyBinding::new("left", CollapseOrParent, tree),
        KeyBinding::new("l", ExpandOrOpen, tree),
        KeyBinding::new("right", ExpandOrOpen, tree),
        KeyBinding::new("o", Open, tree),
        KeyBinding::new("enter", Open, tree),
        KeyBinding::new("ctrl-v", OpenSplitRight, tree),
        KeyBinding::new("ctrl-s", OpenSplitDown, tree),
        KeyBinding::new("/", Filter, tree),
        KeyBinding::new("a", NewNote, tree),
        KeyBinding::new("shift-a", NewFolder, tree),
        KeyBinding::new("r", Rename, tree),
        KeyBinding::new("m", Move, tree),
        KeyBinding::new("d", Delete, tree),
        KeyBinding::new("shift-t", ToggleView, tree),
        KeyBinding::new("space e", Dispatch(Command::ToggleFolders.into()), tree),
        KeyBinding::new("space t", Dispatch(Command::ToggleTags.into()), tree),
        KeyBinding::new("q", Hide, tree),
        KeyBinding::new("ctrl-w l", FocusEditor, tree),
        KeyBinding::new("ctrl-w ctrl-l", FocusEditor, tree),
        KeyBinding::new("escape", Cancel, tree),
        KeyBinding::new("enter", Confirm, input),
        KeyBinding::new("escape", Cancel, input),
        KeyBinding::new("y", Confirm, confirm),
        KeyBinding::new("enter", Confirm, confirm),
        KeyBinding::new("n", Cancel, confirm),
        KeyBinding::new("escape", Cancel, confirm),
        KeyBinding::new("j", MenuNext, menu),
        KeyBinding::new("down", MenuNext, menu),
        KeyBinding::new("k", MenuPrevious, menu),
        KeyBinding::new("up", MenuPrevious, menu),
        KeyBinding::new("enter", Confirm, menu),
        KeyBinding::new("escape", Cancel, menu),
    ]);
}
