//! Ediciones que no salen de una tecla, como renombrar un tag en un buffer
//! con cambios sin guardar (05 §5).

use metalmind_buffer::{Edit, Goal, Selection, Splice};

use super::Vim;
use crate::lines::clamp_normal;
use crate::{Target, VimError};

impl Vim {
    /// Aplica `edits`, en coordenadas del texto de ahora, como un solo paso
    /// de undo: `u` las deshace juntas y devuelve el cursor a donde estaba.
    /// Los cursores siguen al texto, y las marks y la jumplist también, como
    /// tras cualquier edición. No es un cambio para `.`, y lo pendiente
    /// (count, operador) se cancela; el modo no cambia, porque puede ser el
    /// de otro documento, como un Visual en otro panel.
    ///
    /// # Errors
    /// Si hay un Insert abierto, que ya tiene su transacción, o alguna
    /// edición no cae en el buffer; entonces no se aplica ninguna.
    pub fn apply_edits(&mut self, edits: Vec<Edit>, t: &mut Target<'_>) -> Result<(), VimError> {
        let version = t.buffer.version();
        t.buffer.begin_transaction(t.selections.clone())?;
        self.reset();
        let count = t.buffer.splice_count();
        if let Err(error) = t.buffer.apply(edits) {
            t.buffer.commit_transaction(t.selections.clone())?;
            return Err(error.into());
        }
        // Como los cursores de los otros paneles (lo borrado cae al inicio
        // del reemplazo), para que todos queden igual.
        let buffer = &*t.buffer;
        let splices = buffer.splices_since(count);
        let after = t.selections.map(|selection| {
            let head = Splice::map_all(splices, selection.head);
            if selection.anchor == selection.head {
                return Selection::cursor(clamp_normal(buffer, head));
            }
            Selection {
                anchor: Splice::map_all(splices, selection.anchor),
                head,
                goal: Goal::None,
            }
        });
        t.buffer.commit_transaction(after.clone())?;
        *t.selections = after;
        self.follow_cursor(t, version);
        Ok(())
    }
}
