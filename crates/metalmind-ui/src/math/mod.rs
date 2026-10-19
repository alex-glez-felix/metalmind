//! Las fórmulas LaTeX de las notas, compuestas con RaTeX (08 §7). El layout
//! va en el hilo de la UI, porque da el alto de cada línea en el mismo
//! frame; la imagen, en el executor de fondo, y mientras no está la vista
//! reserva su lugar. Una por ventana, compartida por sus paneles.

mod engine;
mod geometry;
mod images;
mod metrics;

use std::sync::Arc;

use gpui_kit::{AppContext as _, Context, RenderImage, Window};

pub use self::engine::{Look, MathError, Metrics, PADDING, Picture, measure, render};
pub use self::geometry::{FormulaBox, Geometry};
pub use self::images::{Image, ImageKey};
use self::images::{Images, paint};
pub use self::metrics::FormulaMeasure;
pub use self::metrics::MetricsCache;

#[derive(Default)]
pub struct Formulas {
    metrics: Arc<MetricsCache>,
    /// El medidor de la letra de hoy: el mismo mientras no cambie, porque
    /// el motor compara las medidas para saber si revisar las ventanas.
    measure: Option<Arc<FormulaMeasure>>,
    images: Images,
}

impl Formulas {
    /// El medidor que se le presta al motor para la letra `geometry`.
    pub fn measure(&mut self, geometry: Geometry) -> Arc<FormulaMeasure> {
        match &self.measure {
            Some(measure) if measure.geometry() == geometry => measure.clone(),
            _ => {
                let measure = Arc::new(FormulaMeasure::new(self.metrics.clone(), geometry));
                self.measure = Some(measure.clone());
                measure
            }
        }
    }

    /// Lo que mide la fórmula, en em, o por qué no compone.
    ///
    /// # Errors
    /// Si RaTeX no la entiende o no puede componerla.
    pub fn metrics(&self, latex: &str, display: bool) -> Result<Metrics, MathError> {
        self.metrics.get(latex, display)
    }

    /// Empieza un frame de la ventana: las imágenes que pida cualquier
    /// panel desde ahora no salen de la caché hasta el siguiente.
    pub fn next_frame(&mut self) {
        self.images.next_frame();
    }

    /// La imagen de `key` si ya está; si no, la manda a pintar al fondo y
    /// avisa a quien observa esta entidad cuando llega.
    pub fn image(&mut self, key: &ImageKey, cx: &mut Context<Self>) -> Image {
        if let Some(image) = self.images.get(key) {
            return image;
        }
        let job = key.clone();
        let painting = cx.background_spawn(async move {
            let image = paint(&job);
            (job, image)
        });
        cx.spawn(async move |this, cx| {
            let (key, image) = painting.await;
            // Si la ventana ya se cerró, no hay a quién dársela.
            let _ = this.update(cx, |formulas, cx| {
                if let Image::Failed(error) = &image {
                    // Compuso al medirla y falló al pintarla: desde ahora se ve
                    // como una que no compone, con su fuente y su mensaje.
                    log::error!("no se pudo pintar la fórmula {:?}: {error}", key.latex);
                    formulas
                        .metrics
                        .fail(&key.latex, key.display, error.clone());
                }
                formulas.images.finish(key, image);
                cx.notify();
            });
        })
        .detach();
        Image::Pending
    }

    /// Suelta del atlas de la ventana las imágenes que salieron de la
    /// caché.
    pub fn drop_evicted(&mut self, window: &mut Window) {
        for image in self.images.take_evicted() {
            drop_image(window, image);
        }
    }
}

fn drop_image(window: &mut Window, image: Arc<RenderImage>) {
    if let Err(error) = window.drop_image(image) {
        log::error!("no se pudo soltar la imagen de una fórmula: {error:#}");
    }
}
