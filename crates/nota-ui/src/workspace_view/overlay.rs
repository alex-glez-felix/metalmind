//! El picker que flota sobre los paneles: el finder o la paleta, uno a la
//! vez, centrado arriba como en Zed.

use gpui_kit::{
    AnyElement, App, Entity, FocusHandle, Focusable as _, InteractiveElement as _,
    IntoElement as _, ParentElement as _, Styled as _, Subscription, div,
};

use crate::finder::Note;
use crate::palette::Entry;
use crate::picker::PickerView;
use crate::theme::Theme;

pub enum Overlay {
    Finder {
        picker: Entity<PickerView<Note>>,
        _events: Subscription,
    },
    Palette {
        picker: Entity<PickerView<Entry>>,
        _events: Subscription,
    },
}

impl Overlay {
    pub fn focus_handle(&self, cx: &App) -> FocusHandle {
        match self {
            Self::Finder { picker, .. } => picker.focus_handle(cx),
            Self::Palette { picker, .. } => picker.focus_handle(cx),
        }
    }

    /// Tapa los paneles para que el ratón no les llegue mientras está abierto.
    pub fn render(&self, theme: &Theme) -> AnyElement {
        let picker = match self {
            Self::Finder { picker, .. } => picker.clone().into_any_element(),
            Self::Palette { picker, .. } => picker.clone().into_any_element(),
        };
        div()
            .absolute()
            .inset_0()
            .occlude()
            .flex()
            .flex_col()
            .items_center()
            .pt(theme.picker_top)
            .child(picker)
            .into_any_element()
    }
}
