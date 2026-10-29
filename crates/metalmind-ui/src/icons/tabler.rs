//! El set de Tabler que viene con Metalmind (05 §7), embebido y comprimido
//! con `scripts/tabler.sh`. Se descomprime la primera vez que se pide un
//! ícono y queda en memoria.

use std::collections::HashMap;
use std::sync::LazyLock;

use miniz_oxide::inflate::decompress_to_vec_with_limit;

const COMPRESSED: &[u8] = include_bytes!("../../assets/tabler.deflate");
/// Más de lo que ocupa el set descomprimido (2,2 MB en la 3.50): un tope
/// contra un archivo roto, no un límite del set.
const MAX_SIZE: usize = 8 << 20;

/// Lo que va alrededor del contenido de cada ícono, que trae su propio
/// trazo o relleno en un `<g>`.
const SVG_START: &str =
    r#"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24">"#;
const SVG_END: &str = "</svg>";

static SET: LazyLock<Result<Tabler, SetError>> = LazyLock::new(Tabler::decode);

#[derive(Debug, thiserror::Error)]
pub enum SetError {
    #[error("el set de Tabler embebido no se pudo descomprimir: {0}")]
    Inflate(String),
    #[error("el set de Tabler embebido no es UTF-8")]
    NotUtf8,
    #[error("la línea {0} del set de Tabler embebido no tiene nombre y contenido")]
    Line(usize),
}

/// Los íconos por nombre (`folder`, `folder-filled`).
pub struct Tabler {
    icons: HashMap<Box<str>, Box<str>>,
}

impl Tabler {
    /// El set, descomprimido la primera vez.
    ///
    /// # Errors
    /// Si el archivo embebido está roto.
    pub fn get() -> Result<&'static Self, &'static SetError> {
        SET.as_ref()
    }

    /// El SVG entero del ícono, o `None` si el set no lo tiene.
    pub fn svg(&self, name: &str) -> Option<String> {
        self.icons
            .get(name)
            .map(|content| format!("{SVG_START}{content}{SVG_END}"))
    }

    pub fn contains(&self, name: &str) -> bool {
        self.icons.contains_key(name)
    }

    pub fn names(&self) -> impl Iterator<Item = &str> {
        self.icons.keys().map(AsRef::as_ref)
    }

    fn decode() -> Result<Self, SetError> {
        let bytes = decompress_to_vec_with_limit(COMPRESSED, MAX_SIZE)
            .map_err(|error| SetError::Inflate(error.to_string()))?;
        let text = String::from_utf8(bytes).map_err(|_| SetError::NotUtf8)?;
        let icons = text
            .lines()
            .enumerate()
            .map(|(at, line)| {
                line.split_once('\t')
                    .map(|(name, content)| (name.into(), content.into()))
                    .ok_or(SetError::Line(at + 1))
            })
            .collect::<Result<_, _>>()?;
        Ok(Self { icons })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trae_los_de_linea_y_los_rellenos() {
        let set = Tabler::get().unwrap();
        assert_eq!(set.names().count(), 6257);
        assert!(set.contains("folder"));
        assert!(set.contains("folder-filled"));
        assert!(!set.contains("no-existe"));
    }

    #[test]
    fn cada_icono_es_un_svg_de_24_con_su_trazo_o_su_relleno() {
        let set = Tabler::get().unwrap();
        let outline = set.svg("folder").unwrap();
        assert!(outline.starts_with(SVG_START), "{outline}");
        assert!(outline.ends_with("</g></svg>"), "{outline}");
        assert!(outline.contains(r#"stroke="currentColor" stroke-width="2""#));
        assert!(
            set.svg("folder-filled")
                .unwrap()
                .contains(r#"<g fill="currentColor">"#)
        );
        assert_eq!(set.svg("no-existe"), None);
    }
}
