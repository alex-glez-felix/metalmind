//! Los grupos de colores semánticos de un tema. Cada grupo existe en tres
//! formas: como se escribe (`Raw*`), resuelto pero quizá incompleto
//! (`Partial*`, para fusionar con `extends`) y completo.

use std::collections::BTreeMap;

use super::color::{Base, Color, ColorRef};
use crate::diagnostic::Location;
use crate::parse::Spanned;
use serde::{Deserialize, Deserializer};

/// Un nombre de la paleta que no existe, con dónde se escribió.
pub(crate) struct UnknownColor {
    pub(crate) key: String,
    pub(crate) name: String,
    pub(crate) location: Option<Location>,
}

pub(crate) type Palette = BTreeMap<String, Color>;

/// Un color escrito. Sin la clave se hereda; con `null` (o la clave vacía)
/// es un error, no una herencia silenciosa.
fn present<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<Spanned<ColorRef>>, D::Error> {
    Spanned::deserialize(deserializer).map(Some)
}

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
            location: spanned.location(),
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
            $(
                #[serde(deserialize_with = "present")]
                $field: Option<Spanned<ColorRef>>,
            )*
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
    /// La interfaz: editor, gutter, barra de estado, panel lateral y menús.
    "ui" => UiColors, RawUi, PartialUi {
        background,
        foreground,
        selection,
        /// Las coincidencias de la búsqueda activa.
        search,
        /// La coincidencia donde está el cursor.
        search_current,
        /// Fondo de la línea del cursor.
        current_line,
        gutter,
        /// Números de línea.
        gutter_foreground,
        /// El número de la línea del cursor.
        gutter_current,
        status_background,
        status_foreground,
        /// La barra de estado de un panel sin foco.
        status_unfocused_foreground,
        status_border,
        /// La línea entre dos paneles.
        pane_border,
        /// El `[+]` de un archivo con cambios sin guardar.
        modified,
        error,
        /// La ventana flotante del finder y de la paleta.
        picker_background,
        picker_border,
        /// La fila elegida de la lista.
        picker_selected,
        /// La fila bajo el ratón.
        picker_hover,
        /// Las letras que coinciden con lo escrito.
        picker_match,
        /// Lo secundario: carpeta de una nota, atajo de una acción, el
        /// texto de ayuda del input vacío.
        picker_muted,
        /// El panel lateral y la barra de botones que lo abre.
        panel_background,
        panel_foreground,
        /// Adjuntos, encabezado y botones inactivos del panel.
        panel_muted,
        /// La fila seleccionada con el panel enfocado.
        panel_selection,
        /// La fila seleccionada con el foco en otro lado.
        panel_selection_unfocused,
        panel_hover,
        /// La línea entre el panel y los editores.
        panel_border,
        /// El ícono de una carpeta.
        panel_folder,
        /// El ícono de una nota.
        panel_note,
        /// Las letras que coinciden con el filtro del panel.
        panel_match,
        /// Los menús contextuales.
        menu_background,
        menu_border,
        menu_hover,
    }
}

color_group! {
    /// El cursor en cada modo: bloque en Normal y Visual, barra en Insert.
    "cursor" => CursorColors, RawCursor, PartialCursor {
        normal,
        insert,
        visual,
        replace,
        /// El cursor de un panel sin foco, en cualquier modo.
        unfocused,
    }
}

color_group! {
    /// El markdown del editor: la sintaxis se ve atenuada y cada cosa con
    /// su color.
    "syntax" => SyntaxColors, RawSyntax, PartialSyntax {
        heading,
        bold,
        italic,
        /// Un wikilink o un link markdown a algo del vault, y uno externo.
        link,
        /// Un link a una nota que no existe.
        link_unresolved,
        embed,
        tag,
        code,
        /// El fondo del código inline.
        code_background,
        /// El fondo de un bloque de código cercado.
        code_block_background,
        quote,
        list_marker,
        /// `[ ]` y `[x]` de una tarea.
        checkbox,
        /// El texto de una tarea marcada.
        task_done,
        /// El fondo de `==resaltado==`.
        highlight,
        comment,
        frontmatter,
        /// Las claves del frontmatter.
        frontmatter_key,
        /// Un `^bloque`.
        block_id,
        /// El lenguaje tras la cerca de un bloque de código.
        fence_info,
        code_keyword,
        code_string,
        code_number,
        code_comment,
        /// Los marcadores (`**`, `#`, `[[`) cuando se ven.
        markup,
    }
}
