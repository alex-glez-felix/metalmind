//! Lo que flota sobre los paneles: el finder, la búsqueda de texto, la
//! paleta, los links sin destino, las notas de un link que empata, las
//! plantillas, el nombre de una nota nueva, la fuente de la lista o la vista previa de un cambio
//! de tags, uno a la vez, centrado arriba como en Zed.

use std::sync::Arc;

use gpui_kit::{
    AnyElement, App, Entity, FocusHandle, Focusable as _, InteractiveElement as _,
    IntoElement as _, ParentElement as _, Styled as _, Subscription, Task, div,
};
use metalmind_vault::LinkIndex;

use super::finder::Finder;
use super::link_choice::LinkChoicePicker;
use super::new_note::{NamePicker, TemplatePicker};
use super::note_list::SourcePicker;
use super::palette::Palette;
use super::text_search::TextSearchPicker;
use super::unresolved::UnresolvedPicker;
use crate::tag_change::TagChangeView;
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
    /// Los links sin destino del vault, sacados de `index`. `refresh` rehace
    /// la lista para un índice más nuevo; soltarlo la cancela.
    Unresolved {
        picker: Entity<UnresolvedPicker>,
        index: Arc<LinkIndex>,
        refresh: Option<Task<()>>,
        _events: Subscription,
    },
    /// Las notas entre las que empata un link.
    LinkChoice {
        picker: Entity<LinkChoicePicker>,
        _events: Subscription,
    },
    /// Las plantillas para una nota nueva.
    Templates {
        picker: Entity<TemplatePicker>,
        _events: Subscription,
    },
    /// El nombre de una nota nueva.
    NoteName {
        picker: Entity<NamePicker>,
        _events: Subscription,
    },
    /// De dónde salen las notas de la lista.
    ListSource {
        picker: Entity<SourcePicker>,
        _events: Subscription,
    },
    /// La vista previa de renombrar o fusionar tags.
    TagChange {
        view: Entity<TagChangeView>,
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
            Self::Unresolved { picker, .. } => picker.focus_handle(cx),
            Self::Palette { picker, .. } => picker.focus_handle(cx),
            Self::Templates { picker, .. } => picker.focus_handle(cx),
            Self::NoteName { picker, .. } => picker.focus_handle(cx),
            Self::ListSource { picker, .. } => picker.focus_handle(cx),
            Self::TagChange { view, .. } => view.focus_handle(cx),
        }
    }

    /// Tapa los paneles para que el ratón no les llegue mientras está abierto.
    pub fn render(&self, theme: &Theme) -> AnyElement {
        let picker = match self {
            Self::Finder { picker, .. } | Self::LinkChoice { picker, .. } => {
                picker.clone().into_any_element()
            }
            Self::TextSearch { picker, .. } => picker.clone().into_any_element(),
            Self::Unresolved { picker, .. } => picker.clone().into_any_element(),
            Self::Palette { picker, .. } => picker.clone().into_any_element(),
            Self::Templates { picker, .. } => picker.clone().into_any_element(),
            Self::NoteName { picker, .. } => picker.clone().into_any_element(),
            Self::ListSource { picker, .. } => picker.clone().into_any_element(),
            Self::TagChange { view, .. } => view.clone().into_any_element(),
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
