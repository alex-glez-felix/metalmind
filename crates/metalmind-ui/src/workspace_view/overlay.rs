//! Lo que flota sobre los paneles: el finder, la búsqueda de texto, la
//! paleta, el historial de avisos, los links sin destino, las notas de un link que empata, las
//! plantillas, el nombre de una nota nueva, la fuente de la lista, el nombre de una vista guardada o la vista previa de un cambio
//! en el vault, uno a la vez, centrado arriba como en Zed.

use gpui_kit::{
    AnyElement, AnyView, App, Entity, FocusHandle, Focusable, InteractiveElement as _,
    IntoElement as _, ParentElement as _, Render, Styled as _, Subscription, div,
};

use super::unresolved::UnresolvedLinks;
use crate::theme::Theme;

pub struct Overlay {
    view: AnyView,
    focus: FocusHandle,
    /// Lo que le pasa a `view` mientras está abierta.
    _events: Vec<Subscription>,
    /// Si son los links sin destino, lo que hace falta para rehacerlos.
    pub(super) unresolved: Option<UnresolvedLinks>,
}

impl Overlay {
    pub fn new<V: Render + Focusable>(
        view: Entity<V>,
        events: Vec<Subscription>,
        cx: &App,
    ) -> Self {
        Self {
            focus: view.focus_handle(cx),
            view: view.into(),
            _events: events,
            unresolved: None,
        }
    }

    #[cfg(test)]
    pub fn view(&self) -> AnyView {
        self.view.clone()
    }

    pub fn focus_handle(&self) -> FocusHandle {
        self.focus.clone()
    }

    /// Tapa los paneles para que el ratón no les llegue mientras está abierto.
    pub fn render(&self, theme: &Theme) -> AnyElement {
        div()
            .absolute()
            .inset_0()
            .occlude()
            .flex()
            .flex_col()
            .items_center()
            .pt(theme.layout.picker_top)
            .child(self.view.clone())
            .into_any_element()
    }
}
