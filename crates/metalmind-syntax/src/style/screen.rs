//! Cómo se ve una línea en el ancho de un panel: dónde cae cada carácter y,
//! con `wrap`, en qué filas se parte. Los anchos los mide quien pinta (la
//! UI); aquí se decide dónde partir, así el motor mueve la ventana y el
//! cursor con las mismas filas que se pintan.
//!
//! La línea se parte por su fuente, con el cursor encima o no: una fórmula
//! cuenta lo que mide su LaTeX. Así una línea no cambia de filas cuando el
//! cursor entra y muestra la fuente, como `plines` de vim, que mide la línea
//! sin ocultar nada.

use std::fmt::Debug;
use std::ops::Range;
use std::sync::Arc;

use unicode_segmentation::UnicodeSegmentation as _;

use super::LineStyle;

/// Dónde empieza cada carácter de una línea, en píxeles desde su principio,
/// y cuánto mide entera.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Advances {
    /// `(byte, x)` de cada carácter, en orden; los bytes de en medio de un
    /// carácter (o de una ligadura) caen en el que los contiene.
    starts: Vec<(usize, f32)>,
    len: usize,
    width: f32,
}

impl Advances {
    /// Una línea de `len` bytes que mide `width`, con sus caracteres donde
    /// dice `starts` (en orden de bytes).
    #[must_use]
    pub fn new(starts: Vec<(usize, f32)>, len: usize, width: f32) -> Self {
        Self { starts, len, width }
    }

    /// Cada carácter de `text` mide `cell`, uno ancho (CJK, emoji) dos y uno
    /// que se combina con el anterior nada, como en una terminal.
    #[must_use]
    pub fn monospace(text: &str, cell: f32) -> Self {
        let mut x = 0.0;
        let mut starts = Vec::with_capacity(text.len());
        for (byte, ch) in text.char_indices() {
            starts.push((byte, x));
            x += cell * cells(ch);
        }
        Self::new(starts, text.len(), x)
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.len
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    #[must_use]
    pub fn width(&self) -> f32 {
        self.width
    }

    /// Dónde empieza el carácter del byte `byte`; en el fin de línea, su
    /// ancho.
    #[must_use]
    pub fn x(&self, byte: usize) -> f32 {
        if byte >= self.len {
            return self.width;
        }
        let index = self.starts.partition_point(|&(start, _)| start <= byte);
        index
            .checked_sub(1)
            .and_then(|index| self.starts.get(index))
            .map_or(0.0, |&(_, x)| x)
    }

    /// Dónde termina el carácter que empieza en `byte`.
    #[must_use]
    pub fn end(&self, byte: usize) -> f32 {
        let index = self.starts.partition_point(|&(start, _)| start <= byte);
        self.starts.get(index).map_or(self.width, |&(_, x)| x)
    }

    /// El carácter que termina en `byte` o lo contiene.
    #[must_use]
    pub fn char_before(&self, byte: usize) -> usize {
        let index = self.starts.partition_point(|&(start, _)| start < byte);
        index
            .checked_sub(1)
            .and_then(|index| self.starts.get(index))
            .map_or(0, |&(start, _)| start)
    }

    /// El primer carácter que empieza en `x` o más a la derecha.
    #[must_use]
    pub fn first_from(&self, x: f32) -> Option<usize> {
        let index = self.starts.partition_point(|&(_, start)| start < x);
        self.starts.get(index).map(|&(byte, _)| byte)
    }

    /// El último carácter que termina en `x` o más a la izquierda.
    #[must_use]
    pub fn last_until(&self, x: f32) -> Option<usize> {
        let starting = self.starts.partition_point(|&(_, start)| start < x);
        let end = |index: usize| self.starts.get(index + 1).map_or(self.width, |&(_, x)| x);
        (0..starting)
            .rev()
            .take(2)
            .find(|&index| end(index) <= x)
            .and_then(|index| self.starts.get(index))
            .map(|&(byte, _)| byte)
    }

    /// El byte del carácter que ocupa `x`; a la derecha de todo, el fin de
    /// línea.
    #[must_use]
    pub fn byte_at(&self, x: f32) -> usize {
        if x >= self.width {
            return self.len;
        }
        let index = self.starts.partition_point(|&(_, start)| start <= x);
        index
            .checked_sub(1)
            .and_then(|index| self.starts.get(index))
            .map_or(0, |&(byte, _)| byte)
    }
}

fn cells(ch: char) -> f32 {
    use unicode_width::UnicodeWidthChar as _;
    if ch.width() == Some(2) {
        2.0
    } else if ch.width() == Some(0) {
        0.0
    } else {
        1.0
    }
}

/// Una línea en pantalla: dónde cae cada carácter y dónde empieza cada fila.
#[derive(Clone, Debug, PartialEq)]
pub struct ScreenLine {
    advances: Advances,
    /// El byte donde empieza cada fila; la primera, en 0.
    starts: Vec<usize>,
}

impl ScreenLine {
    /// `text` con sus caracteres donde dice `advances`, partida en filas de
    /// `wrap` píxeles si se da. Parte después de un espacio; una palabra que
    /// no cabe en una fila se parte donde llega al borde, y los espacios del
    /// final de una fila no la obligan a partirse (cuelgan del borde). Una
    /// fórmula de `style` no se parte si cabe en una fila.
    #[must_use]
    pub fn new(
        text: &str,
        advances: Advances,
        style: Option<&LineStyle>,
        wrap: Option<f32>,
    ) -> Self {
        let starts = match wrap {
            Some(width) => {
                let formulas = style.map_or(&[][..], |style| &style.math);
                let keep: Vec<Range<usize>> = formulas
                    .iter()
                    .filter(|spot| !spot.below())
                    .map(|spot| spot.range.clone())
                    .collect();
                row_starts(text, &advances, &keep, width)
            }
            None => vec![0],
        };
        Self { advances, starts }
    }

