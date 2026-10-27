//! Lo que el motor guarda de cada panel: la ventana, la jumplist y cómo se
//! ven las líneas en su ancho. Quien aloja el motor tiene un [`PaneState`]
//! por panel y se lo presta en cada tecla con el [`Target`](crate::Target).

use metalmind_buffer::{Buffer, ByteOffset, Selection, Selections};
use metalmind_syntax::Screen;

use crate::{DocState, JumpList, Layout, Mode, Sideways, Viewport};

#[derive(Clone, Debug, Default)]
pub struct PaneState {
    /// La UI le da su alto en cada layout ([`Viewport::resize`]) y pinta
    /// desde [`Viewport::top`]; el motor la mueve tras cada tecla.
    pub viewport: Viewport,
    /// Como en vim, es de cada panel; un split copia la suya. La del panel
    /// que se edita la sigue el motor; las de los demás, quien saca los
    /// reemplazos del buffer ([`JumpList::adjust`]).
    pub jumps: JumpList,
    /// El ancho del área de texto y si parte las líneas largas; sin ella
    /// (un motor sin UI), ninguna línea se parte ni la ventana se corre a lo
    /// ancho.
    pub(crate) screen: Option<Screen>,
}

impl PaneState {
    #[must_use]
    pub fn screen(&self) -> Option<&Screen> {
        self.screen.as_ref()
    }

    /// El ancho de este layout. Si cambia, las líneas pueden ocupar otras
    /// filas: el siguiente [`PaneState::reveal`] vuelve a mostrar el cursor.
    pub fn set_screen(&mut self, screen: Option<Screen>) {
        if screen != self.screen {
            self.screen = screen;
            self.viewport.heights_changed();
        }
    }

    /// Cómo se ven las líneas de `doc` en este panel.
    #[must_use]
    pub fn layout<'a>(&'a self, buffer: &'a Buffer, doc: &'a DocState) -> Layout<'a> {
        doc.layout(buffer, self.screen.as_ref(), self.viewport.row())
    }

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
        let layout = doc.layout(buffer, self.screen.as_ref(), self.viewport.row());
        self.viewport.reveal(buffer, head, scrolloff, &layout);
        self.viewport.follow_in_line(buffer, head, &layout);
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
        let layout = doc.layout(buffer, self.screen.as_ref(), self.viewport.row());
        self.viewport
            .scroll_to(top, buffer, selections, scrolloff, mode, &layout)
    }

    /// La rueda corrió la ventana a lo ancho hasta `left` (sin `wrap`): el
    /// cursor principal se queda dentro, como `set_leftcol` de vim. Devuelve
    /// si se movió.
    pub fn scroll_sideways_to(
        &mut self,
        left: usize,
        buffer: &Buffer,
        selections: &mut Selections,
        doc: &mut DocState,
        mode: Mode,
    ) -> bool {
        self.sideways(buffer, selections, doc, mode, |viewport, head, layout| {
            viewport.set_left(left, buffer, head, layout)
        })
    }

    /// `zh`, `zl`, `zH`, `zL`, `zs` o `ze` con `count`; ver
    /// [`PaneState::scroll_sideways_to`].
    pub(crate) fn scroll_sideways(
        &mut self,
        sideways: Sideways,
        count: usize,
        buffer: &Buffer,
        selections: &mut Selections,
        doc: &mut DocState,
        mode: Mode,
    ) -> bool {
        self.sideways(buffer, selections, doc, mode, |viewport, head, layout| {
            viewport.scroll_sideways(sideways, count, buffer, head, layout)
        })
    }

    /// Corre la ventana con `scroll`, que devuelve a dónde va el cursor
    /// principal si quedó fuera; en Visual, la selección se extiende.
    fn sideways(
        &mut self,
        buffer: &Buffer,
        selections: &mut Selections,
        doc: &mut DocState,
        mode: Mode,
        scroll: impl FnOnce(&mut Viewport, ByteOffset, &Layout<'_>) -> Option<ByteOffset>,
    ) -> bool {
        let primary = *selections.primary();
        doc.ensure(buffer, buffer.line_of(primary.head));
        let layout = doc.layout(buffer, self.screen.as_ref(), self.viewport.row());
        let Some(head) = scroll(&mut self.viewport, primary.head, &layout) else {
            return false;
        };
        let extend = matches!(mode, Mode::Visual(_));
        *selections = selections.map(|selection| {
            if *selection == primary {
                Selection {
                    anchor: if extend { selection.anchor } else { head },
                    ..Selection::cursor(head)
                }
            } else {
                *selection
            }
        });
        true
    }
}
