//! El indicador del focus (05 §3), igual en la barra de estado y en el
//! panel: el alcance escrito tal cual, y un clic lo quita como `:unfocus`.
//! Si no cabe, el tooltip lo muestra entero.

use gpui_kit::{
    Div, ElementId, InteractiveElement as _, ParentElement as _, SharedString, Stateful,
    StatefulInteractiveElement as _, Styled as _, div,
};
use metalmind_keymap::Command;

use crate::actions::Dispatch;
use crate::theme::Theme;
use crate::tooltip::Tooltip;

pub fn focus_indicator(
    id: impl Into<ElementId>,
    label: SharedString,
    theme: &Theme,
) -> Stateful<Div> {
    div()
        .id(id)
        .min_w_0()
        .truncate()
        .text_color(theme.ui.focus_indicator)
        .cursor_pointer()
        .on_click(|_, window, cx| {
            window.dispatch_action(Box::new(Dispatch(Command::ClearFocus.into())), cx);
        })
        .tooltip({
            let text: SharedString = format!("{label} · clic para salir (:unfocus)").into();
            move |_, cx| Tooltip::view(text.clone(), cx)
        })
        .child(label)
}
