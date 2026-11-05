use std::ops::Range;

use gpui_kit::{Bounds, SharedString, TestAppContext, TextRun, Window, font, point, px, size};
use metalmind_config::Config;
use metalmind_syntax::{ScreenLines as _, Styles, TextLines};
use pretty_assertions::assert_eq;

use super::{Line, MathFrame, RowBox, Typeface};
use crate::editor::layout::LaidLine;
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
        let laid = lay(text, show_source, 300., window);
        assert!(
            laid.rows.len() > 2,
            "la línea se parte: {}",
            laid.rows.len()
        );
        let theme = theme();
        let faces = Faces::new(font("Mono"), px(15.), &theme);
        let mut styles = Styles::new(1);
        styles.ensure(usize::MAX, &TextLines::new(text));
        let style = styles.line(0);

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

/// Una matriz: unas 110 columnas de fuente y una imagen angosta.
const MATRIX: &str = r"A = \begin{bmatrix} a_{11} & \cdots & a_{1n} \\ \vdots & \ddots & \vdots \\ a_{m1} & \cdots & a_{mn} \end{bmatrix}";

/// Si lo que se ve de una línea es una sola imagen y ningún texto.
fn is_image(laid: &LaidLine) -> bool {
    let no_text = laid
        .rows
        .iter()
        .flat_map(|row| &row.pieces)
        .all(|piece| piece.text.is_none());
    laid.formulas.len() == 1 && no_text
}

/// Sola en su línea, partida en filas y más angosta que ellas: la imagen,
/// centrada sobre las filas que tiene; con el cursor, la fuente en las mismas.
#[rstest::rstest]
#[case::bloque(format!("$${MATRIX}$$"))]
#[case::en_linea(format!("${MATRIX}$"))]
fn una_formula_sola_partida_se_pinta_sobre_sus_filas(#[case] text: String) {
    let mut cx = TestAppContext::single();
    let window = cx.add_empty_window();
    window.update(|window, _| {
        let image = lay(&text, false, 400., window);
        let source = lay(&text, true, 400., window);
        assert!(image.rows.len() > 1, "la fuente se parte");
        assert!(is_image(&image));
        assert!(source.formulas.is_empty(), "con el cursor, la fuente");
        let rows = |laid: &LaidLine| -> Vec<_> {
            let rows = laid.rows.iter();
            rows.map(|row| (row.source.clone(), row.top, row.height))
                .collect()
        };
        assert_eq!(rows(&image), rows(&source), "las mismas filas");

        let bounds = image.formulas[0].bounds;
        let above = bounds.top() - image.origin.y;
        let below = image.origin.y + image.height - bounds.bottom();
        assert!((f32::from(above) - f32::from(below)).abs() < 1.0);
    });
}

/// Con sangría, la sangría es parte de la fórmula: la imagen va desde el
/// borde, dentro de la línea, y un clic en cualquiera de sus filas va al
/// principio de la línea, también con la sangría sola en la primera fila.
#[test]
fn una_formula_sola_con_sangria_es_la_linea_entera() {
    let mut cx = TestAppContext::single();
    let window = cx.add_empty_window();
    window.update(|window, _| {
        for indent in [4, 12, 60] {
            let text = format!("{}${MATRIX}$", " ".repeat(indent));
            let laid = lay(&text, false, 250., window);
            assert!(laid.rows.len() > 1, "la línea se parte");
            assert!(is_image(&laid), "sangría {indent}");
            let bounds = laid.formulas[0].bounds;
            let line = Bounds::new(laid.origin, size(px(250.), laid.height));
            assert!(
                line.dilate(px(1.)).contains(&bounds.origin)
                    && line.dilate(px(1.)).contains(&bounds.bottom_right()),
                "sangría {indent}: {bounds:?} fuera de {line:?}"
            );
            for (index, row) in laid.rows.iter().enumerate() {
                let last = index + 1 == laid.rows.len();
                let x = bounds.left() - laid.origin.x + px(1.);
                assert_eq!(row.closest_index_for_x(x, last), 0, "fila {index}");
            }
        }
    });
}

/// Un clic en cualquier fila de la imagen va al principio de la fórmula, y
/// un tramo de cualquier fila la cubre a lo ancho.
#[test]
fn toda_la_imagen_partida_es_la_misma_formula() {
    let mut cx = TestAppContext::single();
    let window = cx.add_empty_window();
    window.update(|window, _| {
        let text = format!("$${MATRIX}$$");
        let laid = lay(&text, false, 400., window);
        let piece = laid.rows[0].pieces[0].clone();
        for (index, row) in laid.rows.iter().enumerate() {
            let last = index + 1 == laid.rows.len();
            assert_eq!(row.closest_index_for_x(piece.x, last), 0, "fila {index}");
            assert_eq!(row.x_for_index(row.source.start), piece.x, "fila {index}");
            assert_eq!(
                row.x_for_end(row.source.end),
                piece.x + piece.width,
                "fila {index}"
            );
        }
    });
}

