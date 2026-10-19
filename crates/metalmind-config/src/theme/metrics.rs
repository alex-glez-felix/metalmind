//! Las medidas de un tema: la escala de espaciado, radios, anchos de línea,
//! tipografía, alto de las filas y las medidas de cada parte de la UI. Como
//! los colores, cada grupo existe como se escribe (`Raw*`), resuelto pero
//! quizá incompleto (`Partial*`, para `extends`) y completo; las medidas de
//! `rows` y `layout` pueden nombrar un paso de `spacing`.

use std::fmt;

use serde::de::DeserializeOwned;
use serde::{Deserialize, Deserializer};

use super::units::LengthRef;
pub use super::units::{Count, Factor, Length, Px, TextStyle, Weight};
use crate::diagnostic::Location;
use crate::parse::Spanned;

/// Un paso de `spacing` que no existe, con dónde se escribió.
pub(crate) struct UnknownStep {
    pub(crate) key: String,
    pub(crate) name: String,
    pub(crate) location: Option<Location>,
}

/// Lo escrito en un campo, ya resuelto contra la escala de espaciado.
pub(crate) trait Metric: Copy {
    type Raw: DeserializeOwned + fmt::Debug;
    fn resolve(raw: &Self::Raw, spacing: Option<&PartialSpacing>) -> Result<Self, String>;
}

macro_rules! plain_metric {
    ($($ty:ty),*) => {$(
        impl Metric for $ty {
            type Raw = Self;
            fn resolve(raw: &Self, _: Option<&PartialSpacing>) -> Result<Self, String> {
                Ok(*raw)
            }
        }
    )*};
}

plain_metric!(Px, Factor, Weight, Count, TextStyle);

impl Metric for Length {
    type Raw = LengthRef;

    fn resolve(raw: &LengthRef, spacing: Option<&PartialSpacing>) -> Result<Self, String> {
        match raw {
            LengthRef::Px(px) => Ok(Self(px.0)),
            LengthRef::Spacing(name) => spacing
                .and_then(|spacing| spacing.step(name))
                .map(|px| Self(px.0))
                .ok_or_else(|| name.clone()),
        }
    }
}

fn present<'de, D: Deserializer<'de>, T: Deserialize<'de>>(
    deserializer: D,
) -> Result<Option<Spanned<T>>, D::Error> {
    Spanned::deserialize(deserializer).map(Some)
}