    /// Partida donde dice quien la mide: `breaks` es el byte donde empieza
    /// cada fila después de la primera, en orden.
    #[must_use]
    pub fn with_breaks(advances: Advances, breaks: impl IntoIterator<Item = usize>) -> Self {
        let starts = std::iter::once(0).chain(breaks).collect();
        Self { advances, starts }
    }

    #[must_use]
    pub fn advances(&self) -> &Advances {
        &self.advances
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.advances.len
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.advances.is_empty()
    }

    /// Cuántas filas ocupa; al menos una.
    #[must_use]
    pub fn rows(&self) -> usize {
        self.starts.len()
    }

    /// La fila del byte `byte`: el que empieza una fila es de ella, y el fin
    /// de línea, de la última.
    #[must_use]
    pub fn row_of(&self, byte: usize) -> usize {
        self.starts
            .partition_point(|&start| start <= byte)
            .saturating_sub(1)
    }

    /// Los bytes de la fila `row`.
    #[must_use]
    pub fn row(&self, row: usize) -> Range<usize> {
        let start = self.starts.get(row).copied().unwrap_or(self.len());
        let end = self.starts.get(row + 1).copied().unwrap_or(self.len());
        start..end
    }

    /// Dónde cae el byte `byte` desde el principio de su fila.
    #[must_use]
    pub fn x(&self, byte: usize) -> f32 {
        self.advances.x(byte) - self.row_left(self.row_of(byte))
    }

    /// Dónde termina, desde el principio de su fila, el carácter de `byte`.
    #[must_use]
    pub fn end(&self, byte: usize) -> f32 {
        self.advances.end(byte) - self.row_left(self.row_of(byte))
    }

    /// El carácter de la fila `row` que ocupa `x`, desde su principio; a
    /// la derecha de su texto, el último de la fila.
    #[must_use]
    pub fn byte_at(&self, row: usize, x: f32) -> usize {
        let range = self.row(row);
        let byte = self.advances.byte_at(self.row_left(row) + x.max(0.0));
        if byte < range.end || range.is_empty() {
            return byte.clamp(range.start, range.end);
        }
        self.advances.char_before(range.end).max(range.start)
    }

