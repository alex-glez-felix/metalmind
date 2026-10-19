//! Las imágenes de las fórmulas: se pintan en el executor de fondo de GPUI
//! y se guardan por fórmula, letra, escala y color, con un tope en bytes.
//! Nada va a disco: pintar una fórmula cuesta menos que leer un archivo.

use std::collections::HashMap;
use std::sync::Arc;

use gpui_kit::RenderImage;
use image::{Frame, ImageBuffer};

use super::engine::{self, Look, MathError, Picture};

/// Lo que ocupan, a lo sumo, las imágenes guardadas: una fórmula ocupa
/// ~29 KB a escala 1 y ~115 KB a escala 2 (08 §5).
pub const BUDGET: usize = 32 * 1024 * 1024;

/// Una fórmula tal como se pinta. Los `f32` van por sus bits: son medidas
/// del tema y de la pantalla, no cuentas, así que dos iguales tienen los
/// mismos bits.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ImageKey {
    pub latex: String,
    pub display: bool,
    size: u32,
    scale: u32,
    color: [u32; 4],
}

impl ImageKey {
    pub fn new(latex: &str, display: bool, look: Look) -> Self {
        Self {
            latex: latex.to_owned(),
            display,
            size: look.size.to_bits(),
            scale: look.scale.to_bits(),
            color: look.color.map(f32::to_bits),
        }
    }

    pub fn look(&self) -> Look {
        Look {
            size: f32::from_bits(self.size),
            scale: f32::from_bits(self.scale),
            color: self.color.map(f32::from_bits),
        }
    }
}

/// Lo que hay de una fórmula.
#[derive(Clone, Debug)]
pub enum Image {
    /// Se está pintando: la vista reserva su lugar.
    Pending,
    Ready(Arc<RenderImage>),
    Failed(MathError),
}

#[derive(Debug)]
struct Entry {
    image: Image,
    bytes: usize,
    /// El último frame que la pidió.
    used: u64,
}

/// Las imágenes con su LRU: al pasar del tope se sacan las que hace más
/// frames que nadie pide, nunca una del frame en curso.
#[derive(Debug)]
pub struct Images {
    entries: HashMap<ImageKey, Entry>,
    frame: u64,
    bytes: usize,
    budget: usize,
    /// Las que salieron y hay que soltar del atlas de GPUI, que es de la
    /// ventana: se sueltan en el siguiente paint.
    evicted: Vec<Arc<RenderImage>>,
}

impl Default for Images {
    fn default() -> Self {
        Self::with_budget(BUDGET)
    }
}

impl Images {
    pub fn with_budget(budget: usize) -> Self {
        Self {
            entries: HashMap::new(),
            frame: 0,
            bytes: 0,
            budget,
            evicted: Vec::new(),
        }
    }

    /// Empieza un frame: lo que se pida desde ahora es lo visible.
    pub fn next_frame(&mut self) {
        self.frame += 1;
    }

    /// Lo que hay de `key`, o `None` si hay que pintarla: entonces queda
    /// pendiente y quien llama lanza el trabajo.
    pub fn get(&mut self, key: &ImageKey) -> Option<Image> {
        if let Some(entry) = self.entries.get_mut(key) {
            entry.used = self.frame;
            return Some(entry.image.clone());
        }
        self.entries.insert(
            key.clone(),
            Entry {
                image: Image::Pending,
                bytes: 0,
                used: self.frame,
            },
        );
        None
    }

    /// Llegó lo pintado en el fondo.
    pub fn finish(&mut self, key: ImageKey, image: Image) {
        let bytes = match &image {
            Image::Ready(image) => image.as_bytes(0).map_or(0, <[u8]>::len),
            Image::Failed(_) | Image::Pending => key.latex.len(),
        };
        let used = self
            .entries
            .get(&key)
            .map_or(self.frame, |entry| entry.used);
        if let Some(old) = self.entries.insert(key, Entry { image, bytes, used }) {
            self.release(old);
        }
        self.bytes += bytes;
        self.evict();
    }

