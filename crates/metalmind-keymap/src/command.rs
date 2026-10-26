//! Los comandos de la ventana: guardar, abrir y cerrar hacen I/O, y los
//! paneles, los pickers y el panel lateral son de la UI. Se invocan por su
//! nombre desde `:`, los atajos y la paleta.

use metalmind_vim::{ActionName, HostCommand, RangeRule};

use crate::named::named_actions;

named_actions! {
    /// Un comando de la ventana, invocable también por su nombre.
    pub enum Command {
        /// `:w [ruta]`; con ruta, le da archivo a un buffer sin nombre o
        /// escribe una copia.
        Save = "editor::save",
        /// `:w! [ruta]`: guarda aunque el archivo haya cambiado en disco
        /// desde que se leyó, y con ruta pisa un archivo que ya existe.
        ForceSave = "editor::force_save",
        /// `:q`, que cierra el panel; en el último, se niega si algún buffer
        /// tiene cambios.
        Quit = "editor::quit",
        /// `:q!`
        ForceQuit = "editor::force_quit",
        /// `:wq [ruta]`
        SaveAndQuit = "editor::save_and_quit",
        /// `:wa`, que guarda todos los buffers con cambios.
        SaveAll = "workspace::save_all",
        /// `:qa`, que cierra la ventana si ningún buffer tiene cambios.
        QuitAll = "workspace::quit_all",
        /// `:qa!`, que cierra la ventana aunque haya cambios.
        ForceQuitAll = "workspace::force_quit_all",
        /// `:wqa` y `:xa`: guarda todos los buffers con cambios y cierra.
        SaveAllAndQuit = "workspace::save_all_and_quit",
        /// `:e [ruta]`. Con otra ruta, el buffer de antes queda oculto con
        /// sus cambios, como con `hidden` de vim; sin ruta, relee el archivo
        /// del panel y se niega si se perderían cambios.
        Edit = "editor::edit",
        /// `:e! [ruta]`; sin ruta, relee el archivo y descarta los cambios.
        ForceEdit = "editor::force_edit",
        /// `ctrl-w v`, `:vsplit`
        SplitRight = "pane::split_right",
        /// `ctrl-w s`, `:split`
        SplitDown = "pane::split_down",
        /// `ctrl-w h`
        FocusLeft = "pane::focus_left",
        /// `ctrl-w j`
        FocusDown = "pane::focus_down",
        /// `ctrl-w k`
        FocusUp = "pane::focus_up",
        /// `ctrl-w l`
        FocusRight = "pane::focus_right",
        /// `ctrl-w w`
        FocusNext = "pane::focus_next",
        /// `ctrl-w o`, `:only`: cierra los demás paneles; sus buffers quedan
        /// ocultos.
        Only = "pane::only",
        /// `:set {opciones}`: las opciones del panel enfocado, como sus
        /// números de línea.
        Set = "editor::set",
        /// `space f`: el finder de notas.
        OpenFinder = "workspace::open_finder",
        /// `space /`: la búsqueda de texto en las notas del vault.
        OpenTextSearch = "workspace::open_text_search",
        /// `space p`: la paleta de comandos.
        OpenCommandPalette = "workspace::open_command_palette",
        /// `space e`: muestra el panel lateral en la vista de carpetas, o lo
        /// esconde si ya la muestra.
        ToggleFolders = "panel::toggle_folders",
        /// `space t`: lo mismo con la vista de tags.
        ToggleTags = "panel::toggle_tags",
        /// `gd` o `enter`: abre lo que nombra el link bajo el cursor, en el
        /// heading o el bloque que nombre.
        FollowLink = "editor::follow_link",
        /// `:tagrename #viejo #nuevo`: la vista previa de renombrar un tag o
        /// un alias en todo el vault; si el nuevo ya existe, se fusionan.
        RenameTag = "tags::rename",
        /// `:tagmerge #a #b`: la vista previa de pasar las notas de `#a` a
        /// `#b`.
        MergeTag = "tags::merge",
        /// `:tagundo`: deshace el último cambio de tags aplicado.
        UndoTagChange = "tags::undo",
    }
}

/// Nombre completo en `:`, cuántas letras bastan para abreviarlo y el
/// comando.
const EX: &[(&str, usize, Command)] = &[
    ("write", 1, Command::Save),
    ("write!", 1, Command::ForceSave),
    ("quit", 1, Command::Quit),
    ("quit!", 1, Command::ForceQuit),
    ("wq", 2, Command::SaveAndQuit),
    ("wall", 2, Command::SaveAll),
    ("qall", 2, Command::QuitAll),
    ("quitall", 5, Command::QuitAll),
    ("qall!", 2, Command::ForceQuitAll),
    ("quitall!", 5, Command::ForceQuitAll),
    ("wqall", 3, Command::SaveAllAndQuit),
    ("xall", 2, Command::SaveAllAndQuit),
    ("edit", 1, Command::Edit),
    ("edit!", 1, Command::ForceEdit),
    ("split", 2, Command::SplitDown),
    ("vsplit", 2, Command::SplitRight),
    ("only", 2, Command::Only),
    ("set", 2, Command::Set),
    ("tagrename", 9, Command::RenameTag),
    ("tagmerge", 8, Command::MergeTag),
    ("tagundo", 7, Command::UndoTagChange),
];

impl Command {
    /// Si acepta lo escrito tras su nombre en `:`.
    #[must_use]
    pub fn takes_argument(self) -> bool {
        matches!(
            self,
            Self::Save
                | Self::ForceSave
                | Self::SaveAndQuit
                | Self::Edit
                | Self::ForceEdit
                | Self::Set
                | Self::RenameTag
                | Self::MergeTag
        )
    }

    /// El error que da en `:` con un rango, que ninguno admite.
    fn with_range(self) -> RangeRule {
        match self {
            Self::Save | Self::SaveAndQuit => RangeRule::PartialWrite,
            Self::SaveAll
            | Self::QuitAll
            | Self::ForceQuitAll
            | Self::SaveAllAndQuit
            | Self::Edit
            | Self::ForceEdit
            | Self::Set
            | Self::RenameTag
            | Self::MergeTag
            | Self::UndoTagChange => RangeRule::NotAllowed,
            _ => RangeRule::Unsupported,
        }
    }
}

/// Los comandos de `:` que el motor reconoce y devuelve por su nombre.
pub(crate) fn ex_commands() -> impl Iterator<Item = HostCommand> {
    EX.iter().map(|&(name, min, command)| HostCommand {
        name,
        min,
        action: ActionName::new(command.name()),
        takes_argument: command.takes_argument(),
        with_range: command.with_range(),
    })
}
