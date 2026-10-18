//! Cómo se mueve la ventana de vim sobre el texto, portado de `move.c` de
//! nvim 0.12 sin pliegues ni filas de relleno. Una línea puede ocupar más de
//! una fila (un heading más grande): es el caso de vim con `wrap`, donde
//! cada línea ocupa `plines` filas, sin `smoothscroll`, así que la primera
//! línea visible siempre se ve entera. El alto se mide en unidades de
//! [`LineHeights`]; las líneas cuentan desde 0, en vim desde 1.

use std::cmp::Ordering;

use super::LineHeights;

/// La ventana y el cursor mientras dura un comando.
#[derive(Clone, Copy)]
pub(crate) struct Window<'h> {
    /// Primera línea visible (`w_topline`).
    pub top: usize,
    /// El alto, en unidades.
    pub height: usize,
    /// Cuántas unidades mide una línea de texto: una fila.
    pub row: usize,
    pub lines: usize,
    /// En líneas de texto; se compara con filas, como en vim.
    pub scrolloff: usize,
    /// Línea del cursor.
    pub cursor: usize,
    /// La línea del cursor con la que `top` quedó validado (`VALID_TOPLINE`
    /// con `w_valid_cursor` de vim); si el cursor sigue ahí, no se recalcula.
    pub settled: Option<usize>,
    pub heights: &'h dyn LineHeights,
}

impl std::fmt::Debug for Window<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Window")
            .field("top", &self.top)
            .field("height", &self.height)
            .field("cursor", &self.cursor)
            .finish_non_exhaustive()
    }
}

