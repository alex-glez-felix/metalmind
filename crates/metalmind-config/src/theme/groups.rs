//! Los grupos de colores semánticos de un tema. Cada grupo existe en tres
//! formas: como se escribe (`Raw*`), resuelto pero quizá incompleto
//! (`Partial*`, para fusionar con `extends`) y completo, en las unidades de
//! quien lo usa (ver [`Represent`]).

use std::collections::BTreeMap;

use super::color::{Base, Color, ColorRef};
use super::represent::{Plain, Represent, Units};
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
        pub struct $name<U: Units = Plain> {
            $($(#[$field_meta])* pub $field: <U as Represent<Color>>::Value,)*
        }

        impl $name {
            /// El grupo en las unidades de `U`.
            #[must_use]
            pub fn convert<U: Units>(self) -> $name<U> {
                $name { $($field: <U as Represent<Color>>::represent(self.$field),)* }
            }
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
    /// Los colores por su papel. Se escriben con la paleta, y `ui`, `cursor`
    /// y `syntax` los nombran como a un color de la paleta: cambiar `accent`
    /// cambia todo lo que lo usa.
    "roles" => RoleColors, RawRoles, PartialRoles {
        /// El fondo del editor.
        background,
        /// El fondo de los paneles, las barras y lo que flota.
        surface,
        /// Lo que se levanta sobre `surface`: hover, la línea del cursor.
        raised,
        text,
        /// Lo secundario: atajos, detalles, encabezados.
        text_muted,
        /// Lo que casi no se ve: números de línea, la sintaxis atenuada.
        text_faint,
        /// Lo que está pero no tiene el foco.
        text_inactive,
        /// El color de la app: cursor, coincidencias, lo elegido.
        accent,
        /// La línea entre dos superficies.
        border_subtle,
        /// Una línea que tiene que verse, como la que separa paneles.
        border_strong,
        /// Error y lo que destruye.
        danger,
        /// Aviso y lo pendiente, como cambios sin guardar.
        warning,
    }
}

impl PartialRoles {
    pub(crate) const NAMES: [&'static str; 12] = [
        "background",
        "surface",
        "raised",
        "text",
        "text_muted",
        "text_faint",
        "text_inactive",
        "accent",
        "border_subtle",
        "border_strong",
        "danger",
        "warning",
    ];

    /// Cada rol con su nombre, para resolver los grupos que lo nombran.
    pub(crate) fn named(&self) -> [(&'static str, Option<Color>); 12] {
        let colors = [
            self.background,
            self.surface,
            self.raised,
            self.text,
            self.text_muted,
            self.text_faint,
            self.text_inactive,
            self.accent,
            self.border_subtle,
            self.border_strong,
            self.danger,
            self.warning,
        ];
        let mut named = Self::NAMES.map(|name| (name, None));
        for ((_, slot), color) in named.iter_mut().zip(colors) {
            *slot = color;
        }
        named
    }
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
        /// La letra de una mark en el margen.
        gutter_mark,
        status_background,
        status_foreground,
        /// La barra de estado de un panel sin foco.
        status_unfocused_foreground,
        status_border,
        /// La línea entre dos paneles.
        pane_border,
        /// El `[+]` de un archivo con cambios sin guardar.
        modified,
        /// El indicador del autoguardado: "guardado".
        status_saved,
        /// El indicador del autoguardado: "sin guardar".
        status_unsaved,
        /// El indicador del autoguardado: el archivo cambió fuera con cambios
        /// sin guardar.
        status_changed_on_disk,
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
        /// Lo escrito que no va a encontrar nada, como un tag que no existe.
        picker_warning,
        /// Lo secundario: carpeta de una nota, atajo de una acción, el
        /// texto de ayuda del input vacío.
        picker_muted,
        /// El alcance del modo focus en la barra de estado, el panel y los
        /// pickers.
        focus_indicator,
        /// El panel lateral y la barra de botones que lo abre.
        panel_background,
        panel_foreground,
        /// Adjuntos, encabezado y botones inactivos del panel.
        panel_muted,
        /// La fila seleccionada con el panel enfocado.
        panel_selection,
        /// El borde de la fila seleccionada con el foco en otro lado.
        panel_selection_unfocused,
        /// La fila de la nota abierta en el panel de edición enfocado, y el
        /// botón de la vista que se ve.
        panel_active,
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
        /// Los avisos en la esquina de arriba a la derecha.
        toast_background,
        toast_border,
        /// El ícono de cada nivel de aviso.
        toast_info,
        toast_warning,
        toast_error,
        /// La tecla que cierra los toasts.
        toast_hint,
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
        /// El LaTeX de una fórmula cuando se ve su fuente.
        math,
        /// Los marcadores (`**`, `#`, `[[`) cuando se ven.
        markup,
    }
}
