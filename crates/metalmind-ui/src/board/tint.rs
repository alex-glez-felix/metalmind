//! El color de un elemento: el que dice el archivo, resuelto contra el tema
//! al pintar, así cambiar de tema no reconstruye la escena.

use gpui_kit::{Hsla, Rgba};

use crate::theme::Theme;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Tint {
    /// Sin color en el archivo, o uno que no se entiende.
    Plain,
    /// `"1"`…`"6"`: de la paleta del tema.
    Preset(Preset),
    /// `#rrggbb`: del archivo, igual en cualquier tema.
    Custom(Hsla),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Preset {
    Red,
    Orange,
    Yellow,
    Green,
    Cyan,
    Purple,
}

impl Tint {
    #[must_use]
    pub fn parse(color: Option<&str>) -> Self {
        match color {
            None => Self::Plain,
            Some("1") => Self::Preset(Preset::Red),
            Some("2") => Self::Preset(Preset::Orange),
            Some("3") => Self::Preset(Preset::Yellow),
            Some("4") => Self::Preset(Preset::Green),
            Some("5") => Self::Preset(Preset::Cyan),
            Some("6") => Self::Preset(Preset::Purple),
            Some(hex) => {
                Rgba::try_from(hex).map_or(Self::Plain, |color| Self::Custom(Hsla::from(color)))
            }
        }
    }

    /// El color, o `plain` si el elemento no trae uno.
    fn resolve(self, plain: Hsla, theme: &Theme) -> Hsla {
        match self {
            Self::Plain => plain,
            Self::Preset(Preset::Red) => theme.ui.board_color_1,
            Self::Preset(Preset::Orange) => theme.ui.board_color_2,
            Self::Preset(Preset::Yellow) => theme.ui.board_color_3,
            Self::Preset(Preset::Green) => theme.ui.board_color_4,
            Self::Preset(Preset::Cyan) => theme.ui.board_color_5,
            Self::Preset(Preset::Purple) => theme.ui.board_color_6,
            Self::Custom(color) => color,
        }
    }
}

/// De qué color va una pieza de la escena.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Role {
    Line(Tint),
    Fill(Tint),
    Card,
    CardAccent,
    Group(Tint),
}

impl Role {
    #[must_use]
    pub fn color(self, theme: &Theme) -> Hsla {
        let faded = |tint: Tint, alpha: f32| Hsla {
            a: alpha,
            ..tint.resolve(theme.ui.board_text, theme)
        };
        match self {
            Self::Line(tint) => tint.resolve(theme.ui.board_line, theme),
            Self::Fill(Tint::Plain) => theme.ui.board_fill,
            Self::Fill(tint) => faded(tint, theme.layout.board_fill_alpha),
            Self::Card => theme.ui.board_card,
            Self::CardAccent => theme.ui.board_card_accent,
            Self::Group(Tint::Plain) => theme.ui.board_group,
            Self::Group(tint) => faded(tint, theme.layout.board_group_alpha),
        }
    }

    /// Si es el trazo, y no el relleno, de un elemento.
    #[must_use]
    pub fn is_line(self) -> bool {
        matches!(self, Self::Line(_))
    }
}

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use super::*;

    #[rstest]
    #[case(None, Tint::Plain)]
    #[case(Some("1"), Tint::Preset(Preset::Red))]
    #[case(Some("6"), Tint::Preset(Preset::Purple))]
    #[case(Some("7"), Tint::Plain)]
    #[case(Some("rojo"), Tint::Plain)]
    fn lee_el_color_del_archivo(#[case] color: Option<&str>, #[case] tint: Tint) {
        assert_eq!(Tint::parse(color), tint);
    }

    #[test]
    fn un_hex_se_queda_como_viene() {
        let Tint::Custom(color) = Tint::parse(Some("#ff0000")) else {
            panic!("un #rrggbb es un color propio");
        };
        assert_eq!(color.h, 0.0);
        assert_eq!(color.s, 1.0);
    }
}