impl Window<'_> {
    /// Lo que mide `line` (`plines`); nunca menos de una unidad.
    pub fn h(&self, line: usize) -> usize {
        self.heights.height(line).max(1)
    }

    /// `scrolloff` en unidades.
    fn so(&self) -> usize {
        self.scrolloff.saturating_mul(self.row)
    }

    /// Cuántas filas de texto caben.
    pub fn height_lines(&self) -> usize {
        (self.height / self.row.max(1)).max(1)
    }

    /// `comp_botline`: la primera línea que no se ve entera (`w_botline`;
    /// `lines` si se ve el final) y lo que ocupan las de arriba. Una línea
    /// más alta que la ventana cuenta como vista.
    pub fn fit(&self) -> (usize, usize) {
        let mut used = 0;
        let mut line = self.top;
        while line < self.lines {
            let h = self.h(line);
            if used + h > self.height {
                break;
            }
            used += h;
            line += 1;
        }
        if line == self.top && line < self.lines {
            return (line + 1, self.height);
        }
        (line, used)
    }

    pub fn bottom(&self) -> usize {
        self.fit().0
    }

    /// Lo que sobra bajo la última línea entera (`w_empty_rows`).
    pub fn empty_rows(&self) -> usize {
        let (_, used) = self.fit();
        self.height.saturating_sub(used)
    }

    fn shows_end(&self) -> bool {
        self.bottom() == self.lines
    }

    pub fn last_line(&self) -> usize {
        self.lines - 1
    }

    fn settle(&mut self) {
        self.settled = Some(self.cursor);
    }

    /// `update_topline`: lo que hace vim antes de pintar para que el cursor
    /// se vea con `scrolloff` líneas alrededor. Si tiene que mover mucho la
    /// ventana, deja el cursor en medio.
    pub fn show_cursor(&mut self) {
        if self.settled == Some(self.cursor) {
            return;
        }
        let mut check_bottom = true;
        if self.top > 0 && (self.cursor < self.top || self.lacks_top_context()) {
            let half = (self.height_lines() / 2).saturating_sub(1).max(2);
            if self
                .top
                .saturating_add(self.scrolloff)
                .saturating_sub(self.cursor)
                >= half
            {
                self.scroll_cursor_halfway(false, false);
                check_bottom = false;
            } else {
                self.scroll_cursor_top(1, false);
            }
        }
        if check_bottom && self.lacks_bottom_context() {
            let bottom = self.bottom();
            let rows = (self.cursor + 1 + self.scrolloff).saturating_sub(bottom);
            if rows <= self.height_lines() + 1 {
                self.scroll_cursor_bot(1, false);
            } else {
                self.scroll_cursor_halfway(false, false);
            }
        }
        self.settle();
    }

    /// `check_top_offset`: menos de `scrolloff` filas sobre el cursor.
    fn lacks_top_context(&self) -> bool {
        if self.cursor >= self.top.saturating_add(self.scrolloff) {
            return false;
        }
        let mut rows = 0;
        let mut line = self.cursor;
        while rows < self.so() && line > self.top {
            line -= 1;
            rows += self.h(line);
        }
        rows < self.so()
    }

    /// Faltan filas bajo el cursor y hay texto más abajo para mostrarlas.
    fn lacks_bottom_context(&self) -> bool {
        let (bottom, used) = self.fit();
        if bottom >= self.lines {
            return false;
        }
        if self.cursor >= bottom {
            return true;
        }
        if self.cursor.saturating_add(self.scrolloff) < bottom {
            return false;
        }
        let mut rows = self.height.saturating_sub(used);
        for line in self.cursor + 1..bottom {
            if rows >= self.so() {
                break;
            }
            rows += self.h(line);
        }
        rows < self.so()
    }

    /// `scroll_cursor_top`: el cursor arriba, con `scrolloff` filas encima
    /// si caben. Sin `always` solo baja `top`.
    pub fn scroll_cursor_top(&mut self, min_scroll: usize, always: bool) {
        let off = self.so();
        let mut used = self.h(self.cursor);
        let mut scrolled = if self.cursor < self.top { used } else { 0 };
        let mut extra = 0;
        let mut new_top = self.cursor;
        let mut below = self.cursor + 1;
        let mut above = self.cursor.checked_sub(1);
        while let Some(line) = above {
            let h = self.h(line);
            if line < self.top {
                scrolled += h;
            }
            if (new_top >= self.top || scrolled > min_scroll) && extra >= off {
                break;
            }
            used += h;
            // Como vim, no cuenta la última línea como contexto de abajo.
            if extra + h <= off && below + 1 < self.lines {
                used += self.h(below);
            }
            if used > self.height {
                break;
            }
            extra += h;
            new_top = line;
            above = line.checked_sub(1);
            below += 1;
        }
        if used > self.height {
            self.scroll_cursor_halfway(false, false);
            return;
        }
        if new_top < self.top || always {
            self.top = new_top;
        }
        self.top = self.top.min(self.cursor);
        self.settle();
    }

    /// `scroll_cursor_bot`: el cursor abajo, con `scrolloff` filas debajo
    /// si las hay. Con `set_bottom` (`zb`) parte de la ventana que termina
    /// justo en el cursor.
    pub fn scroll_cursor_bot(&mut self, min_scroll: usize, set_bottom: bool) {
        let (bottom, empty_rows) = if set_bottom {
            self.top = self.top_ending_at(self.cursor);
            let used: usize = (self.top..=self.cursor).map(|line| self.h(line)).sum();
            (self.cursor + 1, self.height.saturating_sub(used))
        } else {
            (self.bottom(), self.empty_rows())
        };
        let empty = isize_of(empty_rows);
        let mut used = self.h(self.cursor);
        let mut scrolled: isize = 0;
        if self.cursor >= bottom {
            scrolled = isize_of(used);
            if self.cursor == bottom {
                scrolled -= empty;
            }
        }
        let min_scroll = isize_of(min_scroll);
        let so = self.so();
        let mut extra = 0;
        let mut above = self.cursor;
        let mut below = self.cursor;
        while above > 0 {
            let enough = (scrolled <= 0 || scrolled >= min_scroll) && extra >= so;
            if (enough || below >= self.last_line()) && above <= bottom {
                break;
            }
            let previous = above;
            above -= 1;
            let h = self.h(above);
            used += h;
            if used > self.height {
                break;
            }
            if above >= bottom {
                scrolled += isize_of(h);
                if above == bottom && previous > bottom {
                    scrolled -= empty;
                }
            }
            if below < self.last_line() {
                let previous = below;
                below += 1;
                let h = self.h(below);
                used += h;
                if used > self.height {
                    break;
                }
                if extra < so || scrolled < min_scroll {
                    extra += h;
                    if below >= bottom {
                        scrolled += isize_of(h);
                        if previous < bottom {
                            scrolled -= empty;
                        }
                    }
                }
            }
        }
        let shift = if scrolled <= 0 {
            0
        } else if used > self.height {
            usize::MAX
        } else {
            self.lines_in_rows(scrolled.unsigned_abs(), bottom)
        };
        if shift >= self.height_lines() && isize_of(shift.saturating_mul(self.row)) > min_scroll {
            self.scroll_cursor_halfway(false, true);
        } else if shift > 0 {
            self.scroll_up(shift);
        }
        self.settle();
    }

    /// La primera línea de la ventana que termina en `last`, como `zb`.
    fn top_ending_at(&self, last: usize) -> usize {
        let mut used = 0;
        let mut top = last;
        let mut line = last + 1;
        while line > 0 {
            line -= 1;
            let h = self.h(line);
            if used + h > self.height {
                break;
            }
            top = line;
            used += h;
        }
        top
    }

    /// Cuántas líneas desde arriba suman `rows` filas, sin pasar de
    /// `bottom`; `usize::MAX` si no alcanzan.
    fn lines_in_rows(&self, rows: usize, bottom: usize) -> usize {
        let mut covered = 0;
        let mut count = 0;
        let mut line = self.top;
        while covered < rows && line <= bottom && line < self.lines {
            covered += self.h(line);
            count += 1;
            line += 1;
        }
        if covered < rows { usize::MAX } else { count }
    }

    /// `scroll_cursor_halfway`: el cursor en medio. Con `at_end` las filas
    /// vacías tras el final también cuentan, como en `zz`.
    pub fn scroll_cursor_halfway(&mut self, at_end: bool, prefer_above: bool) {
        let mut used = self.h(self.cursor);
        let mut top = self.cursor;
        let mut below = self.cursor;
        'fill: while top > 0 {
            let mut rows_above = 0;
            let mut rows_below = 0;
            for round in 1..=2 {
                let add_below = if prefer_above {
                    round == 2 && rows_below < rows_above
                } else {
                    round == 1 && rows_below <= rows_above
                };
                if add_below {
                    if below < self.last_line() {
                        below += 1;
                        let h = self.h(below);
                        used += h;
                        if used > self.height {
                            break 'fill;
                        }
                        rows_below += h;
                    } else {
                        rows_below += self.row;
                        if at_end {
                            used += self.row;
                        }
                    }
                }
                let add_above = round == 1
                    && if prefer_above {
                        rows_below >= rows_above
                    } else {
                        rows_below > rows_above
                    };
                if add_above {
                    let h = self.h(top - 1);
                    used += h;
                    if used > self.height {
                        break 'fill;
                    }
                    rows_above += h;
                    top -= 1;
                }
            }
        }
        self.top = top;
        self.settle();
    }

    /// `cursor_correct`: mueve el cursor, no la ventana, para que tenga
    /// `scrolloff` filas alrededor; menos si se ve el principio o el final
    /// del texto. Como en vim, solo valida `top` si tuvo que revisar.
    pub fn cursor_correct(&mut self) {
        let mut above_wanted = self.so();
        let mut below_wanted = self.so();
        if self.top == 0 {
            above_wanted = 0;
            below_wanted = below_wanted.min(self.height / 2);
        }
        let bottom = self.bottom();
        if self.shows_end() {
            below_wanted = 0;
            above_wanted = above_wanted.min(self.height.saturating_sub(1) / 2);
        }
        let row = self.row.max(1);
        let line = self.cursor;
        if line >= self.top.saturating_add(above_wanted / row)
            && line.saturating_add(below_wanted / row) < bottom
        {
            return;
        }
        let mut first = self.top;
        let mut last = bottom - 1;
        let (mut above, mut below) = (0, 0);
        while (above < above_wanted || below < below_wanted) && first < last {
            if below < below_wanted && (below <= above || above >= above_wanted) {
                below += self.h(last);
                last -= 1;
            }
            if above < above_wanted && (above < below || below >= below_wanted) {
                above += self.h(first);
                first += 1;
            }
        }
        match first.cmp(&last) {
            Ordering::Equal => self.cursor = first,
            Ordering::Greater => self.cursor = last,
            Ordering::Less => {
                if line < first && self.top > 0 {
                    self.cursor = first;
                }
                if line > last && !self.shows_end() {
                    self.cursor = last;
                }
            }
        }
        self.settle();
    }

    /// `scrollup` (`ctrl-e`): sube el texto hasta que la última línea quede
    /// arriba; el cursor no queda por encima de la ventana. Devuelve si se
    /// movió la ventana.
    pub fn scroll_up(&mut self, count: usize) -> bool {
        let before = self.top;
        self.top = self.top.saturating_add(count).min(self.last_line());
        self.cursor = self.cursor.max(self.top);
        self.top != before
    }

    /// `scrolldown` (`ctrl-y`): baja el texto hasta la primera línea; el
    /// cursor no queda por debajo de la ventana.
    pub fn scroll_down(&mut self, count: usize) {
        self.top = self.top.saturating_sub(count);
        let last_shown = self.bottom() - 1;
        self.cursor = self.cursor.min(last_shown).max(self.top);
    }

    /// `scroll_redraw`: `ctrl-e` o `ctrl-y` y, con `scrolloff`, el cursor
    /// que se corrige. Si la ventana no se movió, el cursor sí, para no
    /// atascarse.
    pub fn scroll_lines(&mut self, down: bool, count: usize) {
        let (top, cursor) = (self.top, self.cursor);
        if down {
            self.scroll_up(count);
        } else {
            self.scroll_down(count);
        }
        if self.scrolloff == 0 {
            return;
        }
        self.cursor_correct();
        self.settle();
        if self.top != top {
            return;
        }
        if down {
            if self.cursor <= cursor && self.cursor < self.last_line() {
                self.cursor = (cursor + 1).min(self.last_line());
            }
        } else if self.cursor >= cursor && top > 0 {
            self.cursor = cursor.saturating_sub(1);
        }
        self.settle();
    }

    /// `M`: la línea de en medio de las filas ocupadas, como `nv_scroll`.
    pub fn middle(&self) -> usize {
        let (_, used) = self.fit();
        let half = used.div_ceil(2).max(1);
        let mut rows = 0;
        let mut line = self.top;
        while line < self.last_line() {
            rows += self.h(line);
            if rows >= half {
                break;
            }
            line += 1;
        }
        line
    }
}

fn isize_of(value: usize) -> isize {
    isize::try_from(value).unwrap_or(isize::MAX)
}
