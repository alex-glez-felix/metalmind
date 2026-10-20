//! Lo que el motor guarda de cada panel: la ventana y la jumplist. Quien
//! aloja el motor tiene un [`PaneState`] por panel y se lo presta en cada
//! tecla con el [`Target`](crate::Target).

use metalmind_buffer::{Buffer, Selections};

use crate::{DocState, JumpList, LineHeights, Mode, Uniform, Viewport};

#[derive(Clone, Debug, Default)]
pub struct PaneState {
    /// La UI le da su alto en cada layout ([`Viewport::resize`]) y pinta
    /// desde [`Viewport::top`]; el motor la mueve tras cada tecla.
    pub viewport: Viewport,
    /// Como en vim, es de cada panel; un split copia la suya. La del panel
    /// que se edita la sigue el motor; las de los demás, quien saca los
    /// reemplazos del buffer ([`JumpList::adjust`]).
    pub jumps: JumpList,
}

impl PaneState {
    /// La ventana muestra el cursor principal con `scrolloff` líneas
    /// alrededor, como vim antes de pintar, con lo que mide cada línea de
    /// `doc`: antes se sigue lo editado y se mide lo que se va a mirar.
    pub fn reveal(
        &mut self,
        buffer: &mut Buffer,
        selections: &Selections,
        doc: &mut DocState,
        scrolloff: usize,
    ) {
        doc.sync(buffer);
        let head = selections.primary().head;
        doc.ensure_window(buffer, &self.viewport, head, scrolloff, 0);
        let uniform = Uniform(self.viewport.row());
        let heights = doc.heights().unwrap_or(&uniform);
        self.viewport.reveal(buffer, head, scrolloff, heights);
    }

    /// La rueda llevó la ventana a `top`: los cursores la siguen si
    /// quedarían fuera, como en vim, y en Visual extienden la selección.
    /// Devuelve si los movió.
    pub fn scroll_to(
        &mut self,
        top: usize,
        buffer: &Buffer,
        selections: &mut Selections,
        doc: &mut DocState,
        scrolloff: usize,
        mode: Mode,
    ) -> bool {
        let head = selections.primary().head;
        let extra = top.saturating_sub(self.viewport.top());
        doc.ensure_window(buffer, &self.viewport, head, scrolloff, extra);
        let uniform = Uniform(self.viewport.row());
        let heights: &dyn LineHeights = doc.heights().unwrap_or(&uniform);
        self.viewport
            .scroll_to(top, buffer, selections, scrolloff, mode, heights)
    }
}
