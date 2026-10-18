//! `pagescroll` de nvim: `ctrl-d`, `ctrl-u`, `ctrl-f` y `ctrl-b`.

use super::window::Window;

impl Window {
    /// Media pantalla, o las `amount` líneas que fijó un count (la opción
    /// `scroll` de vim). La ventana no deja ver filas tras el final y el
    /// cursor baja o sube lo mismo, aunque la ventana ya no se mueva.
    /// Devuelve si cambió algo; si no, vim avisa con un pitido.
    pub fn scroll_half_page(&mut self, down: bool, amount: usize) -> bool {
        let (top, cursor) = (self.top, self.cursor);
        let moves = amount.min(self.height);
        let mut count = moves;
        if down && self.top + 1 + self.height + count > self.lines {
            let shown = 1 + (self.lines - self.top - 1).min(self.height + count);
            if shown < self.height + count {
                count = shown.saturating_sub(self.height);
            }
        }
        if count > 0 {
            self.scroll_lines(down, count);
            self.cursor = cursor;
        }
        self.cursor = if down {
            cursor.saturating_add(moves).min(self.lines - 1)
        } else {
            cursor.saturating_sub(moves)
        };
        self.finish_page(top, cursor)
    }

    /// `count` pantallas, menos dos líneas que se repiten para no perder el
    /// hilo; el cursor queda arriba (`ctrl-f`) o abajo (`ctrl-b`).
    pub fn scroll_page(&mut self, down: bool, count: usize) -> bool {
        let (top, cursor) = (self.top, self.cursor);
        let lines = count.saturating_mul(self.page_overlap(down));
        self.scroll_lines(down, lines);
        if self.top != top {
            self.cursor = if down { self.top } else { self.bottom() - 1 };
        }
        self.finish_page(top, cursor)
    }

    fn finish_page(&mut self, top: usize, cursor: usize) -> bool {
        if self.scrolloff > 0 {
            self.cursor_correct();
        }
        self.top != top || self.cursor != cursor
    }

    /// `get_scroll_overlap`: cuánto avanza una pantalla. Repite dos líneas,
    /// una en ventanas chicas y ninguna si ya se ve el borde hacia el que
    /// se va.
    fn page_overlap(&self, down: bool) -> usize {
        let full = self.height;
        let at_edge = if down {
            self.bottom() == self.lines
        } else {
            self.top == 0
        };
        if at_edge || full < 4 {
            return full;
        }
        let exists = |steps: usize| {
            if down {
                self.bottom().checked_sub(steps).is_some()
            } else {
                self.top - 1 + steps < self.lines
            }
        };
        if !exists(2) {
            full
        } else if !exists(3) || full < 5 {
            full - 1
        } else {
            full - 2
        }
    }
}
