//! Los comandos que solo le tocan a las vistas de la ventana: abrir un
//! picker, mostrar un panel, pedirle algo a la lista. El modelo no decide nada
//! con ellos: con que no sean suyos le basta para cortar una macro.

use metalmind_vim::{ActionName, ArgumentKind, HostCommand, RangeRule};

use crate::named::named_actions;

named_actions! {
    /// Un comando de las vistas de la ventana, invocable también por su nombre.
    pub enum WindowCommand {
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
        /// `space b`: muestra el panel de contexto de la nota activa, a la
        /// derecha (#164), o lo cierra.
        ToggleContext = "context::toggle",
        /// `space l`: muestra la lista de notas, o la pliega y deja la nota
        /// sola.
        ToggleNoteList = "list::toggle",
        /// `space v`: elige de dónde salen las notas de la lista: el buzón,
        /// todas, una vista guardada, un tag o una carpeta.
        ChooseListSource = "list::choose_source",
        /// `space c`, o un clic en la ruta sobre un panel: la lista muestra
        /// esa carpeta (la de la nota enfocada, sin argumento) con la nota
        /// elegida.
        ShowFolder = "list::show_folder",
        /// `s` en la lista: guarda su fuente y los tags del filtro como una
        /// vista con nombre (06 §10), que sale en `space v`.
        SaveListView = "list::save_view",
        /// Cambia el nombre de la vista guardada que muestra la lista.
        RenameListView = "list::rename_view",
        /// Borra la vista guardada que muestra la lista; sus notas no cambian.
        DeleteListView = "list::delete_view",
        /// `space u`: el picker de los links del vault que no llevan a
        /// ningún archivo; abre la nota en el elegido.
        OpenUnresolvedLinks = "links::open_unresolved",
        /// `:vaultundo` (o `:tagundo`), `space z` o "Deshacer" en el aviso
        /// al aplicar: deshace el último cambio aplicado al vault, sea de
        /// tags o el renombre de una nota con sus links.
        UndoVaultChange = "vault::undo",
        /// `space r` o "Volver" en el aviso: la vista previa de tags que se
        /// dejó para abrir la línea de un cambio, rehecha con lo de ahora.
        ResumeTagChange = "tags::resume_preview",
        /// `space n`, `:template [plantilla [nombre]]`: una nota nueva desde
        /// una plantilla; lo que falte se elige en un picker.
        NewNoteFromTemplate = "notes::new_from_template",
        /// `space d`, `:today`: abre la nota del día, creándola con su
        /// plantilla si todavía no existe.
        OpenDailyNote = "notes::open_daily",
        /// `:board [nombre]`: una pizarra nueva y vacía. Con nombre se crea
        /// en la raíz del vault; sin él, el panel lateral lo pregunta.
        NewBoard = "board::new",
        /// Abre el finder y la nota elegida entra como una tarjeta en el centro
        /// de la pizarra del panel.
        InsertNoteCard = "board::insert_note_card",
        /// `:focus proyectos/ #rust -#archivado`: reduce el finder, la
        /// búsqueda y el panel a ese alcance (05 §3); sin alcance, dice el de
        /// ahora.
        Focus = "workspace::focus",
        /// `:unfocus`, o un clic en el indicador de la barra de estado.
        ClearFocus = "workspace::clear_focus",
        /// `:messages`: el historial de avisos, también los que ya se
        /// cerraron; el elegido vuelve a verse en un toast.
        ShowMessages = "messages::show",
    }
}

/// Nombre completo en `:`, cuántas letras bastan para abreviarlo y el
/// comando.
const EX: &[(&str, usize, WindowCommand)] = &[
    ("tagundo", 7, WindowCommand::UndoVaultChange),
    ("vaultundo", 9, WindowCommand::UndoVaultChange),
    ("template", 8, WindowCommand::NewNoteFromTemplate),
    ("today", 5, WindowCommand::OpenDailyNote),
    ("board", 5, WindowCommand::NewBoard),
    ("focus", 5, WindowCommand::Focus),
    ("unfocus", 7, WindowCommand::ClearFocus),
    ("messages", 3, WindowCommand::ShowMessages),
];

impl WindowCommand {
    /// Qué acepta tras su nombre en `:`.
    #[must_use]
    pub fn argument(self) -> ArgumentKind {
        match self {
            Self::NewNoteFromTemplate | Self::NewBoard | Self::Focus => ArgumentKind::Words,
            _ => ArgumentKind::None,
        }
    }
}

/// Los comandos de `:` que el motor reconoce y devuelve por su nombre. Ninguno
/// admite un rango.
pub(crate) fn ex_commands() -> impl Iterator<Item = HostCommand> {
    EX.iter().map(|&(name, min, command)| HostCommand {
        name,
        min,
        action: ActionName::new(command.name()),
        argument: command.argument(),
        with_range: RangeRule::NotAllowed,
    })
}
