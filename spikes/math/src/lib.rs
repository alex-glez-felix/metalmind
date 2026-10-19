//! Spike de fórmulas: tres motores que pasan de LaTeX a una imagen que GPUI
//! puede pintar, con la misma interfaz para medirlos igual.
//!
//! - [`motor_typst`]: `mitex` (LaTeX → código de Typst) + `typst` + `typst-render`.
//! - [`motor_ratex`]: RaTeX (port de KaTeX) + su rasterizador.
//! - [`motor_mathml`]: `pulldown-latex` o `math-core` (LaTeX → MathML) +
//!   `formulary` (layout de MathML Core) + `resvg`.

pub mod formulas;
pub mod image;
pub mod memory;
#[cfg(feature = "mathml")]
pub mod motor_mathml;
#[cfg(feature = "ratex")]
pub mod motor_ratex;
#[cfg(feature = "typst")]
pub mod motor_typst;

use std::time::{Duration, Instant};

pub use image::Image;

/// En línea (`$…$`) o en bloque (`$$…$$`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Mode {
    Inline,
    Display,
}

/// Cómo se pinta: el tamaño de la fuente del editor en píxeles lógicos, la
/// escala de la pantalla y el color del texto del tema.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Look {
    pub size: f32,
    pub scale: f32,
    pub color: [u8; 3],
}

impl Default for Look {
    fn default() -> Self {
        Self {
            size: 16.0,
            scale: 1.0,
            color: [0x1f, 0x23, 0x28],
        }
    }
}

/// Por qué una fórmula no se pudo pintar.
#[derive(Debug, thiserror::Error)]
pub enum MathError {
    #[error("no se pudo convertir: {0}")]
    Convert(String),
    #[error("no se pudo componer: {0}")]
    Layout(String),
    #[error("no se pudo pintar: {0}")]
    Render(String),
}

/// Cuánto tardó cada paso de una fórmula: pasar el LaTeX a la entrada del
/// motor, componer y pintar.
#[derive(Debug, Clone, Copy, Default)]
pub struct Phases {
    pub convert: Duration,
    pub layout: Duration,
    pub raster: Duration,
}

impl Phases {
    #[must_use]
    pub fn total(&self) -> Duration {
        self.convert + self.layout + self.raster
    }
}

pub struct Rendered {
    pub image: Image,
    pub phases: Phases,
}

/// Un motor: de LaTeX a píxeles.
pub trait Engine {
    /// # Errors
    /// Si el LaTeX no se entiende o el motor no sabe componerlo.
    fn render(&mut self, latex: &str, mode: Mode, look: &Look) -> Result<Rendered, MathError>;
}

/// Corre `step` y dice cuánto tardó.
pub(crate) fn timed<T>(step: impl FnOnce() -> T) -> (T, Duration) {
    let start = Instant::now();
    let value = step();
    (value, start.elapsed())
}

/// Los motores del spike, por nombre en la línea de comandos.
#[cfg(all(feature = "typst", feature = "ratex", feature = "mathml"))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Typst,
    Ratex,
    Mathml,
    MathCore,
}

#[cfg(all(feature = "typst", feature = "ratex", feature = "mathml"))]
impl Kind {
    pub const ALL: [Self; 4] = [Self::Typst, Self::Ratex, Self::Mathml, Self::MathCore];

    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::Typst => "typst",
            Self::Ratex => "ratex",
            Self::Mathml => "mathml",
            Self::MathCore => "mathcore",
        }
    }

    #[must_use]
    pub fn parse(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|kind| kind.name() == name)
    }

    /// Arranca el motor: fuentes, biblioteca, lo que haga falta antes de la
    /// primera fórmula.
    ///
    /// # Errors
    /// Si las fuentes embebidas no cargan.
    pub fn start(self) -> Result<Box<dyn Engine>, MathError> {
        Ok(match self {
            Self::Typst => Box::new(motor_typst::TypstEngine::new()?),
            Self::Ratex => Box::new(motor_ratex::RatexEngine),
            Self::Mathml => Box::new(motor_mathml::MathmlEngine::new(
                motor_mathml::Converter::PulldownLatex,
                motor_typst::math_font()?,
            )?),
            Self::MathCore => Box::new(motor_mathml::MathmlEngine::new(
                motor_mathml::Converter::MathCore,
                motor_typst::math_font()?,
            )?),
        })
    }
}
