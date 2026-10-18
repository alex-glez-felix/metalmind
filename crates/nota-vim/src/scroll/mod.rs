//! Lo visible de un panel y cómo lo mueven el cursor y los comandos de
//! scroll. El motor no sabe de pantallas: quien lo aloja le dice cuántas
//! filas caben.

mod page;
mod window;

use nota_buffer::{Buffer, ByteOffset, Selection, Selections};

pub(crate) use self::window::Window;
use crate::Mode;
use crate::columns::Columns;
use crate::lines::{Want, tab_end};

/// La primera línea visible de un panel y cuántas filas caben. Quien aloja
/// el motor guarda uno por panel, le da su altura en cada layout
/// ([`Viewport::resize`]) y se lo presta al motor mientras edita en ese
/// panel ([`crate::Vim::swap_viewport`]); el motor lo deja donde quedó tras
/// cada tecla, y la UI pinta desde [`Viewport::top`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Viewport {
    top: usize,
    height: usize,
    /// Las líneas que fijó un count de `ctrl-d` o `ctrl-u` (la opción
    /// `scroll` de vim, que es de cada ventana).
    half_page: Option<usize>,
    /// La línea del cursor con la que `top` quedó validado.
    settled: Option<usize>,
    /// Cómo se miden las columnas en esta ventana (su `tabstop`).
    columns: Columns,
}

/// `ctrl-e`, `ctrl-y`, `ctrl-d`, `ctrl-u`, `ctrl-f` y `ctrl-b`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Scroll {
    LineDown,
    LineUp,
    HalfPageDown,
    HalfPageUp,
    PageDown,
    PageUp,
}

/// Dónde de la ventana: arriba, en medio o abajo.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Place {
    Top,
    Middle,
    Bottom,
}

impl Default for Viewport {
    /// Una fila, hasta que quien aloja el motor diga cuántas caben.
    fn default() -> Self {
        Self::new(1)
    }
}

impl Viewport {
    #[must_use]
    pub fn new(height: usize) -> Self {
        Self {
            top: 0,
            height: height.max(1),
            half_page: None,
            settled: None,
            columns: Columns::default(),
        }
    }

    /// Cuántas columnas avanza un tab; 8, como en vim, si no se dice otra
    /// cosa. Cuenta para la columna que conservan `j` y `k` y para los
    /// bloques de `ctrl-v`, así que tiene que ser el mismo con el que se
    /// pinta el texto.
    pub fn set_tabstop(&mut self, tabstop: usize) {
        self.columns = Columns::new(tabstop);
    }

    pub(crate) fn columns(&self) -> Columns {
        self.columns
    }

    #[must_use]
    pub fn top(&self) -> usize {
        self.top
    }

    #[must_use]
    pub fn height(&self) -> usize {
        self.height
    }

    /// Las filas que caben ahora. Si cambian, `ctrl-d` vuelve a media
    /// pantalla, como en vim, y el siguiente [`Viewport::reveal`] vuelve a
    /// mostrar el cursor.
    pub fn resize(&mut self, height: usize) {
        let height = height.max(1);
        if height != self.height {
            self.height = height;
            self.half_page = None;
            self.settled = None;
        }
    }

    /// Mueve lo mínimo para que se vea el cursor con `scrolloff` líneas
    /// alrededor, o lo centra si está lejos, como vim antes de pintar. Si
    /// el texto se acortó, no deja la ventana pasada del final.
    pub fn reveal(&mut self, buffer: &Buffer, cursor: ByteOffset, scrolloff: usize) {
        let mut window = self.window(buffer, cursor, scrolloff);
        window.show_cursor();
        self.apply(&window);
    }

    /// Lleva la ventana a `top` como la rueda en vim, que es un `ctrl-e` o
    /// `ctrl-y` de varias líneas: los cursores la siguen si quedarían
    /// fuera, y en Visual extienden la selección. Devuelve si los movió.
    pub fn scroll_to(
        &mut self,
        top: usize,
        buffer: &Buffer,
        selections: &mut Selections,
        scrolloff: usize,
        mode: Mode,
    ) -> bool {
        let head = selections.primary().head;
        let mut window = self.window(buffer, head, scrolloff);
        let top = top.min(window.lines - 1);
        if top == window.top {
            return false;
        }
        if top > window.top {
            window.scroll_lines(true, top - window.top);
        } else {
            window.scroll_lines(false, window.top - top);
        }
        self.apply(&window);
        let moved = window.cursor != buffer.line_of(head);
        if moved {
            let shift = Shift {
                from: buffer.line_of(head),
                line: window.cursor,
            };
            *selections = shift.apply(buffer, selections, mode, self.columns);
        }
        moved
    }

