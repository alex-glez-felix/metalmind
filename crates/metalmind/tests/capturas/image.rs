//! Una captura en RGB, guardada y leída como PNG, y la diferencia entre dos.

use std::fs::File;
use std::io::{BufReader, BufWriter};
use std::path::Path;

#[derive(PartialEq, Eq)]
pub struct Image {
    pub width: u16,
    pub height: u16,
    /// Tres bytes por píxel, fila por fila.
    pub rgb: Vec<u8>,
}

impl Image {
    pub fn save(&self, path: &Path) {
        let file = BufWriter::new(File::create(path).expect("crear el PNG"));
        let mut encoder = png::Encoder::new(file, self.width.into(), self.height.into());
        encoder.set_color(png::ColorType::Rgb);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder.write_header().expect("escribir el encabezado");
        writer.write_image_data(&self.rgb).expect("escribir el PNG");
    }

    pub fn load(path: &Path) -> Self {
        let file = BufReader::new(File::open(path).expect("abrir el PNG"));
        let mut reader = png::Decoder::new(file).read_info().expect("leer el PNG");
        let mut rgb = vec![0; reader.output_buffer_size().expect("tamaño del PNG")];
        let info = reader.next_frame(&mut rgb).expect("leer los píxeles");
        assert_eq!(
            (info.color_type, info.bit_depth),
            (png::ColorType::Rgb, png::BitDepth::Eight),
            "{} no es un PNG de estas capturas",
            path.display()
        );
        rgb.truncate(info.buffer_size());
        Self {
            width: u16::try_from(info.width).expect("ancho"),
            height: u16::try_from(info.height).expect("alto"),
            rgb,
        }
    }

    /// Cuántos píxeles cambian de `self` a `other` y una imagen que los
    /// marca en rojo sobre `other` atenuada; `None` si son iguales. Dos
    /// tamaños distintos cuentan como todos los píxeles distintos.
    pub fn diff(&self, other: &Self) -> Option<(usize, Self)> {
        if self == other {
            return None;
        }
        if (self.width, self.height) != (other.width, other.height) {
            let pixels = usize::from(other.width) * usize::from(other.height);
            return Some((pixels.max(1), other.faded()));
        }
        let mut changed = 0;
        let rgb = self
            .rgb
            .as_chunks::<3>()
            .0
            .iter()
            .zip(other.rgb.as_chunks::<3>().0)
            .flat_map(|(before, after)| {
                if before == after {
                    fade(*after)
                } else {
                    changed += 1;
                    [255, 0, 0]
                }
            })
            .collect();
        let marked = Self {
            width: other.width,
            height: other.height,
            rgb,
        };
        Some((changed, marked))
    }

    fn faded(&self) -> Self {
        Self {
            width: self.width,
            height: self.height,
            rgb: self
                .rgb
                .as_chunks::<3>()
                .0
                .iter()
                .flat_map(|&pixel| fade(pixel))
                .collect(),
        }
    }
}

/// El píxel a un cuarto de su brillo, para que el rojo de lo que cambió resalte.
fn fade(pixel: [u8; 3]) -> [u8; 3] {
    pixel.map(|channel| channel / 4)
}

#[test]
fn la_diferencia_cuenta_los_pixeles_que_cambian() {
    let image = |rgb: Vec<u8>| Image {
        width: 2,
        height: 1,
        rgb,
    };
    let before = image(vec![8, 8, 8, 40, 40, 40]);
    assert!(before.diff(&image(vec![8, 8, 8, 40, 40, 40])).is_none());
    let (changed, marked) = before.diff(&image(vec![8, 8, 8, 40, 40, 41])).unwrap();
    assert_eq!(changed, 1);
    assert_eq!(marked.rgb, [2, 2, 2, 255, 0, 0]);
}

#[test]
fn una_imagen_vuelve_igual_del_png() {
    let dir = tempfile::tempdir().unwrap();
    let image = Image {
        width: 2,
        height: 2,
        rgb: (0..12).collect(),
    };
    let path = dir.path().join("a.png");
    image.save(&path);
    assert!(Image::load(&path) == image);
}
