//! La imagen de una fórmula: RGBA con alfa sin premultiplicar, que es lo
//! que guarda el `RenderImage` de GPUI (en BGRA).

use std::io;
use std::path::Path;

#[derive(Debug, Clone)]
pub struct Image {
    pub width: u32,
    pub height: u32,
    /// Dónde cae la línea base, en píxeles desde arriba: con ella una
    /// fórmula en línea se alinea con el texto.
    pub baseline: f32,
    pub rgba: Vec<u8>,
}

impl Image {
    /// De los píxeles de tiny-skia, que vienen premultiplicados.
    #[must_use]
    pub fn from_premultiplied(width: u32, height: u32, baseline: f32, mut rgba: Vec<u8>) -> Self {
        for pixel in rgba.as_chunks_mut::<4>().0 {
            let alpha = u16::from(pixel[3]);
            if alpha != 0 && alpha != 255 {
                for channel in &mut pixel[..3] {
                    *channel = u8::try_from(u16::from(*channel) * 255 / alpha).unwrap_or(u8::MAX);
                }
            }
        }
        Self {
            width,
            height,
            baseline,
            rgba,
        }
    }

    /// Los bytes que ocupa en memoria.
    #[must_use]
    pub fn bytes(&self) -> usize {
        self.rgba.len()
    }

    /// Los mismos píxeles en BGRA, el orden de GPUI.
    #[must_use]
    pub fn to_bgra(&self) -> Vec<u8> {
        let mut bgra = self.rgba.clone();
        for pixel in bgra.as_chunks_mut::<4>().0 {
            pixel.swap(0, 2);
        }
        bgra
    }

    /// # Errors
    /// Si no se puede escribir el archivo.
    pub fn save_png(&self, path: &Path) -> io::Result<()> {
        let file = std::io::BufWriter::new(std::fs::File::create(path)?);
        let mut encoder = png::Encoder::new(file, self.width, self.height);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder.write_header().map_err(io::Error::other)?;
        writer
            .write_image_data(&self.rgba)
            .map_err(io::Error::other)?;
        writer.finish().map_err(io::Error::other)
    }

    /// Un lienzo de un color.
    #[must_use]
    pub fn filled(width: u32, height: u32, color: [u8; 4]) -> Self {
        let pixels = usize::try_from(u64::from(width) * u64::from(height)).unwrap_or(0);
        Self {
            width,
            height,
            baseline: 0.0,
            rgba: color.repeat(pixels),
        }
    }

    /// Pinta `top` encima, con su esquina en `(x, y)`.
    pub fn draw(&mut self, top: &Self, x: u32, y: u32) {
        for row in 0..top.height {
            let Some(target_row) = y.checked_add(row).filter(|r| *r < self.height) else {
                break;
            };
            for column in 0..top.width {
                let Some(target_column) = x.checked_add(column).filter(|c| *c < self.width) else {
                    break;
                };
                let from = pixel_index(top.width, column, row);
                let to = pixel_index(self.width, target_column, target_row);
                blend(&mut self.rgba[to..to + 4], &top.rgba[from..from + 4]);
            }
        }
    }
}

fn pixel_index(width: u32, x: u32, y: u32) -> usize {
    usize::try_from((u64::from(y) * u64::from(width) + u64::from(x)) * 4).unwrap_or(usize::MAX)
}

/// `source` sobre `target`, con alfa sin premultiplicar.
fn blend(target: &mut [u8], source: &[u8]) {
    let alpha = u32::from(source[3]);
    for channel in 0..3 {
        let over = u32::from(source[channel]) * alpha + u32::from(target[channel]) * (255 - alpha);
        target[channel] = u8::try_from(over / 255).unwrap_or(u8::MAX);
    }
    let covered = alpha + u32::from(target[3]) * (255 - alpha) / 255;
    target[3] = u8::try_from(covered).unwrap_or(u8::MAX);
}
