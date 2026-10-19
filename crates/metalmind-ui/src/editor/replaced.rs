//! Cómo se parte una línea que no muestra su fuente: los tramos de texto
//! entre las fórmulas que los reemplazan, sin nada de GPUI. La fórmula de un
//! bloque de varias líneas no está aquí: va debajo de la línea.

use std::ops::Range;

use metalmind_syntax::{MathPlace, MathSpot};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Part {
    Text(Range<usize>),
    /// La fórmula `spot` de la línea, en lugar de los bytes `source`.
    Formula {
        spot: usize,
        source: Range<usize>,
    },
}

/// Las partes de una línea de `len` bytes con `spots`; `composes` dice si
/// la fórmula de ese índice compone (si no, se ve su fuente, como texto).
pub fn parts(len: usize, spots: &[MathSpot], composes: impl Fn(usize) -> bool) -> Vec<Part> {
    let mut out = Vec::new();
    let mut at = 0;
    for (index, spot) in spots.iter().enumerate() {
        if spot.below() || !composes(index) {
            continue;
        }
        if spot.place == MathPlace::Alone {
            // Lo de alrededor es solo espacio: la fórmula es la línea.
            return vec![Part::Formula {
                spot: index,
                source: 0..len,
            }];
        }
        if spot.range.start > at {
            out.push(Part::Text(at..spot.range.start));
        }
        out.push(Part::Formula {
            spot: index,
            source: spot.range.clone(),
        });
        at = spot.range.end;
    }
    if at < len || out.is_empty() {
        out.push(Part::Text(at..len));
    }
    out
}

#[cfg(test)]
mod tests {
    use metalmind_syntax::{Lines, Styles, TextLines};
    use pretty_assertions::assert_eq;

    use super::*;

    /// La línea `line` de `text` con sus fórmulas entre `⟦ ⟧`; las que
    /// tienen `\bad` no componen.
    fn render(text: &str, line: usize) -> String {
        let lines = TextLines::new(text);
        let mut styles = Styles::new(lines.line_count());
        styles.ensure(usize::MAX, &lines);
        let source = lines.line(line);
        let spots = &styles.line(line).unwrap().math;
        parts(source.len(), spots, |index| {
            !spots[index].latex.contains(r"\bad")
        })
        .into_iter()
        .map(|part| match part {
            Part::Text(range) => source[range].to_owned(),
            Part::Formula { source: range, .. } => format!("⟦{}⟧", &source[range]),
        })
        .collect()
    }

    #[test]
    fn cada_formula_reemplaza_su_tramo() {
        assert_eq!(render("a $x$ b $y$", 0), "a ⟦$x$⟧ b ⟦$y$⟧");
        assert_eq!(render("$x$", 0), "⟦$x$⟧");
        assert_eq!(render("texto $$x$$ más", 0), "texto ⟦$$x$$⟧ más");
    }

    #[test]
    fn un_bloque_solo_en_su_linea_es_la_linea() {
        assert_eq!(render("  $$x$$ ", 0), "⟦  $$x$$ ⟧");
    }

    #[test]
    fn la_que_no_compone_se_queda_como_texto() {
        assert_eq!(render(r"a $\bad$ b $y$", 0), r"a $\bad$ b ⟦$y$⟧");
        assert_eq!(render(r"$$\bad$$", 0), r"$$\bad$$");
    }

    #[test]
    fn el_cierre_de_un_bloque_queda_como_texto() {
        assert_eq!(render("$$\nx\n$$ y $z$", 2), "$$ y ⟦$z$⟧");
        assert_eq!(render("", 0), "");
    }
}
