//! RaTeX: parser y layout de KaTeX en Rust, con las fuentes de KaTeX
//! embebidas. Su API pública solo pinta a PNG, así que la imagen se
//! decodifica otra vez; `ratex-gpui` hace lo mismo.

use ratex_layout::LayoutOptions;
use ratex_types::{Color, DisplayList, MathStyle};

use ratex_parser::ParseNode;

use crate::{Engine, Image, Look, MathError, Mode, Phases, Rendered, timed};

/// Lo que se deja alrededor de la fórmula, en píxeles lógicos.
const PADDING: f32 = 1.0;

pub struct RatexEngine;

impl RatexEngine {
    /// # Errors
    /// Si el parser no entiende el LaTeX.
    pub fn parse(latex: &str) -> Result<Vec<ParseNode>, MathError> {
        ratex_parser::parse(latex).map_err(|error| MathError::Convert(error.to_string()))
    }

    #[must_use]
    pub fn compose(nodes: &[ParseNode], mode: Mode, look: &Look) -> DisplayList {
        let [r, g, b] = look.color.map(|channel| f32::from(channel) / 255.0);
        let options = LayoutOptions {
            style: match mode {
                Mode::Inline => MathStyle::Text,
                Mode::Display => MathStyle::Display,
            },
            color: Color::rgb(r, g, b),
            ..LayoutOptions::default()
        };
        ratex_layout::to_display_list(&ratex_layout::layout(nodes, &options))
    }

    /// # Errors
    /// Si la fórmula es más grande de lo que RaTeX acepta pintar.
    pub fn rasterize(list: &DisplayList, look: &Look) -> Result<Image, MathError> {
        let options = ratex_render::RenderOptions {
            font_size: look.size,
            padding: PADDING,
            background_color: Color::new(0.0, 0.0, 0.0, 0.0),
            font_dir: String::new(),
            device_pixel_ratio: look.scale,
        };
        let png = ratex_render::render_to_png(list, &options).map_err(MathError::Render)?;
        let (width, height, rgba) = decode(&png)?;
        #[expect(
            clippy::cast_possible_truncation,
            reason = "la altura en em de una fórmula cabe en f32"
        )]
        let baseline = (list.height as f32 * look.size + PADDING) * look.scale;
        Ok(Image {
            width,
            height,
            baseline,
            rgba,
        })
    }
}

impl Engine for RatexEngine {
    fn render(&mut self, latex: &str, mode: Mode, look: &Look) -> Result<Rendered, MathError> {
        let (nodes, convert) = timed(|| Self::parse(latex));
        let nodes = nodes?;
        let (list, layout) = timed(|| Self::compose(&nodes, mode, look));
        let (image, raster) = timed(|| Self::rasterize(&list, look));
        Ok(Rendered {
            image: image?,
            phases: Phases {
                convert,
                layout,
                raster,
            },
        })
    }
}

fn decode(png: &[u8]) -> Result<(u32, u32, Vec<u8>), MathError> {
    let failed = |error: png::DecodingError| MathError::Render(error.to_string());
    let mut reader = png::Decoder::new(std::io::Cursor::new(png))
        .read_info()
        .map_err(failed)?;
    let mut rgba = vec![0; reader.output_buffer_size()];
    let info = reader.next_frame(&mut rgba).map_err(failed)?;
    if info.color_type != png::ColorType::Rgba || info.bit_depth != png::BitDepth::Eight {
        return Err(MathError::Render(format!(
            "PNG {:?} de {:?} bits, se esperaba RGBA de 8",
            info.color_type, info.bit_depth
        )));
    }
    rgba.truncate(info.buffer_size());
    Ok((info.width, info.height, rgba))
}
