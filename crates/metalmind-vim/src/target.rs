use metalmind_buffer::{Buffer, Selections};

use crate::{Clipboard, DocState, FileId, PaneState};

/// Lo que edita una tecla, prestado por quien aloja el motor: el buffer y
/// los cursores del panel, lo que el motor guarda del documento y del panel,
/// y el archivo del documento, para la jumplist y las marks `A`–`Z`. Lo de
/// todo el editor (el modo, los registros, las macros, las marks `A`–`Z`,
/// el historial de `:`) queda en el [`Vim`](crate::Vim).
pub struct Target<'a> {
    pub buffer: &'a mut Buffer,
    pub selections: &'a mut Selections,
    pub doc: &'a mut DocState,
    pub pane: &'a mut PaneState,
    pub file: FileId,
    /// Lo que leen y escriben `"+` y `"*`. Sin él, el motor los guarda como
    /// registros normales, como nvim sin proveedor de portapapeles.
    pub clipboard: Option<&'a mut dyn Clipboard>,
}
