//! RaTeX detrás de dos funciones: medir una fórmula (su layout, µs) y
//! pintarla (ms, en el fondo). Sin GPUI, y sin pánicos: RaTeX es joven y
//! tiene `unwrap` adentro, así que cualquier pánico suyo se contiene y se
//! vuelve un error que se muestra como el de una fórmula mal escrita.

use std::any::Any;
use std::panic::{self, AssertUnwindSafe};

use ratex_layout::LayoutOptions;
use ratex_render::RenderOptions;
use ratex_types::{Color, DisplayList, MathStyle};

/// Lo que mide una fórmula, en em de la letra con que se compone.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Metrics {
    pub width: f32,
    /// Sobre la línea base.
    pub height: f32,
    /// Bajo la línea base.
    pub depth: f32,
}

/// Por qué una fórmula no se ve.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum MathError {
    /// El LaTeX está mal escrito: el mensaje del parser de RaTeX.
    #[error("{0}")]
    Parse(String),
    #[error("RaTeX no pudo componer la fórmula: {0}")]
    Layout(String),
    #[error("RaTeX no pudo pintar la fórmula: {0}")]
    Render(String),
    #[error("RaTeX falló con esta fórmula: {0}")]
    Panic(String),
}

/// Cómo se pinta: tamaño de la letra en píxeles lógicos, escala de la
/// pantalla y color (RGBA de 0 a 1).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Look {
    pub size: f32,
    pub scale: f32,
    pub color: [f32; 4],
}

/// Los píxeles de una fórmula, en BGRA sin premultiplicar (lo que guarda el
/// `RenderImage` de GPUI), con `PADDING` píxeles lógicos alrededor.
#[derive(Clone, Debug)]
pub struct Picture {
    pub width: u32,
    pub height: u32,
    pub bgra: Vec<u8>,
}

/// Lo que se deja alrededor de la fórmula al pintarla, en píxeles lógicos:
/// el suavizado de los bordes cae ahí y no se recorta.
pub const PADDING: f32 = 1.0;

/// El layout de la fórmula, sin pintarla.
///
/// # Errors
/// Si RaTeX no la entiende o no puede componerla.
pub fn measure(latex: &str, display: bool) -> Result<Metrics, MathError> {
    let list = compose(latex, display, Color::BLACK)?;
    Ok(metrics(&list))
}

/// La fórmula pintada.
///
/// # Errors
/// Si RaTeX no la entiende, no puede componerla o pintarla.
pub fn render(latex: &str, display: bool, look: Look) -> Result<Picture, MathError> {
    let [r, g, b, a] = look.color;
    let list = compose(latex, display, Color::new(r, g, b, a))?;
    let options = RenderOptions {
        font_size: look.size,
        padding: PADDING,
        background_color: Color::new(0.0, 0.0, 0.0, 0.0),
        font_dir: String::new(),
        device_pixel_ratio: look.scale,
    };
    let png = contain(|| ratex_render::render_to_png(&list, &options).map_err(MathError::Render))?;
    decode(&png)
}

fn compose(latex: &str, display: bool, color: Color) -> Result<DisplayList, MathError> {
    contain(|| {
        let nodes =
            ratex_parser::parse(latex).map_err(|error| MathError::Parse(error.to_string()))?;
        let options = LayoutOptions {
            style: if display {
                MathStyle::Display
            } else {
                MathStyle::Text
            },
            color,
            ..LayoutOptions::default()
        };
        let root = ratex_layout::layout(&nodes, &options);
        ratex_layout::validate_layout(&root).map_err(MathError::Layout)?;
        Ok(ratex_layout::to_display_list(&root))
    })
}

#[expect(
    clippy::cast_possible_truncation,
    reason = "lo que mide una fórmula en em cabe en f32"
)]
fn metrics(list: &DisplayList) -> Metrics {
    Metrics {
        width: list.width as f32,
        height: list.height as f32,
        depth: list.depth as f32,
    }
}

/// Corre `work` y convierte un pánico en un error.
fn contain<T>(work: impl FnOnce() -> Result<T, MathError>) -> Result<T, MathError> {
    panic::catch_unwind(AssertUnwindSafe(work))
        .unwrap_or_else(|payload| Err(MathError::Panic(panic_message(payload.as_ref()))))
}

fn panic_message(payload: &(dyn Any + Send)) -> String {
    payload
        .downcast_ref::<&str>()
        .map(|message| (*message).to_owned())
        .or_else(|| payload.downcast_ref::<String>().cloned())
        .unwrap_or_default()
}

/// RaTeX solo entrega PNG; se decodifica a BGRA para GPUI.
fn decode(png: &[u8]) -> Result<Picture, MathError> {
    let failed = |error: png::DecodingError| MathError::Render(error.to_string());
    let mut reader = png::Decoder::new(std::io::Cursor::new(png))
        .read_info()
        .map_err(failed)?;
    let mut pixels = vec![0; reader.output_buffer_size()];
    let info = reader.next_frame(&mut pixels).map_err(failed)?;
    if info.color_type != png::ColorType::Rgba || info.bit_depth != png::BitDepth::Eight {
        return Err(MathError::Render(format!(
            "PNG {:?} de {:?} bits, se esperaba RGBA de 8",
            info.color_type, info.bit_depth
        )));
    }
    pixels.truncate(info.buffer_size());
    for pixel in pixels.as_chunks_mut::<4>().0 {
        pixel.swap(0, 2);
    }
    Ok(Picture {
        width: info.width,
        height: info.height,
        bgra: pixels,
    })
}

#[cfg(test)]
mod tests {
    use metalmind_perf::FORMULAS;

    use super::*;

    const LOOK: Look = Look {
        size: 16.0,
        scale: 1.0,
        color: [1.0, 1.0, 1.0, 1.0],
    };

    #[test]
    fn las_22_formulas_del_spike_componen() {
        for formula in FORMULAS {
            let metrics = measure(formula.latex, formula.display);
            assert!(metrics.is_ok(), "{}: {metrics:?}", formula.name);
            let picture = render(formula.latex, formula.display, LOOK);
            assert!(picture.is_ok(), "{}: {picture:?}", formula.name);
        }
    }

    #[test]
    fn una_fraccion_es_mas_alta_que_una_letra() {
        let letter = measure("x", false).unwrap();
        let fraction = measure(r"\frac{a}{b}", false).unwrap();
        assert!(fraction.height + fraction.depth > letter.height + letter.depth);
    }

    #[test]
    fn el_latex_mal_escrito_es_un_error_con_el_mensaje_del_parser() {
        let error = measure(r"\frac{a", false).unwrap_err();
        assert!(matches!(error, MathError::Parse(ref message) if !message.is_empty()));
        assert!(matches!(
            measure(r"\comandoquenoexiste", false),
            Err(MathError::Parse(_))
        ));
    }

    #[test]
    fn la_imagen_mide_la_formula_a_su_escala() {
        let metrics = measure("x^2", false).unwrap();
        let picture = render("x^2", false, Look { scale: 2.0, ..LOOK }).unwrap();
        let expected = (metrics.width * LOOK.size + 2.0 * PADDING) * 2.0;
        assert!(
            (f32::from(u16::try_from(picture.width).unwrap()) - expected).abs() <= 2.0,
            "{picture:?}"
        );
        assert_eq!(
            picture.bgra.len(),
            (picture.width * picture.height * 4) as usize
        );
    }

    #[test]
    fn un_panico_se_vuelve_error() {
        let contained: Result<(), MathError> = contain(|| panic!("roto"));
        assert_eq!(contained, Err(MathError::Panic("roto".to_owned())));
    }
}
