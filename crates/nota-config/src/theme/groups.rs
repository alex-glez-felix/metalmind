//! Los grupos de colores semánticos de un tema. Cada grupo existe en tres
//! formas: como se escribe (`Raw*`), resuelto pero quizá incompleto
//! (`Partial*`, para fusionar con `extends`) y completo.

use std::collections::BTreeMap;

use serde::Deserialize;
use toml::Spanned;

use super::color::{Base, Color, ColorRef};

/// Un nombre de la paleta que no existe, con dónde se escribió.
pub(crate) struct UnknownColor {
    pub(crate) key: String,
    pub(crate) name: String,
    pub(crate) offset: usize,
}

pub(crate) type Palette = BTreeMap<String, Color>;

fn resolve(
    field: Option<&Spanned<ColorRef>>,
    key: String,
    palette: &Palette,
) -> Result<Option<Color>, UnknownColor> {
    let Some(spanned) = field else {
        return Ok(None);
    };
    let ColorRef { base, alpha } = spanned.get_ref();
    let color = match base {
        Base::Literal(color) => *color,
        Base::Palette(name) => *palette.get(name).ok_or_else(|| UnknownColor {
            key,
            name: name.clone(),
            offset: spanned.span().start,
        })?,
    };
    Ok(Some(Color {
        alpha: color.alpha * alpha,
        ..color
    }))
}

macro_rules! color_group {
    (
        $(#[$meta:meta])*
        $section:literal => $name:ident, $raw:ident, $partial:ident {
            $($(#[$field_meta:meta])* $field:ident,)*
        }
    ) => {
        $(#[$meta])*
        #[derive(Clone, Copy, Debug, PartialEq)]
        pub struct $name {
            $($(#[$field_meta])* pub $field: Color,)*
        }

        #[derive(Debug, Default, Deserialize)]
        #[serde(default)]
        pub(crate) struct $raw {
            $($field: Option<Spanned<ColorRef>>,)*
        }

        #[derive(Debug, Default)]
        pub(crate) struct $partial {
            $($field: Option<Color>,)*
        }

        impl $raw {
            pub(crate) fn resolve(&self, palette: &Palette) -> Result<$partial, UnknownColor> {
                Ok($partial {
                    $($field: resolve(
                        self.$field.as_ref(),
                        concat!($section, ".", stringify!($field)).to_owned(),
                        palette,
                    )?,)*
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

color_group! {
    /// La interfaz: editor, gutter y barra de estado.
    "ui" => UiColors, RawUi, PartialUi {
        background,
        foreground,
        selection,
        /// Fondo de la línea del cursor.
        current_line,
        gutter,
        /// Números de línea.
        gutter_foreground,
        status_background,
        status_foreground,
        status_border,
        /// El `[+]` de un archivo con cambios sin guardar.
        modified,
        error,
    }
}

color_group! {
    /// El cursor en cada modo: bloque en Normal y Visual, barra en Insert.
    "cursor" => CursorColors, RawCursor, PartialCursor {
        normal,
        insert,
        visual,
        replace,
    }
}

color_group! {
    /// Markdown para el live preview (M4).
    "syntax" => SyntaxColors, RawSyntax, PartialSyntax {
        heading,
        bold,
        italic,
        link,
        tag,
        code,
        quote,
        list_marker,
        highlight,
        comment,
        frontmatter,
        /// Los marcadores (`**`, `#`, `[[`) cuando se ven.
        markup,
    }
}