macro_rules! metric_group {
    (
        $(#[$meta:meta])*
        $section:literal => $name:ident, $raw:ident, $partial:ident {
            $($(#[$field_meta:meta])* $field:ident: $ty:ty,)*
        }
    ) => {
        $(#[$meta])*
        #[derive(Clone, Copy, Debug, PartialEq)]
        pub struct $name {
            $($(#[$field_meta])* pub $field: $ty,)*
        }

        #[derive(Debug, Default, Deserialize)]
        #[serde(default)]
        pub(crate) struct $raw {
            $(
                #[serde(deserialize_with = "present")]
                $field: Option<Spanned<<$ty as Metric>::Raw>>,
            )*
        }

        #[derive(Debug, Default)]
        pub(crate) struct $partial {
            $($field: Option<$ty>,)*
        }

        impl $raw {
            /// Lo escrito, con los pasos de `spacing` resueltos.
            pub(crate) fn resolve(&self, spacing: Option<&PartialSpacing>) -> Result<$partial, UnknownStep> {
                Ok($partial {
                    $($field: self.$field.as_ref().map(|spanned| {
                        <$ty as Metric>::resolve(spanned.get_ref(), spacing).map_err(|name| UnknownStep {
                            key: concat!($section, ".", stringify!($field)).to_owned(),
                            name,
                            location: spanned.location(),
                        })
                    }).transpose()?,)*
                })
            }
        }

        impl $partial {
            /// `self` encima de `base`: lo que `self` no define sale de `base`.
            pub(crate) fn over(self, base: Self) -> Self {
                Self { $($field: self.$field.or(base.$field),)* }
            }

            /// El grupo completo, o las claves que faltan.
            pub(crate) fn complete(self, missing: &mut Vec<&'static str>) -> Option<$name> {
                $(
                    if self.$field.is_none() {
                        missing.push(concat!($section, ".", stringify!($field)));
                    }
                )*
                Some($name { $($field: self.$field?,)* })
            }
        }
    };
}

metric_group! {
    /// La escala de espaciado, en píxeles (base 4). `rows` y `layout` nombran sus pasos.
    "spacing" => Spacing, RawSpacing, PartialSpacing {
        xs: Px,
        sm: Px,
        md: Px,
        lg: Px,
    }
}

impl PartialSpacing {
    fn step(&self, name: &str) -> Option<Px> {
        match name {
            "xs" => self.xs,
            "sm" => self.sm,
            "md" => self.md,
            "lg" => self.lg,
            _ => None,
        }
    }
}

metric_group! {
    /// Los radios de las esquinas, en píxeles.
    "radius" => Radius, RawRadius, PartialRadius {
        sm: Px,
        md: Px,
        lg: Px,
    }
}

metric_group! {
    /// Los anchos de línea, en píxeles.
    "stroke" => Stroke, RawStroke, PartialStroke {
        /// Bordes y separadores.
        border: Px,
        /// El cursor de barra y el de los campos de texto.
        caret: Px,
        /// El subrayado del texto provisional y el tachado.
        text_line: Px,
    }
}

metric_group! {
    /// Los tamaños de letra, relativos a `font.size` de la config, y los pesos.
    "text" => Text, RawText, PartialText {
        /// El texto de las notas.
        editor: TextStyle,
        /// El texto de la interfaz: árbol, menús, picker, barra de estado.
        ui_md: TextStyle,
        /// Lo secundario de la interfaz: atajos, detalles y contadores.
        ui_sm: TextStyle,
        normal: Weight,
        /// La negrita del markdown y las letras que coinciden en el picker.
        strong: Weight,
    }
}

metric_group! {
    /// El alto de las filas de las listas.
    "rows" => Rows, RawRows, PartialRows {
        /// Una fila del árbol y del encabezado del panel: veces la fuente, redondeado.
        tree: Factor,
        /// Una entrada del menú contextual: veces la fuente, redondeado.
        menu: Factor,
        /// Una fila del picker: la línea de texto con esto arriba y abajo.
        picker_padding: Length,
    }
}

metric_group! {
    /// Las medidas de cada parte de la interfaz: píxeles o un paso de `spacing`.
    "layout" => Layout, RawLayout, PartialLayout {
        status_padding_x: Length,
        status_padding_y: Length,
        /// Entre los campos de la barra de estado.
        status_gap: Length,
        picker_width: Length,
        /// Del borde de arriba de la ventana al picker.
        picker_top: Length,
        /// Cuántas filas se ven antes de hacer scroll.
        picker_rows: Count,
        picker_padding_x: Length,
        /// Arriba y abajo del texto que se escribe en el picker.
        picker_input_padding_y: Length,
        /// Entre el texto de una fila del picker y su detalle, y antes del contador.
        picker_gap: Length,
        panel_padding_x: Length,
        panel_padding_y: Length,
        /// Entre el ícono y el nombre de una fila, y entre los botones.
        panel_gap: Length,
        /// La sangría de cada nivel del árbol.
        panel_indent: Length,
        /// A los lados del texto de un botón del panel.
        panel_button_padding_x: Length,
        /// Los íconos del panel: veces la fuente, redondeado.
        icon_size: Factor,
        /// La barra de botones a la izquierda del panel.
        rail_width: Length,
        rail_button_size: Length,
        menu_width: Length,
        /// Alrededor de las entradas del menú y de sus separadores.
        menu_padding: Length,
    }
}

/// Los grupos de medidas de un archivo de tema, como se escriben.
pub(crate) struct RawMetrics<'a> {
    pub(crate) spacing: &'a RawSpacing,
    pub(crate) radius: &'a RawRadius,
    pub(crate) stroke: &'a RawStroke,
    pub(crate) text: &'a RawText,
    pub(crate) rows: &'a RawRows,
    pub(crate) layout: &'a RawLayout,
}

/// Las medidas de un tema.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Metrics {
    pub spacing: Spacing,
    pub radius: Radius,
    pub stroke: Stroke,
    pub text: Text,
    pub rows: Rows,
    pub layout: Layout,
}

/// Todo menos la escala de espaciado, que se resuelve antes.
#[derive(Debug, Default)]
pub(crate) struct PartialMetrics {
    radius: PartialRadius,
    stroke: PartialStroke,
    text: PartialText,
    rows: PartialRows,
    layout: PartialLayout,
}

impl RawMetrics<'_> {
    /// La escala de espaciado de este archivo; va antes que el resto porque
    /// `rows` y `layout` nombran la escala final de toda la cadena.
    pub(crate) fn spacing(&self) -> Result<PartialSpacing, UnknownStep> {
        self.spacing.resolve(None)
    }

    pub(crate) fn resolve(&self, spacing: &PartialSpacing) -> Result<PartialMetrics, UnknownStep> {
        Ok(PartialMetrics {
            radius: self.radius.resolve(None)?,
            stroke: self.stroke.resolve(None)?,
            text: self.text.resolve(None)?,
            rows: self.rows.resolve(Some(spacing))?,
            layout: self.layout.resolve(Some(spacing))?,
        })
    }
}

impl PartialMetrics {
    pub(crate) fn over(self, base: Self) -> Self {
        Self {
            radius: self.radius.over(base.radius),
            stroke: self.stroke.over(base.stroke),
            text: self.text.over(base.text),
            rows: self.rows.over(base.rows),
            layout: self.layout.over(base.layout),
        }
    }

    /// Las medidas completas con la escala `spacing`, o las claves que faltan.
    pub(crate) fn complete(
        self,
        spacing: PartialSpacing,
        missing: &mut Vec<&'static str>,
    ) -> Option<Metrics> {
        let spacing = spacing.complete(missing);
        let radius = self.radius.complete(missing);
        let stroke = self.stroke.complete(missing);
        let text = self.text.complete(missing);
        let rows = self.rows.complete(missing);
        let layout = self.layout.complete(missing);
        Some(Metrics {
            spacing: spacing?,
            radius: radius?,
            stroke: stroke?,
            text: text?,
            rows: rows?,
            layout: layout?,
        })
    }
}
