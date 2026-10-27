//! La ventana del panel que se edita: los comandos de scroll, `H`, `M` y
//! `L` fuera de un operador, y que el cursor se vea tras cada tecla.

use metalmind_buffer::{ByteOffset, Goal, Selection};

use super::Vim;
use super::dispatch::move_cursors;
use crate::lines::{self, Want, at_byte_column, clamp_normal, last_char, tab_end};
use crate::motion::vertical;
use crate::scroll::Shift;
use crate::{Place, Scroll, Sideways, Target, VimError};

impl Vim {
    /// Las líneas que se dejan visibles por encima y por debajo del cursor.
    pub fn set_scrolloff(&mut self, lines: usize) {
        self.scrolloff = lines;
    }

    /// Calcula el estilo de las líneas que va a mirar la ventana, hasta
    /// `extra` líneas más allá de lo que se ve.
    fn measure(&self, t: &mut Target<'_>, extra: usize) {
        let head = t.selections.primary().head;
        t.doc
            .ensure_window(t.buffer, &t.pane.viewport, head, self.scrolloff, extra);
    }

    /// La rueda llevó la ventana a `top`: los cursores la siguen, como en
    /// vim, y lo pendiente (un count, un operador) se cancela, como con un
    /// clic. Si se mueven en Insert, lo que se teclee después es otro cambio.
    pub fn scroll_to(&mut self, top: usize, t: &mut Target<'_>) {
        self.reset();
        let moved = t.pane.scroll_to(
            top,
            t.buffer,
            t.selections,
            t.doc,
            self.scrolloff,
            self.mode,
        );
        if let Some(session) = self.insert.as_mut().filter(|_| moved) {
            session.cursor_moved();
        }
    }

    /// La rueda corrió la ventana a lo ancho hasta `left`, como
    /// [`Vim::scroll_to`] a lo alto.
    pub fn scroll_sideways_to(&mut self, left: usize, t: &mut Target<'_>) {
        self.reset();
        let moved = t
            .pane
            .scroll_sideways_to(left, t.buffer, t.selections, t.doc, self.mode);
        if let Some(session) = self.insert.as_mut().filter(|_| moved) {
            session.cursor_moved();
        }
    }

    /// `zh`, `zl`, `zH`, `zL`, `zs` y `ze`.
    pub(super) fn scroll_sideways(&mut self, sideways: Sideways, t: &mut Target<'_>) {
        let count = self.take_count().unwrap_or(1);
        self.reset();
        t.pane
            .scroll_sideways(sideways, count, t.buffer, t.selections, t.doc, self.mode);
    }

    /// Tras cada tecla, como vim antes de pintar: el cursor se ve. Si el
    /// texto cambió, se revisa aunque siga en la misma línea.
    pub(super) fn follow_cursor(&mut self, t: &mut Target<'_>, version: u64) {
        if t.buffer.version() != version {
            t.pane.viewport.invalidate();
            self.catch_up(t);
        }
        t.pane.reveal(t.buffer, t.selections, t.doc, self.scrolloff);
    }

    /// Los cursores que no son el principal se mueven las mismas líneas que
    /// él. `ctrl-d`, `ctrl-u`, `ctrl-f` y `ctrl-b` fallan si no se movió nada.
    pub(super) fn scroll(&mut self, scroll: Scroll, t: &mut Target<'_>) {
        let count = self.take_count();
        self.reset();
        let head = t.selections.primary().head;
        let from = t.buffer.line_of(head);
        let page = (t.pane.viewport.height() / t.pane.viewport.row().max(1)).max(1);
        let reach = match scroll {
            Scroll::LineDown | Scroll::LineUp => count.unwrap_or(1),
            Scroll::HalfPageDown | Scroll::HalfPageUp => page,
            Scroll::PageDown | Scroll::PageUp => count.unwrap_or(1).saturating_mul(page),
        };
        self.measure(t, reach);
        let half = matches!(scroll, Scroll::HalfPageDown | Scroll::HalfPageUp)
            .then(|| t.pane.viewport.half_page(count));
        let layout = t
            .doc
            .layout(t.buffer, t.pane.screen.as_ref(), t.pane.viewport.row());
        let mut window = t
            .pane
            .viewport
            .window(t.buffer, head, self.scrolloff, &layout);
        let moved = match (scroll, half) {
            (Scroll::LineDown | Scroll::LineUp, _) => {
                window.scroll_lines(scroll == Scroll::LineDown, count.unwrap_or(1));
                true
            }
            (Scroll::HalfPageDown | Scroll::HalfPageUp, Some(amount)) => {
                window.scroll_half_page(scroll == Scroll::HalfPageDown, amount)
            }
            _ => window.scroll_page(scroll == Scroll::PageDown, count.unwrap_or(1)),
        };
        let line = window.cursor;
        t.pane.viewport.apply(&window);
        if !moved {
            self.fail();
        }
        let shift = (moved && line != from).then_some(Shift { from, line });
        if let Some(shift) = shift {
            *t.selections =
                shift.apply(t.buffer, t.selections, self.mode, t.pane.viewport.columns());
        }
    }

