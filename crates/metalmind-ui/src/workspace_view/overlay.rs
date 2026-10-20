//! El picker que flota sobre los paneles: el finder, la búsqueda de texto,
//! la paleta o las notas de un link que empata, uno a la vez, centrado
//! arriba como en Zed.

use gpui_kit::{
    AnyElement, App, Entity, FocusHandle, Focusable as _, InteractiveElement as _,
    IntoElement as _, ParentElement as _, Styled as _, Subscription, div,
};

use super::finder::Finder;
use super::link_choice::LinkChoicePicker;
use super::palette::Palette;
use super::text_search::TextSearchPicker;
use crate::theme::Theme;

pub enum Overlay {
    Finder {
        picker: Entity<Finder>,
        _events: Subscription,
    },
    TextSearch {
        picker: Entity<TextSearchPicker>,
        _events: Subscription,
    },
    Palette {
        picker: Entity<Palette>,
        _events: Subscription,
    },
    /// Las notas entre las que empata un link.
    LinkChoice {
        picker: Entity<LinkChoicePicker>,
        _events: Subscription,
    },
}

impl Overlay {
    pub fn focus_handle(&self, cx: &App) -> FocusHandle {
        match self {
            Self::Finder { picker, .. } | Self::LinkChoice { picker, .. } => {
                picker.focus_handle(cx)
            }
            Self::TextSearch { picker, .. } => picker.focus_handle(cx),
            Self::Palette { picker, .. } => picker.focus_handle(cx),
        }
    }

    /// Tapa los paneles para que el ratón no les llegue mientras está abierto.
    pub fn render(&self, theme: &Theme) -> AnyElement {
        let picker = match self {
            Self::Finder { picker, .. } | Self::LinkChoice { picker, .. } => {
                picker.clone().into_any_element()
            }
            Self::TextSearch { picker, .. } => picker.clone().into_any_element(),
            Self::Palette { picker, .. } => picker.clone().into_any_element(),
        };
        div()
            .absolute()
            .inset_0()
            .occlude()
            .flex()
            .flex_col()
            .items_center()
            .pt(theme.layout.picker_top)
            .child(picker)
            .into_any_element()
    }
}
