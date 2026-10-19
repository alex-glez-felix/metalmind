//! Cómo se ve el panel. El árbol va en una `uniform_list`: solo se pintan
//! las filas visibles, tenga el vault las notas que tenga.

use std::ops::Range;

use gpui_kit::{
    Action, AnyElement, ClickEvent, Context, Div, HighlightStyle, InteractiveElement as _,
    IntoElement, MouseButton, MouseDownEvent, ParentElement as _, Pixels, Point, Render,
    SharedString, StatefulInteractiveElement as _, Styled as _, StyledText, Window, div,
    prelude::FluentBuilder as _, uniform_list,
};
use metalmind_vault::{EntryKind, VaultPath};

use super::actions::{
    CONFIRM_CONTEXT, FocusEditor, INPUT_CONTEXT, MENU_CONTEXT, NewFolder, NewNote, TREE_CONTEXT,
    ToggleView,
};
use super::menu::ContextMenu;
use super::{Sidebar, SidebarEvent, icons};
use crate::file_tree::Row;
use crate::panel::{Fill, PanelView, Prompt, RowMarks};
use crate::theme::Theme;

impl Render for Sidebar {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = Theme::get(cx).clone();
        let mut root = div()
            .key_context(self.key_context())
            .track_focus(&self.focus_handle)
            .w(theme.panel_width)
            .h_full()
            .flex_none()
            .flex()
            .flex_col()
            .bg(theme.panel_background)
            .text_color(theme.panel_foreground)
            .border_r(theme.border_width)
            .border_color(theme.panel_border)
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|sidebar, _, window, cx| sidebar.focus(window, cx)),
            )
            .on_key_down(cx.listener(Self::on_key_down));
        root = Self::register_actions(root, cx);
        let focused = self.focus_handle.contains_focused(window, cx);
        root.child(self.header(&theme, cx))
            .children(self.prompt_view(&theme, cx))
            .child(self.body(&theme, focused, cx))
            .children(self.panel.error().map(|error| {
                div()
                    .px(theme.panel_padding_x)
                    .py(theme.panel_padding_y)
                    .text_color(theme.error)
                    .child(error.to_owned())
            }))
            .children(self.menu.as_ref().map(|menu| menu.render(&theme, cx)))
    }
}

impl Sidebar {
    fn key_context(&self) -> &'static str {
        if self.menu.is_some() {
            return MENU_CONTEXT;
        }
        match self.panel.prompt() {
            Some(Prompt::ConfirmDelete { .. }) => CONFIRM_CONTEXT,
            Some(_) => INPUT_CONTEXT,
            None => TREE_CONTEXT,
        }
    }

    fn focus(&self, window: &mut Window, cx: &mut Context<Self>) {
        window.focus(&self.focus_handle, cx);
    }

    fn register_actions(root: Div, cx: &mut Context<Self>) -> Div {
        root.on_action(cx.listener(Self::select_next))
            .on_action(cx.listener(Self::select_previous))
            .on_action(cx.listener(Self::select_first))
            .on_action(cx.listener(Self::select_last))
            .on_action(cx.listener(Self::collapse_or_parent))
            .on_action(cx.listener(Self::expand_or_open))
            .on_action(cx.listener(Self::open_selected))
            .on_action(cx.listener(Self::open_split_right))
            .on_action(cx.listener(Self::open_split_down))
            .on_action(cx.listener(Self::filter))
            .on_action(cx.listener(Self::new_note))
            .on_action(cx.listener(Self::new_folder))
            .on_action(cx.listener(Self::rename))
            .on_action(cx.listener(Self::move_entry))
            .on_action(cx.listener(Self::delete))
            .on_action(cx.listener(Self::confirm))
            .on_action(cx.listener(Self::cancel))
            .on_action(cx.listener(Self::menu_next))
            .on_action(cx.listener(Self::menu_previous))
            .on_action(cx.listener(Self::toggle_view))
            .on_action(cx.listener(Self::hide))
            .on_action(cx.listener(|_, _: &FocusEditor, _, cx| {
                cx.emit(SidebarEvent::FocusEditor);
            }))
    }

    fn header(&self, theme: &Theme, cx: &mut Context<Self>) -> Div {
        let title = match self.panel.view {
            PanelView::Folders => "Carpetas",
            PanelView::Tags => "Tags",
        };
        let buttons = div()
            .flex()
            .gap(theme.panel_gap)
            .when(self.panel.view == PanelView::Folders, |buttons| {
                buttons
                    .child(button("new-note", "+nota", NewNote, theme, cx))
                    .child(button("new-folder", "+carpeta", NewFolder, theme, cx))
            })
            .child(button("toggle-view", "T", ToggleView, theme, cx));
        div()
            .flex()
            .flex_none()
            .justify_between()
            .items_center()
            .h(theme.panel_row_height)
            .px(theme.panel_padding_x)
            .text_color(theme.panel_muted)
            .child(title)
            .child(buttons)
    }

    fn body(&self, theme: &Theme, focused: bool, cx: &mut Context<Self>) -> AnyElement {
        let empty = |text: &'static str| {
            div()
                .flex_1()
                .px(theme.panel_padding_x)
                .py(theme.panel_padding_y)
                .text_color(theme.panel_muted)
                .child(text)
                .into_any_element()
        };
        if self.panel.view == PanelView::Tags {
            return empty("La vista de tags llega con el índice de tags (M2.5).");
        }
        let rows = self.panel.tree.rows().len();
        if rows == 0 {
            return empty(if self.panel.tree.is_filtering() {
                "Nada coincide con el filtro."
            } else {
                "El vault está vacío."
            });
        }
        let active = self.active(cx);
        let theme = theme.clone();
        uniform_list(
            "file-tree",
            rows,
            cx.processor(move |sidebar, range: Range<usize>, _, cx| {
                sidebar.rows(range, focused, active.as_ref(), &theme, cx)
            }),
        )
        .track_scroll(&self.scroll)
        .flex_1()
        .into_any_element()
    }

    fn rows(
        &self,
        range: Range<usize>,
        focused: bool,
        active: Option<&VaultPath>,
        theme: &Theme,
        cx: &mut Context<Self>,
    ) -> Vec<AnyElement> {
        let selected = self.panel.tree.selected_row();
        self.panel.tree.rows()[range.clone()]
            .iter()
            .zip(range)
            .map(|(row, at)| {
                let marks = RowMarks::of(selected == Some(at), focused, active == Some(&row.path));
                row_view(row, at, marks, theme, cx)
            })
            .collect()
    }

    fn click(
        &mut self,
        at: usize,
        event: &ClickEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.focus(window, cx);
        if let Some(click) = self.panel.click(at, event.click_count()) {
            let placement = crate::workspace::Placement::Current;
            self.open(&click.path, placement, click.focus_editor, cx);
        }
        cx.notify();
    }

    fn open_menu(
        &mut self,
        at: usize,
        position: Point<Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.focus(window, cx);
        self.panel.tree.select_row(at);
        if let Some(row) = self.panel.tree.selected_entry() {
            self.menu = Some(ContextMenu::new(position, row.path.clone(), row.kind));
        }
        cx.notify();
    }
}

