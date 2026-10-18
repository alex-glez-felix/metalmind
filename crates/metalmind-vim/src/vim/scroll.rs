//! La ventana del panel que se edita: los comandos de scroll, `H`, `M` y
//! `L` fuera de un operador, y que el cursor se vea tras cada tecla.

use metalmind_buffer::{Buffer, ByteOffset, Goal, Selection, Selections};

use super::Vim;
use super::dispatch::move_cursors;
use crate::lines::{self, Want, at_byte_column, clamp_normal, last_char, tab_end};
use crate::motion::vertical;
use crate::scroll::Shift;
use crate::{Place, Scroll, Viewport, VimError};

impl Vim {
    /// Las líneas que se dejan visibles por encima y por debajo del cursor.
    pub fn set_scrolloff(&mut self, lines: usize) {
        self.scrolloff = lines;
    }

    /// Cambia la ventana del motor por `viewport`: antes de editar en un
    /// panel se le da la suya y después se recupera, ya movida.
    pub fn swap_viewport(&mut self, viewport: &mut Viewport) {
        std::mem::swap(&mut self.viewport, viewport);
    }

    /// La rueda llevó la ventana a `top`: los cursores la siguen, como en
    /// vim, y lo pendiente (un count, un operador) se cancela, como con un
    /// clic. Si se mueven en Insert, lo que se teclee después es otro cambio.
    pub fn scroll_to(&mut self, top: usize, buffer: &Buffer, selections: &mut Selections) {
        self.reset();
        let moved = self
            .viewport
            .scroll_to(top, buffer, selections, self.scrolloff, self.mode);
        if let Some(session) = self.insert.as_mut().filter(|_| moved) {
            session.cursor_moved();
        }
    }

    /// Tras cada tecla, como vim antes de pintar: el cursor se ve. Si el
    /// texto cambió, se revisa aunque siga en la misma línea.
    pub(super) fn follow_cursor(&mut self, buffer: &Buffer, selections: &Selections, version: u64) {
        if buffer.version() != version {
            self.viewport.invalidate();
        }
        self.viewport
            .reveal(buffer, selections.primary().head, self.scrolloff);
    }

    /// Los cursores que no son el principal se mueven las mismas líneas que
    /// él. `ctrl-d`, `ctrl-u`, `ctrl-f` y `ctrl-b` fallan si no se movió nada.
    pub(super) fn scroll(&mut self, scroll: Scroll, buffer: &Buffer, selections: &mut Selections) {
        let count = self.take_count();
        self.reset();
        let head = selections.primary().head;
        let from = buffer.line_of(head);
        let mut window = self.viewport.window(buffer, head, self.scrolloff);
        let shift = match scroll {
            Scroll::LineDown | Scroll::LineUp => {
                window.scroll_lines(scroll == Scroll::LineDown, count.unwrap_or(1));
                let line = window.cursor;
                (line != from).then_some(Shift { from, line })
            }
            Scroll::HalfPageDown | Scroll::HalfPageUp => {
                let amount = self.viewport.half_page(count);
                let moved = window.scroll_half_page(scroll == Scroll::HalfPageDown, amount);
                self.page_shift(moved, from, window.cursor)
            }
            Scroll::PageDown | Scroll::PageUp => {
                let moved = window.scroll_page(scroll == Scroll::PageDown, count.unwrap_or(1));
                self.page_shift(moved, from, window.cursor)
            }
        };
        self.viewport.apply(&window);
        if let Some(shift) = shift {
            *selections = shift.apply(buffer, selections, self.mode, self.viewport.columns());
        }
    }

    fn page_shift(&mut self, moved: bool, from: usize, to: usize) -> Option<Shift> {
        if !moved {
            self.fail();
        }
        moved.then_some(Shift { from, line: to })
    }

    /// `zt`, `zz`, `zb` y, al primer no blanco, `z<CR>`, `z.` y `z-`. Con
    /// count van antes a esa línea, como un salto, en el mismo byte de la
    /// línea (`nv_zet` de vim) y buscando la misma columna de pantalla.
    pub(super) fn reposition(
        &mut self,
        place: Place,
        first_non_blank: bool,
        buffer: &mut Buffer,
        selections: &mut Selections,
    ) -> Result<(), VimError> {
        let count = self.take_count();
        self.reset();
        let current = buffer.line_of(selections.primary().head);
        if let Some(line) = count.map(|n| n.saturating_sub(1)).filter(|&l| l != current) {
            self.jumped(buffer, selections)?;
            let line = line.min(buffer.last_line());
            let visual = self.in_visual();
            let columns = self.viewport.columns();
            let mode = self.mode;
            *selections = selections.map(|s| {
                let column = s.head.get() - buffer.line_start(buffer.line_of(s.head)).get();
                let offset = at_byte_column(buffer, line, column);
                let head = if visual {
                    offset
                } else {
                    clamp_normal(buffer, offset)
                };
                let want = Want::of(buffer, s, tab_end(mode, s), columns);
                Selection {
                    goal: want.goal(),
                    ..moved_head(s, head, visual)
                }
            });
        }
        if first_non_blank {
            let visual = self.in_visual();
            *selections = selections.map(|s| {
                let line = buffer.line_of(s.head);
                let head = lines::first_non_blank(buffer, line).min(last_char(buffer, line));
                Selection {
                    goal: Goal::None,
                    ..moved_head(s, head, visual)
                }
            });
        }
        let mut window = self
            .viewport
            .window(buffer, selections.primary().head, self.scrolloff);
        match place {
            Place::Top => window.scroll_cursor_top(0, true),
            Place::Middle => window.scroll_cursor_halfway(true, false),
            Place::Bottom => window.scroll_cursor_bot(0, true),
        }
        self.viewport.apply(&window);
        Ok(())
    }

    /// `H`, `M` y `L` en Normal y Visual: además de ir a la línea, el
    /// cursor respeta `scrolloff`, cosa que con un operador no hace.
    pub(super) fn move_to_window_line(
        &mut self,
        place: Place,
        buffer: &mut Buffer,
        selections: &mut Selections,
    ) -> Result<(), VimError> {
        let count = self.take_count();
        self.reset();
        self.jumped(buffer, selections)?;
        let head = selections.primary().head;
        let mut window = self.viewport.window(buffer, head, self.scrolloff);
        window.cursor = self.viewport.line_at(place, count.unwrap_or(1), buffer);
        window.cursor_correct();
        self.viewport.apply(&window);
        let purpose = self.motion_purpose();
        let columns = self.viewport.columns();
        if let Some(moved) = move_cursors(selections, self.in_visual(), |s| {
            Some(vertical(buffer, s, window.cursor, purpose, columns))
        }) {
            *selections = moved;
        }
        Ok(())
    }
}

/// `selection` con el cursor en `head`; fuera de Visual, sin selección.
fn moved_head(selection: &Selection, head: ByteOffset, visual: bool) -> Selection {
    Selection {
        anchor: if visual { selection.anchor } else { head },
        head,
        goal: selection.goal,
    }
}
