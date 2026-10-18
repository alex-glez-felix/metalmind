//! Diff por líneas entre dos textos y el plan para pasar de uno a otro.
//! Todo en bytes; la UI convierte a columnas en su borde.

use std::ops::Range;
use std::time::{Duration, Instant};

use similar::{Algorithm, DiffOp, capture_diff_slices_deadline};

/// Un diff que se pasa de este tiempo devuelve uno más grueso pero correcto.
const DEADLINE: Duration = Duration::from_millis(100);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChunkKind {
    Equal,
    Delete,
    Insert,
    Replace,
}

/// Un tramo de líneas enteras: `old` en el texto viejo y `new` en el nuevo,
/// en bytes. En `Delete`, `new` es vacío y marca dónde estaría; en `Insert`,
/// lo mismo con `old`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiffChunk {
    pub kind: ChunkKind,
    pub old: Range<usize>,
    pub new: Range<usize>,
}

/// Dos textos y los tramos que los cubren de principio a fin, en orden.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextDiff {
    pub old: String,
    pub new: String,
    pub chunks: Vec<DiffChunk>,
}

impl TextDiff {
    #[must_use]
    pub fn new(old: String, new: String) -> Self {
        let chunks = line_chunks(&old, &new);
        Self { old, new, chunks }
    }
}

/// Un reemplazo sobre el texto actual: `range` en bytes del texto antes de
/// aplicar ningún cambio del plan.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RestoreEdit {
    pub range: Range<usize>,
    pub text: String,
}

/// Lo que hay que cambiar del texto actual para que quede igual a una
/// versión. Solo toca las líneas distintas, así que la UI lo aplica como una
/// sola transacción que se deshace con `u` y no mueve lo que no cambió.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RestorePlan {
    /// En orden y sin solaparse.
    pub edits: Vec<RestoreEdit>,
}

impl RestorePlan {
    #[must_use]
    pub fn new(current: &str, target: &str) -> Self {
        let edits = line_chunks(current, target)
            .into_iter()
            .filter(|chunk| chunk.kind != ChunkKind::Equal)
            .map(|chunk| RestoreEdit {
                range: chunk.old,
                text: target[chunk.new].to_owned(),
            })
            .collect();
        Self { edits }
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.edits.is_empty()
    }

    /// El texto que queda al aplicar el plan sobre `current`, el mismo con el
    /// que se calculó.
    #[must_use]
    pub fn apply(&self, current: &str) -> String {
        let mut out = String::with_capacity(current.len());
        let mut done = 0;
        for edit in &self.edits {
            out.push_str(&current[done..edit.range.start]);
            out.push_str(&edit.text);
            done = edit.range.end;
        }
        out.push_str(&current[done..]);
        out
    }
}

/// Los tramos del diff por líneas de `old` a `new`. Cada línea lleva su `\n`,
/// así que los tramos reproducen los dos textos byte a byte.
pub(super) fn line_chunks(old: &str, new: &str) -> Vec<DiffChunk> {
    let old_lines = Lines::new(old);
    let new_lines = Lines::new(new);
    let ops = capture_diff_slices_deadline(
        Algorithm::Myers,
        &old_lines.lines,
        &new_lines.lines,
        Some(Instant::now() + DEADLINE),
    );
    ops.iter()
        .map(|op| {
            let (kind, old_range, new_range) = match *op {
                DiffOp::Equal {
                    old_index,
                    new_index,
                    len,
                } => (
                    ChunkKind::Equal,
                    old_index..old_index + len,
                    new_index..new_index + len,
                ),
                DiffOp::Delete {
                    old_index,
                    old_len,
                    new_index,
                } => (
                    ChunkKind::Delete,
                    old_index..old_index + old_len,
                    new_index..new_index,
                ),
                DiffOp::Insert {
                    old_index,
                    new_index,
                    new_len,
                } => (
                    ChunkKind::Insert,
                    old_index..old_index,
                    new_index..new_index + new_len,
                ),
                DiffOp::Replace {
                    old_index,
                    old_len,
                    new_index,
                    new_len,
                } => (
                    ChunkKind::Replace,
                    old_index..old_index + old_len,
                    new_index..new_index + new_len,
                ),
            };
            DiffChunk {
                kind,
                old: old_lines.bytes(old_range),
                new: new_lines.bytes(new_range),
            }
        })
        .collect()
}

/// Las líneas de un texto, con su `\n`, y dónde empieza cada una.
struct Lines<'a> {
    lines: Vec<&'a str>,
    starts: Vec<usize>,
}

impl<'a> Lines<'a> {
    fn new(text: &'a str) -> Self {
        let lines: Vec<&str> = text.split_inclusive('\n').collect();
        let mut starts = Vec::with_capacity(lines.len() + 1);
        let mut at = 0;
        for line in &lines {
            starts.push(at);
            at += line.len();
        }
        starts.push(at);
        Self { lines, starts }
    }

    fn bytes(&self, lines: Range<usize>) -> Range<usize> {
        self.starts[lines.start]..self.starts[lines.end]
    }
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;
    use proptest::prelude::*;

    use super::*;

    #[test]
    fn los_tramos_van_por_lineas_y_en_bytes() {
        let diff = TextDiff::new("año\nb\nc".to_owned(), "año\nB\nc\nd\n".to_owned());
        let kinds: Vec<_> = diff
            .chunks
            .iter()
            .map(|chunk| {
                (
                    chunk.kind,
                    &diff.old[chunk.old.clone()],
                    &diff.new[chunk.new.clone()],
                )
            })
            .collect();
        assert_eq!(
            kinds,
            [
                (ChunkKind::Equal, "año\n", "año\n"),
                (ChunkKind::Replace, "b\nc", "B\nc\nd\n"),
            ]
        );
    }

    #[test]
    fn restaurar_solo_toca_lo_distinto() {
        let plan = RestorePlan::new("uno\ndos\ntres\n", "uno\n2\ntres\n");
        assert_eq!(
            plan.edits,
            [RestoreEdit {
                range: 4..8,
                text: "2\n".to_owned()
            }]
        );
        assert!(RestorePlan::new("igual", "igual").is_empty());
    }

    fn text() -> impl Strategy<Value = String> {
        prop::collection::vec(
            prop::sample::select(vec!["a", "b", "ñ", "\n", "\r\n", ""]),
            0..40,
        )
        .prop_map(|parts| parts.concat())
    }

    proptest! {
        #[test]
        fn el_plan_lleva_exactamente_a_la_version(current in text(), target in text()) {
            let plan = RestorePlan::new(&current, &target);
            prop_assert_eq!(plan.apply(&current), target);
        }

        #[test]
        fn los_tramos_cubren_los_dos_textos(old in text(), new in text()) {
            let diff = TextDiff::new(old.clone(), new.clone());
            let rebuilt_old: String = diff.chunks.iter().map(|c| &old[c.old.clone()]).collect();
            let rebuilt_new: String = diff.chunks.iter().map(|c| &new[c.new.clone()]).collect();
            prop_assert_eq!(rebuilt_old, old);
            prop_assert_eq!(rebuilt_new, new);
        }
    }
}
