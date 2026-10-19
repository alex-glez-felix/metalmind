//! Los anchos de los grafemas de una línea con su suma acumulada, para
//! que el ancho de cualquier tramo salga en tiempo constante y recortar
//! una fila larga sea lineal.

use std::ops::Range;

pub(super) struct Widths {
    /// `prefix[i]` es lo que miden los primeros `i` grafemas.
    prefix: Vec<f32>,
}

impl Widths {
    pub(super) fn new(widths: &[f32]) -> Self {
        let prefix = std::iter::once(0.0)
            .chain(widths.iter().scan(0.0, |total, width| {
                *total += width;
                Some(*total)
            }))
            .collect();
        Self { prefix }
    }

    pub(super) fn len(&self) -> usize {
        self.prefix.len() - 1
    }

    /// Lo que mide el tramo `range`.
    pub(super) fn sum(&self, range: Range<usize>) -> f32 {
        self.prefix[range.end] - self.prefix[range.start]
    }

    pub(super) fn at(&self, grapheme: usize) -> f32 {
        self.sum(grapheme..grapheme + 1)
    }

    pub(super) fn total(&self) -> f32 {
        self.sum(0..self.len())
    }
}
