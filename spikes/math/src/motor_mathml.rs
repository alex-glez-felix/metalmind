//! `pulldown-latex` o `math-core` sacan MathML Core del LaTeX y
//! `formulary` lo compone con la tabla MATH de una fuente, sin navegador. `formulary` da una lista
//! de glifos y reglas; aquí se pinta por su SVG con `resvg`, el mismo
//! rasterizador que ya trae GPUI.

use formulary::{LayoutOptions, MathFont};
use pulldown_latex::config::DisplayMode;
use pulldown_latex::{Parser, RenderConfig, Storage};

use crate::{Engine, Image, Look, MathError, Mode, Phases, Rendered, timed};

/// El margen que `formulary::svg` deja alrededor, en unidades de la fuente.
const SVG_MARGIN: f32 = 2.0;

/// Quién pasa el LaTeX a MathML.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Converter {
    PulldownLatex,
    MathCore,
}

pub struct MathmlEngine {
    converter: Converter,
    math_core: math_core::LatexToMathML,
    font: MathFont<'static>,
    svg_options: usvg::Options<'static>,
}

impl MathmlEngine {
    /// `font` es una fuente con tabla MATH.
    ///
    /// # Errors
    /// Si la fuente no tiene tabla MATH o `math-core` no arranca.
    pub fn new(converter: Converter, data: &'static [u8]) -> Result<Self, MathError> {
        let font = MathFont::new(data, 0).map_err(|error| MathError::Layout(error.to_string()))?;
        let math_core =
            math_core::LatexToMathML::new(math_core::MathCoreConfig::default(), Vec::new())
                .map_err(|(error, _, _)| MathError::Convert(error.to_string()))?;
        Ok(Self {
            converter,
            math_core,
            font,
            svg_options: usvg::Options::default(),
        })
    }

    /// El MathML de `latex`.
    ///
    /// # Errors
    /// Si el convertidor no entiende el LaTeX.
    pub fn convert(&self, latex: &str, mode: Mode) -> Result<String, MathError> {
        match self.converter {
            Converter::PulldownLatex => pulldown(latex, mode),
            Converter::MathCore => {
                let display = match mode {
                    Mode::Inline => math_core::MathDisplay::Inline,
                    Mode::Display => math_core::MathDisplay::Block,
                };
                self.math_core
                    .convert_with_local_state(latex, display)
                    .map(|result| result.mathml)
                    .map_err(|error| MathError::Convert(error.to_string()))
            }
        }
    }

    /// Compone el MathML y lo deja en SVG.
    ///
    /// # Errors
    /// Si `formulary` no puede leer el MathML.
    pub fn compose(&self, mathml: &str, look: &Look) -> Result<(String, f32), MathError> {
        let tree =
            formulary::parse(mathml).map_err(|error| MathError::Layout(error.to_string()))?;
        let options = LayoutOptions {
            font_size: look.size,
        };
        let laid = formulary::layout(&tree, &self.font, &options);
        let baseline = SVG_MARGIN + laid.ascent;
        Ok((formulary::svg::to_svg(&laid, &self.font), baseline))
    }

    /// # Errors
    /// Si `resvg` no puede leer el SVG.
    pub fn rasterize(&self, svg: &str, baseline: f32, look: &Look) -> Result<Image, MathError> {
        let tree = usvg::Tree::from_str(svg, &self.svg_options)
            .map_err(|error| MathError::Render(error.to_string()))?;
        let size = tree
            .size()
            .to_int_size()
            .scale_by(look.scale)
            .ok_or_else(|| MathError::Render("la fórmula no tiene tamaño".into()))?;
        let mut pixmap = tiny_skia::Pixmap::new(size.width(), size.height())
            .ok_or_else(|| MathError::Render("la fórmula no tiene tamaño".into()))?;
        let transform = tiny_skia::Transform::from_scale(look.scale, look.scale);
        resvg::render(&tree, transform, &mut pixmap.as_mut());
        let mut image = Image::from_premultiplied(
            pixmap.width(),
            pixmap.height(),
            baseline * look.scale,
            pixmap.take(),
        );
        tint(&mut image, look.color);
        Ok(image)
    }
}

impl Engine for MathmlEngine {
    fn render(&mut self, latex: &str, mode: Mode, look: &Look) -> Result<Rendered, MathError> {
        let (mathml, convert) = timed(|| self.convert(latex, mode));
        let mathml = mathml?;
        let (composed, layout) = timed(|| self.compose(&mathml, look));
        let (svg, baseline) = composed?;
        let (image, raster) = timed(|| self.rasterize(&svg, baseline, look));
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

fn pulldown(latex: &str, mode: Mode) -> Result<String, MathError> {
    let storage = Storage::new();
    let events = Parser::new(latex, &storage)
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| MathError::Convert(error.to_string()))?;
    let config = RenderConfig {
        display_mode: match mode {
            Mode::Inline => DisplayMode::Inline,
            Mode::Display => DisplayMode::Block,
        },
        xml: true,
        ..RenderConfig::default()
    };
    let mut mathml = String::new();
    pulldown_latex::push_mathml(
        &mut mathml,
        events.into_iter().map(Ok::<_, std::fmt::Error>),
        config,
    )
    .map_err(|error| MathError::Convert(error.to_string()))?;
    Ok(mathml)
}

/// El SVG de `formulary` va en negro; la fórmula es de un solo color, así
/// que basta con poner el del tema y dejar la cobertura en el alfa.
fn tint(image: &mut Image, color: [u8; 3]) {
    for pixel in image.rgba.as_chunks_mut::<4>().0 {
        pixel[..3].copy_from_slice(&color);
    }
}