    /// `zt`, `zz`, `zb` y, al primer no blanco, `z<CR>`, `z.` y `z-`. Con
    /// count van antes a esa línea, como un salto, en el mismo byte de la
    /// línea (`nv_zet` de vim) y buscando la misma columna de pantalla.
    pub(super) fn reposition(
        &mut self,
        place: Place,
        first_non_blank: bool,
        t: &mut Target<'_>,
    ) -> Result<(), VimError> {
        let count = self.take_count();
        self.reset();
        let current = t.buffer.line_of(t.selections.primary().head);
        if let Some(line) = count.map(|n| n.saturating_sub(1)).filter(|&l| l != current) {
            self.jumped(t)?;
            let line = line.min(t.buffer.last_line());
            let visual = self.in_visual();
            let columns = t.pane.viewport.columns();
            let mode = self.mode;
            *t.selections = t.selections.map(|s| {
                let column = s.head.get() - t.buffer.line_start(t.buffer.line_of(s.head)).get();
                let offset = at_byte_column(t.buffer, line, column);
                let head = if visual {
                    offset
                } else {
                    clamp_normal(t.buffer, offset)
                };
                let want = Want::of(t.buffer, s, tab_end(mode, s), columns);
                Selection {
                    goal: want.goal(),
                    ..moved_head(s, head, visual)
                }
            });
        }
        if first_non_blank {
            let visual = self.in_visual();
            *t.selections = t.selections.map(|s| {
                let line = t.buffer.line_of(s.head);
                let head = lines::first_non_blank(t.buffer, line).min(last_char(t.buffer, line));
                Selection {
                    goal: Goal::None,
                    ..moved_head(s, head, visual)
                }
            });
        }
        let head = t.selections.primary().head;
        self.measure(t, 0);
        let layout = t
            .doc
            .layout(t.buffer, t.pane.screen.as_ref(), t.pane.viewport.row());
        let mut window = t
            .pane
            .viewport
            .window(t.buffer, head, self.scrolloff, &layout);
        match place {
            Place::Top => window.scroll_cursor_top(0, true),
            Place::Middle => window.scroll_cursor_halfway(true, false),
            Place::Bottom => window.scroll_cursor_bot(0, true),
        }
        t.pane.viewport.apply(&window);
        Ok(())
    }

    /// `H`, `M` y `L` en Normal y Visual: además de ir a la línea, el
    /// cursor respeta `scrolloff`, cosa que con un operador no hace.
    pub(super) fn move_to_window_line(
        &mut self,
        place: Place,
        t: &mut Target<'_>,
    ) -> Result<(), VimError> {
        let count = self.take_count();
        self.reset();
        self.jumped(t)?;
        let head = t.selections.primary().head;
        self.measure(t, 0);
        let layout = t
            .doc
            .layout(t.buffer, t.pane.screen.as_ref(), t.pane.viewport.row());
        let mut window = t
            .pane
            .viewport
            .window(t.buffer, head, self.scrolloff, &layout);
        window.cursor = t.pane.viewport.line_at(place, count.unwrap_or(1), t.buffer);
        window.cursor_correct();
        t.pane.viewport.apply(&window);
        let purpose = self.motion_purpose();
        let columns = t.pane.viewport.columns();
        if let Some(moved) = move_cursors(t.selections, self.in_visual(), |s| {
            Some(vertical(t.buffer, s, window.cursor, purpose, columns))
        }) {
            *t.selections = moved;
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