    /// `H` y `L`: la línea a `count` filas de arriba o de abajo, aunque
    /// `L` con un count grande suba más allá de la ventana. `M`: la de en
    /// medio de las que se ven.
    pub(crate) fn line_at(&self, place: Place, count: usize, buffer: &Buffer) -> usize {
        let window = self.window(buffer, ByteOffset::ZERO, 0);
        let last_line = buffer.last_line();
        let from_edge = count.saturating_sub(1);
        match place {
            Place::Top => window.top.saturating_add(from_edge).min(last_line),
            Place::Middle => {
                let shown = window.bottom() - window.top;
                let half = shown.div_ceil(2);
                (window.top + half - 1).min(last_line)
            }
            Place::Bottom => (window.bottom() - 1).saturating_sub(from_edge),
        }
    }

    pub(crate) fn window(&self, buffer: &Buffer, cursor: ByteOffset, scrolloff: usize) -> Window {
        let lines = buffer.line_count();
        Window {
            top: self.top.min(lines - 1),
            height: self.height,
            lines,
            scrolloff,
            cursor: buffer.line_of(cursor),
            settled: self.settled,
        }
    }

    pub(crate) fn apply(&mut self, window: &Window) {
        self.top = window.top;
        self.settled = window.settled;
    }

    /// Lo que mueven `ctrl-d` y `ctrl-u`: lo que fijó un count o media
    /// pantalla.
    pub(crate) fn half_page(&mut self, count: Option<usize>) -> usize {
        if let Some(count) = count {
            self.half_page = Some(count.min(self.height));
        }
        self.half_page.unwrap_or((self.height / 2).max(1))
    }

    /// El texto cambió: el próximo [`Viewport::reveal`] revisa aunque el
    /// cursor siga en la misma línea.
    pub(crate) fn invalidate(&mut self) {
        self.settled = None;
    }
}

/// Mover todos los cursores las mismas líneas que el principal, que va de
/// `from` a `line`, a la columna que buscan, como `coladvance` de vim.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Shift {
    pub from: usize,
    pub line: usize,
}

impl Shift {
    /// En Visual el otro extremo de cada selección se queda; ahí y en
    /// Insert el cursor puede quedar sobre el fin de línea.
    pub fn apply(
        self,
        buffer: &Buffer,
        selections: &Selections,
        mode: Mode,
        columns: Columns,
    ) -> Selections {
        let extend = matches!(mode, Mode::Visual(_));
        let past_line_end = mode.reaches_line_end();
        selections.map(|selection| {
            let line = buffer.line_of(selection.head);
            let target = if self.line >= self.from {
                line.saturating_add(self.line - self.from)
                    .min(buffer.last_line())
            } else {
                line.saturating_sub(self.from - self.line)
            };
            let want = Want::of(buffer, selection, tab_end(mode, selection), columns);
            let head = if past_line_end {
                want.visual_offset_on(buffer, target, columns)
            } else {
                want.offset_on(buffer, target, columns)
            };
            Selection {
                anchor: if extend { selection.anchor } else { head },
                head,
                goal: want.goal(),
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn numbered(n: usize) -> Buffer {
        let text: String = (1..=n).map(|i| i.to_string() + "\n").collect();
        Buffer::from(text.as_str())
    }

    #[test]
    fn otra_altura_olvida_el_count_de_ctrl_d() {
        let mut viewport = Viewport::new(10);
        assert_eq!(viewport.half_page(Some(3)), 3);
        assert_eq!(viewport.half_page(None), 3);
        viewport.resize(10);
        assert_eq!(viewport.half_page(None), 3);
        viewport.resize(20);
        assert_eq!(viewport.half_page(None), 10);
    }

    #[test]
    fn el_count_de_ctrl_d_no_pasa_de_la_altura() {
        let mut viewport = Viewport::new(10);
        assert_eq!(viewport.half_page(Some(50)), 10);
    }

    #[test]
    fn con_el_texto_acortado_no_queda_pasada_del_final() {
        let mut viewport = Viewport::new(10);
        let mut selections = Selections::single(Selection::cursor(ByteOffset::ZERO));
        viewport.scroll_to(80, &numbered(100), &mut selections, 0, Mode::Normal);
        let short = numbered(5);
        viewport.reveal(&short, short.line_start(4), 0);
        assert_eq!(viewport.top(), 4);
    }
}
