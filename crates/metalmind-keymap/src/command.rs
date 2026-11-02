//! Los comandos que corre el modelo de la ventana: guardar, abrir y cerrar
//! hacen I/O, y los paneles y lo que cambia una opción son de su estado. Se
//! invocan por su nombre desde `:`, los atajos y la paleta. Los que solo le
//! tocan a las vistas son [`WindowCommand`](crate::WindowCommand).

use metalmind_vim::{ActionName, ArgumentKind, HostCommand, RangeRule};

use crate::named::named_actions;

named_actions! {
    /// Un comando del modelo, invocable también por su nombre.
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
        /// Parte las líneas largas en todos los paneles, o deja de
        /// partirlas y la vista se corre a lo ancho, y lo guarda en
        /// `editor.wrap`; `:set wrap!` cambia solo el panel enfocado.
        ToggleWrap = "editor::toggle_wrap",
        /// Enciende o apaga `markdown.continue_lists` y lo guarda: `Enter`,
        /// `o` y `O` en un ítem de lista abren el siguiente.
        ToggleContinueLists = "editor::toggle_continue_lists",
        /// Enciende o apaga `markdown.indent_list_children` y lo guarda:
        /// `>>` y `<<` en un ítem de lista mueven también sus hijos.
        ToggleIndentListChildren = "editor::toggle_indent_list_children",
        /// Enciende o apaga `markdown.align_tables_on_leave` y lo guarda:
        /// salir de Insert tras escribir en una tabla la alinea.
        ToggleAlignTablesOnLeave = "editor::toggle_align_tables_on_leave",
        /// Enciende o apaga `which_key.enabled` y lo guarda: tras un prefijo
        /// como `space`, un panel con las teclas que siguen.
        ToggleWhichKey = "workspace::toggle_which_key",
        /// `gd` o `enter`: abre lo que nombra el link bajo el cursor, en el
        /// heading o el bloque que nombre.
        FollowLink = "editor::follow_link",
        /// `:tagrename #viejo #nuevo`: la vista previa de renombrar un tag o
        /// un alias en todo el vault; si el nuevo ya existe, se fusionan.
        RenameTag = "tags::rename",
        /// `:tagmerge #a #b`: la vista previa de pasar las notas de `#a` a
        /// `#b`.
        MergeTag = "tags::merge",
        /// `:tagaliases #tag [alias…]`: la vista previa de dejar esos alias
        /// en la página del tag; sin ninguno, de quitarlos.
        EditTagAliases = "tags::edit_aliases",
        /// `:tagparents #tag [padre…]`
        EditTagParents = "tags::edit_parents",
        /// `:tagicon #tag [ícono]`: un emoji o `set:nombre`; sin ícono, lo
        /// quita.
        SetTagIcon = "tags::set_icon",
        /// `:tagtemplate #tag [plantilla]`: la plantilla de sus notas nuevas
        /// (06 §11); sin plantilla, la quita.
        SetTagTemplate = "tags::set_template",
        /// `:tagpage #tag`: abre la página del tag; si no existe, la crea
        /// vacía con su carpeta.
        OpenTagPage = "tags::open_page",
        /// Cierra los toasts, como `esc` en Normal; un clic cierra uno.
        /// Siguen en `:messages`.
        DismissMessages = "messages::dismiss",
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
    ("tagaliases", 10, Command::EditTagAliases),
    ("tagparents", 10, Command::EditTagParents),
    ("tagicon", 7, Command::SetTagIcon),
    ("tagtemplate", 11, Command::SetTagTemplate),
    ("tagpage", 7, Command::OpenTagPage),
];

impl Command {
    /// Qué acepta tras su nombre en `:`.
    #[must_use]
    pub fn argument(self) -> ArgumentKind {
        match self {
            Self::Save | Self::ForceSave | Self::SaveAndQuit | Self::Edit | Self::ForceEdit => {
                ArgumentKind::FileName
            }
            Self::Set
            | Self::RenameTag
            | Self::MergeTag
            | Self::EditTagAliases
            | Self::EditTagParents
            | Self::SetTagIcon
            | Self::SetTagTemplate
            | Self::OpenTagPage => ArgumentKind::Words,
            _ => ArgumentKind::None,
        }
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
            | Self::EditTagAliases
            | Self::EditTagParents
            | Self::SetTagIcon
            | Self::SetTagTemplate
            | Self::OpenTagPage
            | Self::DismissMessages => RangeRule::NotAllowed,
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
        argument: command.argument(),
        with_range: command.with_range(),
    })
}
