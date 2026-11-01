use std::ops::Range;

use gpui_kit::{SharedString, TestAppContext, TextRun, font, point, px};
use metalmind_config::Config;
use metalmind_syntax::{ScreenLines as _, Styles, TextLines};
use pretty_assertions::assert_eq;

use super::{Line, MathFrame, RowBox, Typeface};
use crate::editor::links::Links;
use crate::editor::markdown::runs;
use crate::editor::screen::TextScreen;
use crate::editor::text::{Faces, text_runs};
use crate::math::{Formulas, Geometry, Look};
use crate::theme::Theme;
use crate::workspace::SourceLines;

/// Lo que se ve de cada fila: su texto, trozo por trozo, con dónde cae cada
/// byte desde el principio de la fila.
type Seen = Vec<(String, Vec<f32>)>;

/// Una línea con una fórmula y un link, partida por el motor en filas de
/// 300 px y con su primera fila arriba de la ventana: la vista la corta de
/// la forma de la línea entera. Cada fila se ve igual que si se le diera
/// forma sola, como antes.
#[rstest::rstest]
#[case::con_formulas(false)]
#[case::con_la_fuente(true)]
fn una_linea_partida_que_asoma_se_ve_como_sus_filas(#[case] show_source: bool) {
    let mut cx = TestAppContext::single();
    let window = cx.add_empty_window();
    window.update(|window, _| {
        let text = "Una frase larga con [[un link]] y la fórmula $\\frac{a}{b} + x^2$ \
                    en medio, y **negritas** que siguen hasta pasar del borde varias veces.";
        let theme = Theme::with_font(&Config::defaults().unwrap(), "Mono".into());
        let typeface = Typeface {
            font: font("Mono"),
            size: px(15.),
        };
        let faces = Faces::new(typeface.font.clone(), typeface.size, &theme);
        let mut styles = Styles::new(1);
        styles.ensure(usize::MAX, &TextLines::new(text));
        let style = styles.line(0);
        let screen = TextScreen::new(window.text_system().clone(), faces.clone());
        let shown = screen.line(text, style, Some(300.));
        assert!(shown.rows() > 2, "la línea se parte: {}", shown.rows());
        let mut top = px(0.);
        let rows = (0..shown.rows())
            .map(|row| {
                let row = RowBox {
                    source: shown.row(row),
                    top,
                    height: px(20.),
                };
                top += px(20.);
                row
            })
            .collect::<Vec<_>>();
        let height = top;
        let line = Line {
            number: 0,
            text,
            style,
            show_source,
            origin: point(px(0.), px(-20.)),
            height,
            rows,
            width: px(300.),
        };
        let geometry = Geometry {
            size: 15.,
            line_height: 20.,
            ascent: 12.,
            descent: 4.,
            block_margin: 4.,
        };
        let math = MathFrame {
            formulas: &Formulas::default(),
            geometry,
            look: Look {
                size: 15.,
                scale: 1.,
                color: [1.; 4],
                padding: 1.,
            },
            source: SourceLines::only(0),
        };
        let (laid, _) = line.lay(&typeface, &theme, &Links::default(), &math, window);

        let seen: Seen = laid
            .rows
            .iter()
            .flat_map(|row| &row.pieces)
            .filter_map(|piece| {
                let shaped = piece.text.as_ref()?;
                let xs = (0..=shaped.len())
                    .filter(|&byte| shaped.text.is_char_boundary(byte))
                    .map(|byte| f32::from(piece.x + shaped.x_for_index(byte)))
                    .collect();
                Some((shaped.text.to_string(), xs))
            })
            .collect();
        // Lo de antes: cada trozo con su propia forma, desde donde termina
        // lo anterior de su fila.
        let (size, weight) = faces.line(style.map(|style| style.kind));
        let all = text_runs(
            &runs(text.len(), style, |_| true, &[]),
            None,
            &faces,
            weight,
        );
        let alone: Seen = laid
            .rows
            .iter()
            .flat_map(|row| &row.pieces)
            .filter(|piece| piece.text.is_some())
            .map(|piece| {
                let piece_text = &text[piece.source.clone()];
                let shaped = window.text_system().shape_line(
                    SharedString::from(piece_text.to_owned()),
                    size,
                    &clip(&all, piece.source.clone()),
                    None,
                );
                let xs = (0..=piece_text.len())
                    .filter(|&byte| piece_text.is_char_boundary(byte))
                    .map(|byte| f32::from(piece.x + shaped.x_for_index(byte)))
                    .collect();
                (piece_text.to_owned(), xs)
            })
            .collect();
        assert_eq!(seen, alone);
        for row in &laid.rows {
            let pieces: Vec<_> = row
                .pieces
                .iter()
                .map(|piece| piece.source.clone())
                .collect();
            let covered = pieces.iter().map(ExactSizeIterator::len).sum::<usize>();
            assert_eq!(
                covered,
                row.source.len(),
                "la fila pinta sus bytes: {pieces:?}"
            );
        }
    });
}

/// Los tramos de `runs` que caen en `range`, recortados a él.
fn clip(runs: &[TextRun], range: Range<usize>) -> Vec<TextRun> {
    let mut start = 0;
    runs.iter()
        .filter_map(|run| {
            let (from, to) = (start.max(range.start), (start + run.len).min(range.end));
            start += run.len;
            (from < to).then(|| TextRun {
                len: to - from,
                ..run.clone()
            })
        })
        .collect()
}