fn row_view(
    row: &Row,
    at: usize,
    marks: RowMarks,
    theme: &Theme,
    cx: &mut Context<Sidebar>,
) -> AnyElement {
    let depth = u16::try_from(row.depth).unwrap_or(u16::MAX);
    let indent = theme.panel_padding_x + theme.panel_indent * f32::from(depth);
    let text_color = if row.kind == EntryKind::Attachment {
        theme.panel_muted
    } else {
        theme.panel_foreground
    };
    div()
        .id(("file-tree-row", at))
        .h(theme.panel_row_height)
        .w_full()
        .flex()
        .items_center()
        .gap(theme.panel_gap)
        .pl(indent)
        .pr(theme.panel_padding_x)
        .text_color(text_color)
        .map(|element| match marks.fill {
            Some(Fill::Selection) => element.bg(theme.panel_selection),
            Some(Fill::Active) => element.bg(theme.panel_active),
            None => element.hover(|element| element.bg(theme.panel_hover)),
        })
        .border(theme.border_width)
        .border_color(if marks.outline {
            theme.panel_selection_unfocused
        } else {
            gpui_kit::transparent_black()
        })
        .on_click(cx.listener(move |sidebar, event: &ClickEvent, window, cx| {
            sidebar.click(at, event, window, cx);
        }))
        .on_mouse_down(
            MouseButton::Right,
            cx.listener(move |sidebar, event: &MouseDownEvent, window, cx| {
                sidebar.open_menu(at, event.position, window, cx);
            }),
        )
        .child(icons::disclosure(row.kind, row.expanded, theme))
        .child(icons::entry(row.kind, theme))
        .child(div().min_w_0().truncate().child(name(row, theme)))
        .into_any_element()
}

/// El nombre, con las letras que coinciden con el filtro resaltadas.
fn name(row: &Row, theme: &Theme) -> StyledText {
    let name = row.name();
    let style = HighlightStyle {
        color: Some(theme.panel_match),
        ..HighlightStyle::default()
    };
    let highlights: Vec<(Range<usize>, HighlightStyle)> = row
        .highlights
        .iter()
        .filter_map(|&start| {
            let width = name.get(start..)?.chars().next()?.len_utf8();
            Some((start..start + width, style))
        })
        .collect();
    StyledText::new(SharedString::from(name.to_owned())).with_highlights(highlights)
}

/// Un botón de texto que invoca `action` en el panel.
pub(super) fn button<A: Action + Clone>(
    id: &'static str,
    label: &'static str,
    action: A,
    theme: &Theme,
    cx: &mut Context<Sidebar>,
) -> AnyElement {
    div()
        .id(id)
        .px(theme.panel_button_padding_x)
        .rounded(theme.radius_md)
        .text_color(theme.panel_muted)
        .hover(|button| {
            button
                .bg(theme.panel_hover)
                .text_color(theme.panel_foreground)
        })
        .on_click(cx.listener(move |sidebar, _, window, cx| {
            sidebar.focus(window, cx);
            window.dispatch_action(action.boxed_clone(), cx);
        }))
        .child(label)
        .into_any_element()
}