/// Más ancha que la fila, o partida entre texto, se ve su fuente.
#[rstest::rstest]
#[case::mas_ancha_que_la_fila(format!("$${MATRIX}$$"), 60.)]
#[case::entre_texto(format!("Antes ${MATRIX}$ y después."), 400.)]
fn una_formula_partida_que_no_cabe_o_va_entre_texto_se_ve_como_fuente(
    #[case] text: String,
    #[case] wrap: f32,
) {
    let mut cx = TestAppContext::single();
    let window = cx.add_empty_window();
    window.update(|window, _| {
        let laid = lay(&text, false, wrap, window);
        assert!(laid.rows.len() > 1, "la fuente se parte");
        assert!(laid.formulas.is_empty());
        let covered = laid.rows.iter().flat_map(|row| &row.pieces);
        assert!(covered.clone().all(|piece| piece.text.is_some()));
    });
}

fn theme() -> Theme {
    Theme::with_font(&Config::defaults().unwrap(), "Mono".into())
}

/// `text` partida por el motor en filas de `wrap` px y armada como la pinta
/// la vista, con su primera fila arriba de la ventana.
fn lay(text: &str, show_source: bool, wrap: f32, window: &Window) -> LaidLine {
    let theme = theme();
    let typeface = Typeface {
        font: font("Mono"),
        size: px(15.),
    };
    let faces = Faces::new(typeface.font.clone(), typeface.size, &theme);
    let mut styles = Styles::new(1);
    styles.ensure(usize::MAX, &TextLines::new(text));
    let style = styles.line(0);
    let screen = TextScreen::new(window.text_system().clone(), faces.clone());
    let shown = screen.line(text, style, Some(wrap));
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
        width: px(wrap),
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
    line.lay(&typeface, &theme, &Links::default(), &math, window)
        .0
}

/// Lo que mide en la vista cada carácter de `text`, sin partir: dónde
/// empieza cada uno y dónde termina la línea.
fn starts(text: &str, window: &Window) -> Vec<f32> {
    let laid = lay(text, false, 10_000., window);
    let row = &laid.rows[0];
    text.char_indices()
        .map(|(byte, _)| byte)
        .chain([text.len()])
        .map(|byte| f32::from(row.x_for_index(byte)))
        .collect()
}

/// Un carácter ancho ocupa dos celdas exactas de la letra del editor,
/// también en un heading, como en una terminal: "日本語" mide lo que
/// "abcdef", y el cursor y lo que sigue caen en las mismas celdas.
#[test]
#[expect(
    clippy::float_cmp,
    reason = "el motor y la vista salen de las mismas cuentas"
)]
fn un_caracter_ancho_ocupa_dos_celdas() {
    let mut cx = TestAppContext::single();
    let window = cx.add_empty_window();
    window.update(|window, _| {
        for prefix in ["", "# "] {
            let wide = starts(&format!("{prefix}日本語|x"), window);
            let narrow = starts(&format!("{prefix}abcdef|x"), window);
            // 日 en a, 本 en c, 語 en e; lo que sigue, en su lugar.
            let mut cells: Vec<usize> = (0..prefix.len()).collect();
            let after = prefix.len();
            cells.extend([after, after + 2, after + 4, after + 6, after + 7, after + 8]);
            let expected: Vec<f32> = cells.into_iter().map(|index| narrow[index]).collect();
            assert_eq!(wide, expected, "{prefix:?}");
            let screen = TextScreen::new(
                window.text_system().clone(),
                Faces::new(font("Mono"), px(15.), &theme()),
            );
            let measured = |text: &str| {
                let mut styles = Styles::new(1);
                styles.ensure(usize::MAX, &TextLines::new(text));
                screen.line(text, styles.line(0), None).advances().width()
            };
            assert_eq!(
                measured(&format!("{prefix}日本語|x")),
                *wide.last().unwrap(),
                "el motor mide lo mismo que se pinta"
            );
        }
    });
}

/// Un clic en la mitad izquierda de las dos celdas de un carácter ancho cae
/// en él y uno en la derecha, en el siguiente, aunque su glifo sea más
/// angosto que sus celdas.
#[test]
fn un_clic_en_un_ancho_cae_en_la_celda_mas_cercana() {
    let mut cx = TestAppContext::single();
    let window = cx.add_empty_window();
    window.update(|window, _| {
        let text = "日本語";
        let xs = starts(text, window);
        let laid = lay(text, false, 10_000., window);
        let row = &laid.rows[0];
        for (index, (byte, _)) in text.char_indices().enumerate() {
            let (left, right) = (xs[index], xs[index + 1]);
            let quarter = (right - left) / 4.;
            assert_eq!(
                row.closest_index_for_x(px(left + quarter), true),
                byte,
                "cuarto izquierdo de {byte}"
            );
            assert_eq!(
                row.closest_index_for_x(px(right - quarter), true),
                byte + 3,
                "cuarto derecho de {byte}"
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
