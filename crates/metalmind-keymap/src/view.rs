//! Las acciones que atiende una vista de la ventana, y los contextos de
//! teclas de cada vista.

use crate::named::named_actions;

named_actions! {
    /// Del editor, fuera del motor: el portapapeles del sistema.
    pub enum EditorAction {
        /// Copia la selección de Visual o del ratón, o la línea del cursor si
        /// no hay ninguna.
        Copy = "editor::copy",
        /// Pega en el cursor; en Visual, reemplaza la selección.
        Paste = "editor::paste",
        /// Cambia lo escrito del tag por el elegido en la lista del
        /// autocompletado de `#`.
        AcceptTagCompletion = "editor::accept_tag_completion",
        NextTagCompletion = "editor::next_tag_completion",
        PreviousTagCompletion = "editor::previous_tag_completion",
    }
}

named_actions! {
    /// Del panel lateral: el árbol, sus preguntas y su menú contextual.
    pub enum PanelAction {
        /// Cambia entre las vistas de carpetas y de tags.
        ToggleView = "panel::toggle_view",
        Hide = "panel::hide",
        /// Le devuelve el foco al panel de edición.
        FocusEditor = "panel::focus_editor",
        SelectNext = "file_tree::select_next",
        SelectPrevious = "file_tree::select_previous",
        SelectFirst = "file_tree::select_first",
        SelectLast = "file_tree::select_last",
        /// Cierra la carpeta o sube a la que la contiene.
        CollapseOrParent = "file_tree::collapse_or_parent",
        /// Abre la carpeta, baja a su primer hijo o abre el archivo.
        ExpandOrOpen = "file_tree::expand_or_open",
        /// Abre el archivo en el panel de edición, o abre y cierra la
        /// carpeta.
        Open = "file_tree::open",
        OpenSplitRight = "file_tree::open_split_right",
        OpenSplitDown = "file_tree::open_split_down",
        Filter = "file_tree::filter",
        NewNote = "file_tree::new_note",
        NewFolder = "file_tree::new_folder",
        Rename = "file_tree::rename",
        Move = "file_tree::move",
        /// En la vista de tags, preguntan el campo de la página del tag
        /// elegido con lo que tiene ahora, y la abren.
        EditTagAliases = "file_tree::edit_tag_aliases",
        EditTagParents = "file_tree::edit_tag_parents",
        SetTagIcon = "file_tree::set_tag_icon",
        SetTagTemplate = "file_tree::set_tag_template",
        OpenTagPage = "file_tree::open_tag_page",
        /// Manda a la papelera del vault, tras confirmar.
        Delete = "file_tree::delete",
        /// Acepta lo escrito o el borrado.
        Confirm = "file_tree::confirm",
        /// Cierra la pregunta o el menú; en el árbol, quita el filtro.
        Cancel = "file_tree::cancel",
        MenuNext = "menu::select_next",
        MenuPrevious = "menu::select_previous",
    }
}

named_actions! {
    /// De cualquier picker: moverse por la lista como en Telescope y Zed, y
    /// editar el input como en readline.
    pub enum PickerAction {
        SelectNext = "picker::select_next",
        SelectPrevious = "picker::select_previous",
        /// Elige la fila marcada.
        Confirm = "picker::confirm",
        /// Abre lo elegido en un panel nuevo a la derecha.
        ConfirmSplitRight = "picker::confirm_split_right",
        /// Abre lo elegido en un panel nuevo abajo.
        ConfirmSplitDown = "picker::confirm_split_down",
        /// Cierra el picker sin elegir.
        Dismiss = "picker::dismiss",
        DeleteBackward = "picker::delete_backward",
        DeleteForward = "picker::delete_forward",
        DeleteWordBackward = "picker::delete_word_backward",
        DeleteToStart = "picker::delete_to_start",
        MoveLeft = "picker::move_left",
        MoveRight = "picker::move_right",
        MoveToStart = "picker::move_to_start",
        MoveToEnd = "picker::move_to_end",
    }
}

named_actions! {
    /// De la vista previa de un cambio de tags (05 §5).
    pub enum TagChangeAction {
        SelectNext = "tag_change::select_next",
        SelectPrevious = "tag_change::select_previous",
        /// Quita el cambio elegido de lo que se aplica, o lo devuelve.
        ToggleChange = "tag_change::toggle_change",
        /// Escribe lo que queda en el vault.
        Apply = "tag_change::apply",
        /// Cierra la vista previa sin tocar nada.
        Cancel = "tag_change::cancel",
    }
}

/// Los identificadores que cada vista pone en su contexto de teclas.
pub mod context {
    /// La vista de editor, en cualquier modo de vim.
    pub const EDITOR: &str = "Editor";
    /// El árbol del panel lateral con el foco.
    pub const FILE_TREE: &str = "FileTree";
    /// Escribiendo un nombre o el filtro en el panel lateral.
    pub const FILE_TREE_INPUT: &str = "FileTreeInput";
    /// Confirmando un borrado.
    pub const FILE_TREE_CONFIRM: &str = "FileTreeConfirm";
    /// Con el menú contextual abierto.
    pub const FILE_TREE_MENU: &str = "FileTreeMenu";
    /// Lo que se agrega al del editor con la lista del autocompletado de
    /// `#` abierta.
    pub const TAG_COMPLETION: &str = "TagCompletion";
    /// Cualquier picker.
    pub const PICKER: &str = "Picker";
    /// La vista previa de un cambio de tags.
    pub const TAG_CHANGE: &str = "TagChange";
    /// Lo que se agrega al de un picker si abre en un split.
    pub const SPLITS: &str = "splits";
}

/// Dónde vale un atajo que resuelve la ventana, no el motor vim.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum View {
    Editor,
    /// El editor con la lista del autocompletado de `#` abierta.
    TagCompletion,
    FileTree,
    FileTreeInput,
    FileTreeConfirm,
    FileTreeMenu,
    Picker,
    /// Un picker que abre en un split, como el finder.
    PickerWithSplits,
    TagChange,
}

impl View {
    /// Como lo escriben los contextos de Zed y GPUI.
    #[must_use]
    pub const fn predicate(self) -> &'static str {
        match self {
            Self::Editor => context::EDITOR,
            Self::TagCompletion => "Editor && TagCompletion",
            Self::FileTree => context::FILE_TREE,
            Self::FileTreeInput => context::FILE_TREE_INPUT,
            Self::FileTreeConfirm => context::FILE_TREE_CONFIRM,
            Self::FileTreeMenu => context::FILE_TREE_MENU,
            Self::Picker => context::PICKER,
            Self::PickerWithSplits => "Picker && splits",
            Self::TagChange => context::TAG_CHANGE,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn el_predicado_del_autocompletado_nombra_sus_dos_contextos() {
        assert_eq!(
            View::TagCompletion.predicate(),
            format!("{} && {}", context::EDITOR, context::TAG_COMPLETION)
        );
    }

    #[test]
    fn el_predicado_de_splits_nombra_sus_dos_contextos() {
        assert_eq!(
            View::PickerWithSplits.predicate(),
            format!("{} && {}", context::PICKER, context::SPLITS)
        );
    }
}
