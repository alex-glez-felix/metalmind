//! Tres fórmulas pintadas con RaTeX en una ventana GPUI, como se verían en
//! una nota: una en línea, alineada con la línea base del texto, y dos en
//! bloque. La última línea es la fuente de un bloque, como se vería con el
//! cursor encima.
//!
//! `cargo run --release --example tres_formulas`

use std::sync::Arc;

use gpui_kit::{
    App, AppContext, Bounds, Context, FontWeight, IntoElement, ParentElement, Pixels, Render,
    RenderImage, SharedString, Styled, Window, WindowBounds, WindowOptions, div, img, px, rgb,
    size,
};
use image::{Frame, RgbaImage};
use metalmind_spike_math::motor_ratex::RatexEngine;
use metalmind_spike_math::{Engine, Image, Look, MathError, Mode};

const TEXT: u32 = 0x00d4_d4d4;
const MUTED: u32 = 0x0080_8890;
const BACKGROUND: u32 = 0x001e_1f22;
const FONT_SIZE: f32 = 16.0;
const LINE_HEIGHT: f32 = 26.0;

const INLINE: &str = r"e^{i\pi} + 1 = 0";
const GAUSS: &str = r"\int_{-\infty}^{\infty} e^{-x^2}\,dx = \sqrt{\pi}";
const ALIGNED: &str = r"\begin{aligned} (a+b)^2 &= (a+b)(a+b) \\ &= a^2 + 2ab + b^2 \end{aligned}";

/// Una fórmula lista para GPUI y su tamaño en píxeles lógicos.
struct Formula {
    image: Arc<RenderImage>,
    width: Pixels,
    height: Pixels,
    /// Cuánto baja de la línea base.
    depth: Pixels,
}

impl Formula {
    fn new(
        engine: &mut RatexEngine,
        latex: &str,
        mode: Mode,
        scale: f32,
    ) -> Result<Self, MathError> {
        let look = Look {
            size: FONT_SIZE,
            scale,
            color: [0xd4, 0xd4, 0xd4],
        };
        let rendered = engine.render(latex, mode, &look)?;
        Ok(Self::from_image(&rendered.image, scale))
    }

    fn from_image(image: &Image, scale: f32) -> Self {
        let buffer =
            RgbaImage::from_raw(image.width, image.height, image.to_bgra()).unwrap_or_default();
        #[expect(
            clippy::cast_precision_loss,
            reason = "el tamaño de una fórmula cabe en f32"
        )]
        let (width, height) = (image.width as f32 / scale, image.height as f32 / scale);
        Self {
            image: Arc::new(RenderImage::new([Frame::new(buffer)])),
            width: px(width),
            height: px(height),
            depth: px(height - image.baseline / scale),
        }
    }

    fn element(&self) -> impl IntoElement {
        img(self.image.clone()).w(self.width).h(self.height)
    }
}

struct Formulas {
    inline: Formula,
    gauss: Formula,
    aligned: Formula,
}

impl Formulas {
    fn new(scale: f32) -> Result<Self, MathError> {
        let mut engine = RatexEngine;
        Ok(Self {
            inline: Formula::new(&mut engine, INLINE, Mode::Inline, scale)?,
            gauss: Formula::new(&mut engine, GAUSS, Mode::Display, scale)?,
            aligned: Formula::new(&mut engine, ALIGNED, Mode::Display, scale)?,
        })
    }
}

/// La nota, o por qué no se pudieron pintar sus fórmulas.
struct Note(Result<Formulas, MathError>);

impl Render for Note {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let page = div()
            .size_full()
            .bg(rgb(BACKGROUND))
            .text_color(rgb(TEXT))
            .text_size(px(FONT_SIZE))
            .line_height(px(LINE_HEIGHT))
            .p(px(32.0));
        match &self.0 {
            Ok(formulas) => page.child(formulas.note()),
            Err(error) => page.child(error.to_string()),
        }
    }
}

impl Formulas {
    fn note(&self) -> impl IntoElement {
        // La línea base del texto queda a esta distancia del fondo de su caja.
        let baseline_from_bottom = px((LINE_HEIGHT - FONT_SIZE) / 2.0 + FONT_SIZE * 0.22);
        div()
            .flex()
            .flex_col()
            .gap(px(14.0))
            .child(
                div()
                    .text_size(px(FONT_SIZE * 1.6))
                    .font_weight(FontWeight::BOLD)
                    .child("Fórmulas"),
            )
            .child(
                div()
                    .flex()
                    .items_end()
                    .child("La identidad de Euler, ")
                    .child(
                        div()
                            .relative()
                            .top(self.inline.depth - baseline_from_bottom)
                            .child(self.inline.element()),
                    )
                    .child(", une cinco constantes."),
            )
            .child("La integral de Gauss:")
            .child(div().flex().justify_center().child(self.gauss.element()))
            .child("Un desarrollo con aligned:")
            .child(div().flex().justify_center().child(self.aligned.element()))
            .child(
                div()
                    .text_color(rgb(MUTED))
                    .font_family(SharedString::from("monospace"))
                    .child(format!(
                        "$${GAUSS}$$   ← con el cursor encima se ve la fuente"
                    )),
            )
    }
}

fn main() {
    gpui_kit::application().run(|cx: &mut App| {
        let bounds = Bounds::centered(None, size(px(760.0), px(380.0)), cx);
        let options = WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            ..WindowOptions::default()
        };
        let opened = cx.open_window(options, |window, cx| {
            let formulas = Formulas::new(window.scale_factor());
            cx.new(|_| Note(formulas))
        });
        if let Err(error) = opened {
            eprintln!("no se pudo abrir la ventana: {error}");
            cx.quit();
        }
    });
}
