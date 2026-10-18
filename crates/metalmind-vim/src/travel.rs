use metalmind_buffer::{Buffer, BufferError, Selection, Selections, Transaction};

use crate::lines::clamp_normal;

/// Cómo se mueve el cursor por el historial: `u`, `ctrl-r`, `g-`, `g+`.
#[derive(Clone, Copy)]
pub(crate) enum Travel {
    Undo,
    Redo,
    Earlier,
    Later,
}

impl Travel {
    /// Como vim, deja el cursor donde empezó el último cambio recorrido;
    /// `None` si no había historial que recorrer.
    pub(crate) fn run(
        self,
        buffer: &mut Buffer,
        times: usize,
    ) -> Result<Option<Selections>, BufferError> {
        let restored = match self {
            Self::Undo => repeat(buffer, times, Buffer::undo)?,
            Self::Redo => repeat(buffer, times, Buffer::redo)?,
            Self::Earlier => buffer.earlier(times)?.map(cursors_before),
            Self::Later => buffer.later(times)?.map(cursors_before),
        };
        Ok(restored.map(|restored| {
            restored.map(|selection| Selection::cursor(clamp_normal(buffer, selection.head)))
        }))
    }
}

/// Da `times` pasos con `step` mientras haya historial; devuelve las
/// selecciones de antes del último cambio recorrido.
fn repeat(
    buffer: &mut Buffer,
    times: usize,
    step: fn(&mut Buffer) -> Result<Option<&Transaction>, BufferError>,
) -> Result<Option<Selections>, BufferError> {
    let mut restored = None;
    for _ in 0..times {
        let Some(transaction) = step(buffer)? else {
            break;
        };
        restored = Some(cursors_before(transaction));
    }
    Ok(restored)
}

fn cursors_before(transaction: &Transaction) -> Selections {
    transaction.selections_before().clone()
}
