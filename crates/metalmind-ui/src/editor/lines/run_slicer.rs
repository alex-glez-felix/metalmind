use std::ops::Range;

use gpui_kit::TextRun;

/// Los tramos de estilo de una línea, recortados a trozos que van de
/// izquierda a derecha: cada tramo se mira una sola vez por línea.
pub(super) struct RunSlicer<'r> {
    runs: &'r [TextRun],
    /// El primer tramo que puede tocar el próximo trozo, y dónde empieza.
    next: usize,
    start: usize,
}

impl<'r> RunSlicer<'r> {
    pub(super) fn new(runs: &'r [TextRun]) -> Self {
        Self {
            runs,
            next: 0,
            start: 0,
        }
    }

    /// Los tramos que caen en `range`, recortados a él. `range` empieza
    /// donde terminó el anterior o después.
    pub(super) fn take(&mut self, range: Range<usize>) -> Vec<TextRun> {
        let mut out = Vec::new();
        while let Some(run) = self.runs.get(self.next) {
            let end = self.start + run.len;
            let (from, to) = (self.start.max(range.start), end.min(range.end));
            if from < to {
                out.push(TextRun {
                    len: to - from,
                    ..run.clone()
                });
            }
            if end > range.end {
                break;
            }
            self.next += 1;
            self.start = end;
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use gpui_kit::{Font, TextRun};
    use pretty_assertions::assert_eq;

    use super::RunSlicer;

    fn run(len: usize) -> TextRun {
        TextRun {
            len,
            font: Font::default(),
            color: gpui_kit::Hsla::default(),
            background_color: None,
            underline: None,
            strikethrough: None,
        }
    }

    #[test]
    fn los_trozos_se_recortan_de_los_tramos_en_orden() {
        let runs = [run(3), run(4), run(5)];
        let mut slicer = RunSlicer::new(&runs);
        let lens = |runs: Vec<TextRun>| runs.iter().map(|run| run.len).collect::<Vec<_>>();
        assert_eq!(lens(slicer.take(0..2)), [2]);
        assert_eq!(lens(slicer.take(5..9)), [2, 2]);
        assert_eq!(lens(slicer.take(9..12)), [3]);
        assert_eq!(lens(slicer.take(12..12)), Vec::<usize>::new());
    }
}
