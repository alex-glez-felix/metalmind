//! Lo que la ventana corre dentro de las líneas, en las unidades de la
//! pantalla: sin `wrap`, a lo ancho (`w_leftcol` de vim, con `sidescroll=1`
//! y `sidescrolloff=0`, los de nvim); con `wrap`, las filas de arriba de una
//! primera línea más alta que la ventana (`w_skipcol`).

use metalmind_buffer::{Buffer, ByteOffset};
use metalmind_syntax::{Screen, ScreenLine};

use super::{LineHeights, Viewport};
use crate::Layout;

/// `zh`, `zl`, `zH`, `zL`, `zs` y `ze`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Sideways {
    /// `zh`: `count` columnas a la izquierda.
    Left,
    /// `zl`.
    Right,
    /// `zH`: media pantalla.
    HalfLeft,
    /// `zL`.
    HalfRight,
    /// `zs`: el cursor queda al principio de la pantalla.
    CursorStart,
    /// `ze`: al final.
    CursorEnd,
}

impl Viewport {
    /// Sin `wrap`, lo que cada línea tiene oculto a la izquierda.
    #[must_use]
    pub fn left(&self) -> usize {
        self.left
    }

    /// Con `wrap`, lo que la primera línea tiene oculto arriba: es más alta
    /// que la ventana y el cursor está más abajo.
    #[must_use]
    pub fn skip(&self) -> usize {
        self.skip
    }

    /// Tras mover la ventana por líneas, que también se vea la fila o la
    /// columna del cursor, como `curs_columns` de vim.
    pub(crate) fn follow_in_line(&mut self, buffer: &Buffer, cursor: ByteOffset, layout: &Layout) {
        let line = buffer.line_of(cursor);
        let byte = cursor.get() - buffer.line_start(line).get();
        let (Some(screen), Some(shown)) = (layout.screen(), layout.screen_line(line)) else {
            (self.left, self.skip) = (0, 0);
            return;
        };
        if screen.wrap {
            self.left = 0;
            self.skip = self.skip_for(line, &shown, byte, layout);
        } else {
            self.skip = 0;
            self.left = sidescroll(self.left, &shown, byte, screen);
        }
    }

    /// `zh`, `zl`, `zH`, `zL`, `zs` o `ze` con `count`; sin `wrap` no
    /// hacen nada. Devuelve el byte de la línea donde tiene que quedar el
    /// cursor si salió de la pantalla.
    pub(crate) fn scroll_sideways(
        &mut self,
        sideways: Sideways,
        count: usize,
        buffer: &Buffer,
        cursor: ByteOffset,
        layout: &Layout,
    ) -> Option<ByteOffset> {
        let screen = layout.screen().filter(|screen| !screen.wrap)?;
        let line = buffer.line_of(cursor);
        let shown = layout.screen_line(line)?;
        let byte = cursor.get() - buffer.line_start(line).get();
        let columns = |count: usize| units(screen.column * count_f32(count));
        let half = units(screen.width / 2.0);
        let left = match sideways {
            Sideways::Left => self.left.saturating_sub(columns(count)),
            Sideways::Right => self.left.saturating_add(columns(count)),
            Sideways::HalfLeft => self.left.saturating_sub(half.saturating_mul(count)),
            Sideways::HalfRight => self.left.saturating_add(half.saturating_mul(count)),
            Sideways::CursorStart => units(shown.x(byte)),
            Sideways::CursorEnd => units_up(cursor_end(&shown, byte, screen) - screen.width),
        };
        self.set_left(left, buffer, cursor, layout)
    }

    /// Corre la ventana a `left` y deja el cursor dentro, como `set_leftcol`
    /// de vim: si queda a un lado, va al primer o al último carácter que se
    /// ve entero. Si la línea no llega a `left`, el cursor va a su último
    /// carácter y la ventana se corre solo hasta él. Devuelve el cursor si
    /// tuvo que moverse.
    pub(crate) fn set_left(
        &mut self,
        left: usize,
        buffer: &Buffer,
        cursor: ByteOffset,
        layout: &Layout,
    ) -> Option<ByteOffset> {
        let screen = layout.screen().filter(|screen| !screen.wrap)?;
        let line = buffer.line_of(cursor);
        let shown = layout.screen_line(line)?;
        let start = buffer.line_start(line);
        let byte = cursor.get() - start.get();
        self.left = left;
        let advances = shown.advances();
        let (from, to) = (f32_of(left), f32_of(left) + screen.width);
        let moved = if cursor_end(&shown, byte, screen) > to {
            Some(advances.last_until(to).unwrap_or(0))
        } else if shown.x(byte) < from {
            advances.first_from(from).or_else(|| {
                let last = advances.char_before(advances.len());
                self.left = units(advances.x(last));
                Some(last)
            })
        } else {
            None
        };
        moved.map(|byte| start + byte)
    }

    /// Lo que se oculta arriba de `line` para que se vea la fila del
    /// cursor, si `line` es la primera y no cabe: lo mínimo desde lo que ya
    /// se ocultaba.
    fn skip_for(&self, line: usize, shown: &ScreenLine, byte: usize, layout: &Layout) -> usize {
        if line != self.top || layout.height(line) <= self.height {
            return 0;
        }
        let rows = layout.rows(line);
        let row = shown.row_of(byte);
        let top = rows.top(row);
        let bottom = top + rows.rows.get(row).copied().unwrap_or(self.row);
        self.skip.max(bottom.saturating_sub(self.height)).min(top)
    }
}

/// Hasta dónde llega el cursor sobre `byte`; en el fin de línea, una
/// columna de texto.
fn cursor_end(shown: &ScreenLine, byte: usize, screen: &Screen) -> f32 {
    if byte < shown.len() {
        shown.end(byte)
    } else {
        shown.x(byte) + screen.column
    }
}

/// `left` para que se vea el cursor sobre `byte`: lo mínimo si está cerca
/// del borde y, si está a media pantalla o más, centrado.
fn sidescroll(left: usize, shown: &ScreenLine, byte: usize, screen: &Screen) -> usize {
    let (start, end) = (shown.x(byte), cursor_end(shown, byte, screen));
    let from = f32_of(left);
    let off_left = start - from;
    let off_right = end - from - screen.width;
    if off_left >= 0.0 && off_right <= 0.0 {
        return left;
    }
    let diff = if off_left < 0.0 { -off_left } else { off_right };
    if diff >= (screen.width / 2.0).floor() || off_right >= off_left {
        units(start - (screen.width / 2.0).floor())
    } else if off_left < 0.0 {
        units(from - diff)
    } else {
        units_up(from + diff)
    }
}

#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "píxeles de un panel, acotados a [0, ancho de una línea]"
)]
pub(crate) fn units(pixels: f32) -> usize {
    pixels.max(0.0).floor() as usize
}

fn units_up(pixels: f32) -> usize {
    units(pixels.ceil())
}

#[expect(
    clippy::cast_precision_loss,
    reason = "píxeles de una línea: muy por debajo de 2^24"
)]
pub(crate) fn f32_of(units: usize) -> f32 {
    units as f32
}

fn count_f32(count: usize) -> f32 {
    f32_of(count.min(1 << 20))
}