    fn row_left(&self, row: usize) -> f32 {
        self.advances.x(self.row(row).start)
    }
}

/// Lo que sabe quien pinta: dónde cae cada carácter de una línea con su
/// letra (la de un heading es más grande, la negrita puede ser más ancha).
pub trait ScreenLines: Debug + Send + Sync {
    /// `text` con su `style`, partida en filas de `wrap` píxeles si se da.
    fn line(&self, text: &str, style: Option<&LineStyle>, wrap: Option<f32>) -> Arc<ScreenLine>;
}

/// El área de texto de un panel: su ancho, si parte las líneas largas y lo
/// que mide una columna de texto (lo que corren `zh` y `zl`), en píxeles.
#[derive(Clone, Debug)]
pub struct Screen {
    pub width: f32,
    pub wrap: bool,
    pub column: f32,
    pub lines: Arc<dyn ScreenLines>,
}

impl Screen {
    /// `text` como se ve en este panel.
    #[must_use]
    pub fn line(&self, text: &str, style: Option<&LineStyle>) -> Arc<ScreenLine> {
        self.lines
            .line(text, style, self.wrap.then_some(self.width))
    }
}

/// La misma pantalla: el mismo medidor, no uno que mida igual.
impl PartialEq for Screen {
    fn eq(&self, other: &Self) -> bool {
        self.width.to_bits() == other.width.to_bits()
            && self.wrap == other.wrap
            && self.column.to_bits() == other.column.to_bits()
            && Arc::ptr_eq(&self.lines, &other.lines)
    }
}

/// Lo que se parte de una vez: un grafema o una fórmula entera.
struct Unit {
    range: Range<usize>,
    blank: bool,
}

/// El byte donde empieza cada fila de `text` en filas de `width`.
fn row_starts(text: &str, advances: &Advances, keep: &[Range<usize>], width: f32) -> Vec<usize> {
    let mut starts = vec![0];
    let (mut row_start, mut row_left) = (0, 0.0);
    // Dónde puede empezar la fila siguiente: tras el último espacio.
    let mut after_blank = None;
    for unit in units(text, advances, keep, width) {
        if unit.blank {
            after_blank = Some(unit.range.end);
            continue;
        }
        let start = unit.range.start;
        let overflows = |left: f32| advances.x(unit.range.end) - left > width;
        if start == row_start || !overflows(row_left) {
            continue;
        }
        let at = after_blank.filter(|&at| at > row_start).unwrap_or(start);
        starts.push(at);
        (row_start, row_left) = (at, advances.x(at));
        after_blank = None;
        if at < start && overflows(row_left) {
            starts.push(start);
            (row_start, row_left) = (start, advances.x(start));
        }
    }
    starts
}

/// Los grafemas de `text`, con cada rango de `keep` como uno solo si cabe
/// en una fila.
fn units<'a>(
    text: &'a str,
    advances: &'a Advances,
    keep: &'a [Range<usize>],
    width: f32,
) -> impl Iterator<Item = Unit> + 'a {
    let fits = move |range: &Range<usize>| advances.x(range.end) - advances.x(range.start) <= width;
    let mut kept = keep.iter().filter(move |range| fits(range)).peekable();
    let mut skip_to = 0;
    text.grapheme_indices(true)
        .filter_map(move |(start, grapheme)| {
            if start < skip_to {
                return None;
            }
            while kept.next_if(|range| range.end <= start).is_some() {}
            if let Some(range) = kept.peek().filter(|range| range.start == start) {
                skip_to = range.end;
                return Some(Unit {
                    range: (*range).clone(),
                    blank: false,
                });
            }
            Some(Unit {
                range: start..start + grapheme.len(),
                blank: grapheme.chars().all(char::is_whitespace),
            })
        })
}

#[cfg(test)]
#[path = "screen_tests.rs"]
mod tests;
