//! `pagescroll` de nvim: `ctrl-d`, `ctrl-u`, `ctrl-f` y `ctrl-b`. nvim
//! mueve filas y, sin `smoothscroll`, termina en una línea entera; aquí la
//! ventana se mueve de a líneas enteras, las que caben en esas filas.

use super::window::Window;

/// Las líneas enteras de un tramo de filas.
struct Fit {
    lines: usize,
    rows: usize,
    /// Se acabó el texto antes de llenar las filas.
    at_edge: bool,
}

impl Window<'_> {
    /// Media pantalla, o las `amount` filas que fijó un count (la opción
    /// `scroll` de vim). La ventana no deja ver filas tras el final y el
    /// cursor baja o sube las mismas filas, aunque la ventana ya no se
    /// mueva. Devuelve si cambió algo; si no, vim avisa con un pitido.
    pub fn scroll_half_page(&mut self, down: bool, amount: usize) -> bool {
        let (top, cursor) = (self.top, self.cursor);
        let wanted = amount.min(self.height_lines()).saturating_mul(self.row);
        let rows = if down {
            self.rows_before_end(wanted)
        } else {
            wanted
        };
        let mut cursor_rows = wanted;
        if rows > 0 {
            let fit = self.lines_for(down, rows);
            self.scroll_lines(down, fit.lines);
            self.cursor = cursor;
            // nvim termina en una línea entera y el cursor se mueve lo mismo.
            if !fit.at_edge {
                cursor_rows = (wanted + fit.rows).saturating_sub(rows);
            }
        }
        self.cursor = self.cursor_moved(down, cursor, cursor_rows);
        self.finish_page(top, cursor)
    }

    /// Lo que puede bajar la ventana de `rows` sin que se vean filas tras
    /// el final del texto.
    fn rows_before_end(&self, rows: usize) -> usize {
        let limit = self.height + rows;
        let mut shown = 0;
        for line in self.top..self.lines {
            shown += self.h(line);
            if shown >= limit {
                return rows;
            }
        }
        if self.top + 1 >= self.lines {
            return 0;
        }
        shown.saturating_sub(self.height)
    }

    /// `count` pantallas, menos dos líneas que se repiten para no perder el
    /// hilo; el cursor queda arriba (`ctrl-f`) o abajo (`ctrl-b`).
    pub fn scroll_page(&mut self, down: bool, count: usize) -> bool {
        let (top, cursor) = (self.top, self.cursor);
        let rows = count.saturating_mul(self.page_overlap(down));
        let lines = self.lines_for(down, rows).lines;
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

    /// Las líneas enteras que caben en `rows` filas desde arriba (o desde
    /// la de antes de arriba, hacia atrás); al menos una si hay filas.
    fn lines_for(&self, down: bool, rows: usize) -> Fit {
        let mut fit = Fit {
            lines: 0,
            rows: 0,
            at_edge: false,
        };
        if rows == 0 {
            return fit;
        }
        loop {
            let line = if down {
                Some(self.top + fit.lines).filter(|&line| line < self.lines)
            } else {
                self.top.checked_sub(fit.lines + 1)
            };
            let Some(line) = line else {
                fit.at_edge = true;
                break;
            };
            let h = self.h(line);
            if fit.rows + h > rows && fit.lines > 0 {
                break;
            }
            fit.rows += h;
            fit.lines += 1;
            if fit.rows >= rows {
                break;
            }
        }
        fit
    }

    /// La línea a `rows` filas de `from`, como `nv_screengo`: una línea
    /// alta se recorre entera antes de pasar a la siguiente.
    fn cursor_moved(&self, down: bool, from: usize, rows: usize) -> usize {
        let mut left = rows;
        let mut line = from;
        if down {
            while left >= self.h(line) && line < self.last_line() {
                left -= self.h(line);
                line += 1;
            }
        } else {
            while left > 0 && line > 0 {
                line -= 1;
                left = left.saturating_sub(self.h(line));
            }
        }
        line
    }

    /// `get_scroll_overlap`: cuánto avanza una pantalla, en unidades.
    /// Repite dos líneas, una en ventanas chicas y ninguna si ya se ve el
    /// borde hacia el que se va.
    fn page_overlap(&self, down: bool) -> usize {
        let full = self.height;
        let min = full.saturating_sub(2 * self.row);
        let at_edge = if down {
            self.bottom() == self.lines
        } else {
            self.top == 0
        };
        if at_edge {
            return full;
        }
        // Las líneas junto al borde, desde la que queda justo afuera.
        let near = |steps: usize| {
            let line = if down {
                self.bottom().checked_sub(steps)
            } else {
                (self.top + steps).checked_sub(1)
            };
            line.filter(|&line| line < self.lines)
                .map(|line| self.h(line))
        };
        let Some(h1) = near(0) else {
            return full;
        };
        if h1 > min {
            return full;
        }
        let Some(h2) = near(1).filter(|&h2| h2 + h1 <= min) else {
            return full;
        };
        let Some(h3) = near(2).filter(|&h3| h3 + h2 <= min) else {
            return full;
        };
        match near(3) {
            Some(h4) if h4 + h3 + h2 <= min && h3 + h2 + h1 <= min => min,
            _ => min + self.row,
        }
    }
}