    fn evict(&mut self) {
        while self.bytes > self.budget {
            let oldest = self
                .entries
                .iter()
                .filter(|(_, entry)| entry.used < self.frame && entry.bytes > 0)
                .min_by_key(|(_, entry)| entry.used)
                .map(|(key, _)| key.clone());
            let Some(entry) = oldest.and_then(|key| self.entries.remove(&key)) else {
                break;
            };
            self.release(entry);
        }
    }

    fn release(&mut self, entry: Entry) {
        self.bytes -= entry.bytes;
        if let Image::Ready(image) = entry.image {
            self.evicted.push(image);
        }
    }

    /// Las imágenes que salieron, para soltarlas del atlas.
    pub fn take_evicted(&mut self) -> Vec<Arc<RenderImage>> {
        std::mem::take(&mut self.evicted)
    }

    #[cfg(test)]
    pub fn bytes(&self) -> usize {
        self.bytes
    }
}

/// Pinta la fórmula de `key`; corre en el fondo.
pub fn paint(key: &ImageKey) -> Image {
    match engine::render(&key.latex, key.display, key.look()).and_then(to_image) {
        Ok(image) => Image::Ready(Arc::new(image)),
        Err(error) => Image::Failed(error),
    }
}

fn to_image(picture: Picture) -> Result<RenderImage, MathError> {
    let buffer = ImageBuffer::from_raw(picture.width, picture.height, picture.bgra)
        .ok_or_else(|| MathError::Render("los píxeles no son del tamaño de la imagen".into()))?;
    Ok(RenderImage::new(vec![Frame::new(buffer)]))
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;

    use super::*;

    fn key(latex: &str) -> ImageKey {
        let look = Look {
            size: 16.0,
            scale: 1.0,
            color: [1.0; 4],
        };
        ImageKey::new(latex, false, look)
    }

    fn ready(images: &mut Images, latex: &str) {
        assert!(images.get(&key(latex)).is_none());
        images.finish(key(latex), paint(&key(latex)));
    }

    #[test]
    fn se_pinta_una_vez_y_queda_lista() {
        let mut images = Images::default();
        assert!(images.get(&key("x")).is_none());
        assert!(matches!(images.get(&key("x")), Some(Image::Pending)));
        images.finish(key("x"), paint(&key("x")));
        assert!(matches!(images.get(&key("x")), Some(Image::Ready(_))));
        assert!(images.bytes() > 0);
    }

    #[test]
    fn una_formula_rota_queda_como_error() {
        let mut images = Images::default();
        ready(&mut images, r"\frac{");
        assert!(matches!(
            images.get(&key(r"\frac{")),
            Some(Image::Failed(MathError::Parse(_)))
        ));
    }

    #[test]
    fn al_pasar_del_tope_sale_la_que_hace_mas_que_nadie_pide() {
        let mut probe = Images::default();
        ready(&mut probe, "x_1");
        let one = probe.bytes();
        let mut images = Images::with_budget(one * 2 + one / 2);
        ready(&mut images, "x_1");
        images.next_frame();
        ready(&mut images, "x_2");
        images.next_frame();
        assert!(images.get(&key("x_2")).is_some());
        ready(&mut images, "x_3");
        assert_eq!(images.take_evicted().len(), 1);
        assert!(images.get(&key("x_1")).is_none(), "salió la más vieja");
        assert!(matches!(images.get(&key("x_2")), Some(Image::Ready(_))));
    }

    #[test]
    fn nunca_saca_lo_del_frame_en_curso() {
        let mut images = Images::with_budget(1);
        ready(&mut images, "a");
        ready(&mut images, "b");
        assert_eq!(images.take_evicted().len(), 0);
        assert!(matches!(images.get(&key("a")), Some(Image::Ready(_))));
    }
}
