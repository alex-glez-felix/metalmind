//! Dónde va y cómo se ve la lista de `#` y de `[[`: bajo lo escrito, o
//! encima si no cabe; un clic en una fila la acepta, igual que en las de
//! `:`.

use std::ops::Range;

use gpui_kit::{
    AnyElement, App, Bounds, Context, Div, InteractiveElement as _, IntoElement, MouseButton,
    ParentElement as _, Pixels, Point, Stateful, StatefulInteractiveElement as _, anchored, size,
};
use metalmind_keymap::{EditorAction, View};

use super::super::Editor;
use super::super::completion_menu::{CompletionMenu, MenuMetrics, MenuRow};
use super::super::cursor::cursor_position;
use super::super::layout::FrameLayout;
use super::{List, rows};
use crate::actions::AppKeymap;
use crate::theme::Theme;

impl Editor {
    /// La lista bajo lo escrito, o encima si abajo no cabe en el texto del
    /// panel (ver [`MenuMetrics::place`]), sobre `layout`: el del frame que
    /// se está pintando. No pasa de `max_width`, el ancho de la ventana.
    pub(in crate::editor) fn completion_overlay(
        &mut self,
        layout: &FrameLayout,
        max_width: Pixels,
        theme: &Theme,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let list = self.completion.list.as_ref()?;
        let typed = list.typed_in_text()?.len();
        let wanted = usize::from(theme.layout.completion_rows).min(list.len());
        let view = self.view(cx)?;
        let (laid, column) = cursor_position(&view, layout)?;
        let row = laid.row_of(column)?;
        let origin = laid.row_origin(row);
        let x = origin.x + row.x_for_index(column - typed.min(column));
        let line = Bounds::new(origin, size(row.width(), row.height));
        let placement = MenuMetrics {
            row_height: theme.menu_row_height,
            frame: theme.layout.menu_padding + theme.stroke.border,
        }
        .place(wanted, line, layout.viewport.bounds);
        let list = self.completion.list.as_mut()?;
        let visible = list.selection_mut().visible(placement.rows);
        let rows = match &*list {
            List::Tags(list) => rows::tags(list, visible.clone(), theme),
            List::Links(list) => rows::links(list, visible.clone(), theme),
            List::Command(_) | List::Argument(_) => return None,
        };
        let selected = list.selection().selected();
        let accept_keys = Self::accept_keys(View::Completion, cx);
        let menu = Self::menu(rows, visible, selected, accept_keys, max_width, theme, cx);
        Some(
            anchored()
                .position(Point { x, y: placement.y })
                .snap_to_window()
                .child(menu)
                .into_any_element(),
        )
    }

    /// Las teclas que aceptan lo elegido en una lista de `in_view`.
    pub(super) fn accept_keys(in_view: View, cx: &App) -> Option<String> {
        AppKeymap::get(cx).keys_for(EditorAction::AcceptCompletion.into(), |view| {
            view == in_view
        })
    }

    /// La lista abierta con sus `rows`: un clic en una la acepta.
    fn menu(
        rows: Vec<MenuRow>,
        visible: Range<usize>,
        selected: usize,
        accept_keys: Option<String>,
        max_width: Pixels,
        theme: &Theme,
        cx: &mut Context<Self>,
    ) -> Div {
        CompletionMenu {
            rows,
            visible,
            selected,
            accept_keys,
            max_width,
        }
        .render(theme, cx, Self::accept_on_click)
    }

    /// Un clic en la fila `row` la elige y la acepta.
    pub(in crate::editor) fn accept_on_click(
        row: usize,
        element: Stateful<Div>,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        element
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .on_click(cx.listener(move |editor, _, window, cx| {
                if let Some(list) = &mut editor.completion.list {
                    list.selection_mut().select(row);
                }
                // El foco pudo irse (al panel lateral) con la lista abierta.
                editor.take_focus(window, cx);
                editor.accept_completion(window, cx);
            }))
    }
}
