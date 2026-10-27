//! Los colores del visor. En Metalmind saldrían de los tokens del tema; aquí
//! hay dos juegos fijos para las capturas en oscuro y en claro.

use gpui_kit::{Hsla, rgb};
use metalmind_spike_pizarra::escena::Role;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Variante {
    Oscuro,
    Claro,
}

#[derive(Clone, Debug)]
pub struct Tema {
    pub fondo: Hsla,
    pub punto: Hsla,
    pub tinta: Hsla,
    pub tenue: Hsla,
    pub tarjeta: Hsla,
    pub acento: Hsla,
    pub barra: Hsla,
    pub borde: Hsla,
    pub fuente: String,
    paleta: [u32; 6],
}

impl Tema {
    pub fn de(variante: Variante, fuente: &str) -> Self {
        match variante {
            Variante::Oscuro => Self {
                fondo: rgb(0x001e_1f22).into(),
                punto: rgb(0x002e_3035).into(),
                tinta: rgb(0x00d4_d4d4).into(),
                tenue: rgb(0x008a_8f98).into(),
                tarjeta: rgb(0x0026_282c).into(),
                acento: rgb(0x007a_a2f7).into(),
                barra: rgb(0x0026_282c).into(),
                borde: rgb(0x003a_3d43).into(),
                fuente: fuente.to_owned(),
                paleta: [
                    0x00e0_6c75,
                    0x00d1_9a66,
                    0x00e5_c07b,
                    0x0098_c379,
                    0x0061_afef,
                    0x00c6_78dd,
                ],
            },
            Variante::Claro => Self {
                fondo: rgb(0x00fa_fafa).into(),
                punto: rgb(0x00e2_e4e8).into(),
                tinta: rgb(0x001e_1e1e).into(),
                tenue: rgb(0x006b_7280).into(),
                tarjeta: rgb(0x00ff_ffff).into(),
                acento: rgb(0x0019_71c2).into(),
                barra: rgb(0x00f0_f1f3).into(),
                borde: rgb(0x00d5_d8dd).into(),
                fuente: fuente.to_owned(),
                paleta: [
                    0x00e0_3131,
                    0x00f0_8c00,
                    0x00c9_9a00,
                    0x002f_9e44,
                    0x0019_71c2,
                    0x009c_36b5,
                ],
            },
        }
    }

    /// `"1"`…`"6"` es la paleta del tema; `#rrggbb`, ese color; sin color, la tinta.
    pub fn color(&self, color: Option<&str>) -> Hsla {
        let Some(color) = color else {
            return self.tinta;
        };
        if let Some(hex) = color.strip_prefix('#') {
            return u32::from_str_radix(hex, 16).map_or(self.tinta, |c| rgb(c).into());
        }
        color
            .parse::<usize>()
            .ok()
            .and_then(|i| self.paleta.get(i.wrapping_sub(1)))
            .map_or(self.tinta, |c| rgb(*c).into())
    }

    pub fn de_rol(&self, rol: &Role) -> Hsla {
        let tenue = |c: Hsla, alfa: f32| Hsla { a: alfa, ..c };
        match rol {
            Role::Line(color) => self.color(color.as_deref()),
            Role::Fill(None) => tenue(self.tinta, 0.04),
            Role::Fill(color) => tenue(self.color(color.as_deref()), 0.14),
            Role::CardFill => self.tarjeta,
            Role::CardAccent => self.acento,
            Role::GroupFill(color) => tenue(self.color(color.as_deref()), 0.06),
            Role::LabelBackground => self.fondo,
        }
    }
}
