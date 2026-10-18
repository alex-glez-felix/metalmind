//! Cómo se mueve la ventana de vim sobre el texto, portado de `move.c` de
//! nvim 0.12 para el caso de nota: sin pliegues, sin líneas partidas y sin
//! filas de relleno, así que cada línea ocupa una fila. Las líneas cuentan
//! desde 0; en vim, desde 1.

use std::cmp::Ordering;

/// La ventana y el cursor mientras dura un comando.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Window {
    /// Primera línea visible (`w_topline`).
    pub top: usize,
    pub height: usize,
    pub lines: usize,
    pub scrolloff: usize,
    /// Línea del cursor.
    pub cursor: usize,
    /// La línea del cursor con la que `top` quedó validado (`VALID_TOPLINE`
    /// con `w_valid_cursor` de vim); si el cursor sigue ahí, no se recalcula.
    pub settled: Option<usize>,
}

impl Window {
    /// La primera línea que ya no se ve (`w_botline`); `lines` si se ve el
    /// final.
    pub fn bottom(&self) -> usize {
        self.top.saturating_add(self.height).min(self.lines)
    }

    /// Filas vacías bajo el final del texto (`w_empty_rows`).
    pub fn empty_rows(&self) -> usize {
        self.height - (self.bottom() - self.top)
    }

    fn shows_end(&self) -> bool {
        self.bottom() == self.lines
    }

    fn last_line(&self) -> usize {
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
        if self.top > 0 && self.cursor < self.top.saturating_add(self.scrolloff) {
            let half = (self.height / 2).saturating_sub(1).max(2);
            if self.top.saturating_add(self.scrolloff) - self.cursor >= half {
                self.scroll_cursor_halfway(false, false);
                check_bottom = false;
            } else {
                self.scroll_cursor_top(1, false);
            }
        }
        if check_bottom && self.needs_bottom_context() {
            let bottom = self.bottom();
            let rows = (self.cursor + 1 + self.scrolloff).saturating_sub(bottom);
            if rows <= self.height + 1 {
                self.scroll_cursor_bot(1, false);
            } else {
                self.scroll_cursor_halfway(false, false);
            }
        }
        self.settle();
    }

    /// Faltan líneas bajo el cursor y hay texto más abajo para mostrarlas.
    fn needs_bottom_context(&self) -> bool {
        let bottom = self.bottom();
        if bottom >= self.lines {
            return false;
        }
        self.cursor >= bottom || bottom - self.cursor - 1 < self.scrolloff
    }

