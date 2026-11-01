//! Lo que un picker muestra junto a la lista sobre la fila marcada, como la
//! vista previa de Telescope: unas líneas de texto, una de ellas la del
//! resultado, o un aviso.

use std::ops::Range;

use gpui_kit::SharedString;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Preview {
    Lines {
        lines: Vec<PreviewLine>,
        /// La línea del resultado, en `lines`, que se pinta marcada y se
        /// centra; `None` si el texto ya no llega a ella, y entonces se ve
        /// su final, lo más cerca de donde estaba.
        focus: Option<usize>,
    },
    /// Por qué no hay líneas, apagado.
    Message(SharedString),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PreviewLine {
    /// Desde 1, como en el margen del editor.
    pub number: usize,
    pub text: String,
    /// Los bytes de `text` que coinciden, crecientes y sin solaparse.
    pub highlights: Vec<Range<usize>>,
}

/// Las líneas de `count` que se ven con `focus` lo más centrado posible, en
/// un panel de `fit` líneas; sin `focus`, las del final.
#[must_use]
pub fn visible_lines(count: usize, focus: Option<usize>, fit: usize) -> Range<usize> {
    let fit = fit.min(count);
    let start = focus
        .map_or(count, |focus| {
            focus.saturating_sub(fit.saturating_sub(1) / 2)
        })
        .min(count - fit);
    start..start + fit
}

/// El ancho de la vista previa a la derecha de una lista de `list`, con a lo
/// sumo `max_share` de una ventana de `window`: el que pide el tema
/// (`wanted`) o lo que quede; `None` si queda menos de `min`.
#[must_use]
pub fn preview_width(window: f32, max_share: f32, list: f32, wanted: f32, min: f32) -> Option<f32> {
    let room = window * max_share - list;
    (room >= min).then(|| wanted.min(room))
}

/// Cuántas líneas de `line` caben enteras en `height`.
#[must_use]
#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "se trunca a propósito: líneas de un panel, acotadas y nunca negativas"
)]
pub fn lines_that_fit(height: f32, line: f32) -> usize {
    if line <= 0. {
        return 0;
    }
    (height / line).max(0.).floor() as usize
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;

    use super::*;

    #[rstest::rstest]
    #[case::ancha(1920., Some(560.))]
    #[case::de_1280_se_angosta(1280., Some(512.))]
    #[case::justo(1067., Some(320.3))]
    #[case::angosta(1000., None)]
    fn la_vista_previa_cabe_a_la_derecha_o_no_se_ve(
        #[case] window: f32,
        #[case] expected: Option<f32>,
    ) {
        let width = preview_width(window, 0.9, 640., 560., 320.);
        match (width, expected) {
            (Some(width), Some(expected)) => assert!((width - expected).abs() < 0.01, "{width}"),
            _ => assert_eq!(width, expected),
        }
    }

    #[test]
    fn solo_cuenta_las_lineas_enteras() {
        assert_eq!(lines_that_fit(396., 21.), 18);
        assert_eq!(lines_that_fit(20., 21.), 0);
        assert_eq!(lines_that_fit(100., 0.), 0);
    }

    #[rstest::rstest]
    #[case::centrada(61, Some(30), 11, 25..36)]
    #[case::al_principio(61, Some(2), 11, 0..11)]
    #[case::al_final(61, Some(59), 11, 50..61)]
    #[case::caben_todas(5, Some(4), 11, 0..5)]
    #[case::sin_resultado(61, None, 11, 50..61)]
    #[case::par(20, Some(10), 4, 9..13)]
    #[case::vacia(0, None, 11, 0..0)]
    fn centra_la_linea_del_resultado(
        #[case] count: usize,
        #[case] focus: Option<usize>,
        #[case] fit: usize,
        #[case] expected: Range<usize>,
    ) {
        assert_eq!(visible_lines(count, focus, fit), expected);
    }
}
