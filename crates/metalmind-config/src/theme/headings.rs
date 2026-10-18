//! El tamaño y el peso de cada nivel de heading en el editor, relativos a
//! la fuente del editor. Como los colores, un tema hereda los niveles que
//! no define.

use serde::Deserialize;

/// Un nivel de heading: cuántas veces el tamaño de la fuente y su peso
/// (de 100 a 900, como en CSS: 400 es normal y 700 negrita).
#[derive(Clone, Copy, Debug, PartialEq, Deserialize)]
#[serde(try_from = "RawHeading")]
pub struct Heading {
    pub size: f32,
    pub weight: u16,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawHeading {
    size: f32,
    weight: u16,
}

impl Heading {
    const MIN_SIZE: f32 = 0.5;
    const MAX_SIZE: f32 = 4.;
}

impl TryFrom<RawHeading> for Heading {
    type Error = String;

    fn try_from(raw: RawHeading) -> Result<Self, String> {
        if !(Self::MIN_SIZE..=Self::MAX_SIZE).contains(&raw.size) {
            return Err(format!(
                "el tamaño de un heading debe estar entre {} y {} veces la fuente, no {}",
                Self::MIN_SIZE,
                Self::MAX_SIZE,
                raw.size
            ));
        }
        if !(100..=900).contains(&raw.weight) || !raw.weight.is_multiple_of(100) {
            return Err(format!(
                "el peso de un heading va de 100 a 900, de cien en cien; no {}",
                raw.weight
            ));
        }
        Ok(Self {
            size: raw.size,
            weight: raw.weight,
        })
    }
}

/// Los seis niveles, del `#` al `######`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Headings(pub [Heading; 6]);

#[derive(Debug, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub(crate) struct RawHeadings {
    h1: Option<Heading>,
    h2: Option<Heading>,
    h3: Option<Heading>,
    h4: Option<Heading>,
    h5: Option<Heading>,
    h6: Option<Heading>,
}

#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct PartialHeadings([Option<Heading>; 6]);

impl RawHeadings {
    pub(crate) fn partial(&self) -> PartialHeadings {
        PartialHeadings([self.h1, self.h2, self.h3, self.h4, self.h5, self.h6])
    }
}

impl PartialHeadings {
    /// `self` encima de `base`: lo que `self` no define sale de `base`.
    pub(crate) fn over(self, base: Self) -> Self {
        let mut levels = self.0;
        for (level, inherited) in levels.iter_mut().zip(base.0) {
            *level = level.or(inherited);
        }
        Self(levels)
    }

    /// Los seis niveles, o las claves que faltan.
    pub(crate) fn complete(self, missing: &mut Vec<&'static str>) -> Option<Headings> {
        const KEYS: [&str; 6] = [
            "headings.h1",
            "headings.h2",
            "headings.h3",
            "headings.h4",
            "headings.h5",
            "headings.h6",
        ];
        for (level, key) in self.0.iter().zip(KEYS) {
            if level.is_none() {
                missing.push(key);
            }
        }
        let [h1, h2, h3, h4, h5, h6] = self.0;
        Some(Headings([h1?, h2?, h3?, h4?, h5?, h6?]))
    }
}