    /// `scroll_cursor_top`: el cursor arriba, con `scrolloff` líneas encima
    /// si caben. Sin `always` solo baja `top`.
    pub fn scroll_cursor_top(&mut self, min_scroll: usize, always: bool) {
        let off = self.scrolloff;
        let mut scrolled = usize::from(self.cursor < self.top);
        let mut used = 1;
        let mut extra = 0;
        let mut new_top = self.cursor;
        let mut below = self.cursor + 1;
        let mut above = self.cursor.checked_sub(1);
        while let Some(line) = above {
            if line < self.top {
                scrolled += 1;
            }
            if (new_top >= self.top || scrolled > min_scroll) && extra >= off {
                break;
            }
            used += 1;
            // Como vim, no cuenta la última línea como contexto de abajo.
            if extra < off && below + 1 < self.lines {
                used += 1;
            }
            if used > self.height {
                break;
            }
            extra += 1;
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

    /// `scroll_cursor_bot`: el cursor abajo, con `scrolloff` líneas debajo
    /// si las hay. Con `set_bottom` (`zb`) parte de la ventana que termina
    /// justo en el cursor.
    pub fn scroll_cursor_bot(&mut self, min_scroll: usize, set_bottom: bool) {
        let (bottom, empty_rows) = if set_bottom {
            let shown = (self.cursor + 1).min(self.height);
            self.top = self.cursor + 1 - shown;
            (self.cursor + 1, self.height - shown)
        } else {
            (self.bottom(), self.empty_rows())
        };
        let empty = isize_of(empty_rows);
        let mut scrolled: isize = 0;
        if self.cursor >= bottom {
            scrolled = 1;
            if self.cursor == bottom {
                scrolled -= empty;
            }
        }
        let min_scroll = isize_of(min_scroll);
        let mut used = 1;
        let mut extra = 0;
        let mut above = self.cursor;
        let mut below = self.cursor;
        while above > 0 {
            let enough = (scrolled <= 0 || scrolled >= min_scroll) && extra >= self.scrolloff;
            if (enough || below >= self.last_line()) && above <= bottom {
                break;
            }
            let previous = above;
            above -= 1;
            used += 1;
            if used > self.height {
                break;
            }
            if above >= bottom {
                scrolled += 1;
                if above == bottom && previous > bottom {
                    scrolled -= empty;
                }
            }
            if below < self.last_line() {
                let previous = below;
                below += 1;
                used += 1;
                if used > self.height {
                    break;
                }
                if extra < self.scrolloff || scrolled < min_scroll {
                    extra += 1;
                    if below >= bottom {
                        scrolled += 1;
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
            used
        } else {
            let scrolled = scrolled.unsigned_abs();
            if scrolled > bottom - self.top + 1 {
                usize::MAX
            } else {
                scrolled
            }
        };
        if shift >= self.height && isize_of(shift) > min_scroll {
            self.scroll_cursor_halfway(false, true);
        } else if shift > 0 {
            self.scroll_up(shift);
        }
        self.settle();
    }

    /// `scroll_cursor_halfway`: el cursor en medio. Con `at_end` las filas
    /// vacías tras el final también cuentan, como en `zz`.
    pub fn scroll_cursor_halfway(&mut self, at_end: bool, prefer_above: bool) {
        let mut used = 1;
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
                        used += 1;
                        if used > self.height {
                            break 'fill;
                        }
                    } else if at_end {
                        used += 1;
                    }
                    rows_below += 1;
                }
                let add_above = round == 1
                    && if prefer_above {
                        rows_below >= rows_above
                    } else {
                        rows_below > rows_above
                    };
                if add_above {
                    used += 1;
                    if used > self.height {
                        break 'fill;
                    }
                    rows_above += 1;
                    top -= 1;
                }
            }
        }
        self.top = top;
        self.settle();
    }

    /// `cursor_correct`: mueve el cursor, no la ventana, para que tenga
    /// `scrolloff` líneas alrededor; menos si se ve el principio o el final
    /// del texto. Como en vim, solo valida `top` si tuvo que revisar.
    pub fn cursor_correct(&mut self) {
        let mut above_wanted = self.scrolloff;
        let mut below_wanted = self.scrolloff;
        if self.top == 0 {
            above_wanted = 0;
            below_wanted = below_wanted.min(self.height / 2);
        }
        let bottom = self.bottom();
        if self.shows_end() {
            below_wanted = 0;
            above_wanted = above_wanted.min((self.height - 1) / 2);
        }
        let line = self.cursor;
        if line >= self.top.saturating_add(above_wanted)
            && line.saturating_add(below_wanted) < bottom
        {
            return;
        }
        let mut first = self.top;
        let mut last = bottom - 1;
        let (mut above, mut below) = (0, 0);
        while (above < above_wanted || below < below_wanted) && first < last {
            if below < below_wanted && (below <= above || above >= above_wanted) {
                below += 1;
                last -= 1;
            }
            if above < above_wanted && (above < below || below >= below_wanted) {
                above += 1;
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
        let last_row = self.top + self.height - 1;
        self.cursor = self.cursor.min(last_row).max(self.top);
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
}

fn isize_of(value: usize) -> isize {
    isize::try_from(value).unwrap_or(isize::MAX)
}
