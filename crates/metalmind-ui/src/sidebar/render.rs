//! Cómo se ve el panel. El árbol va en una `uniform_list`: solo se pintan
//! las filas visibles, tenga el vault las notas que tenga.

use std::ops::Range;

use gpui_kit::{
    AnyElement, App, ClickEvent, Context, Div, HighlightStyle, InteractiveElement as _,
    IntoElement, MouseButton, MouseDownEvent, ParentElement as _, Pixels, Point, Render,
    SharedString, Stateful, StatefulInteractiveElement as _, Styled as _, StyledText, Window, div,
    prelude::FluentBuilder as _, uniform_list,
};
use metalmind_vault::{EntryKind, VaultPath};

use metalmind_keymap::{Action, PanelAction, context};

use super::menu::ContextMenu;
use super::{Sidebar, icons};
use crate::actions::Dispatch;
use crate::file_tree::Row;
use crate::focus_indicator::focus_indicator;
use crate::panel::{Fill, Outline as _, PanelView, Prompt, RowMarks};
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
            .bg(theme.ui.panel_background)
            .text_color(theme.ui.panel_foreground)
            .border_r(theme.stroke.border)
            .border_color(theme.ui.panel_border)
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|sidebar, _, window, cx| sidebar.focus(window, cx)),
            )
            .on_key_down(cx.listener(Self::on_key_down));
        root = Self::register_actions(root, cx);
        let focused = self.focus_handle.contains_focused(window, cx);
        root.child(self.header(&theme, cx))
            .children(self.focus_row(&theme, cx))
            .children(self.prompt_view(&theme, cx))
            .child(self.body(&theme, focused, cx))
            .children(self.panel.error().map(|error| {
                div()
                    .px(theme.layout.panel_padding_x)
                    .py(theme.layout.panel_padding_y)
                    .text_color(theme.ui.error)
                    .child(error.to_owned())
            }))
            .children(
                self.menu
                    .as_ref()
                    .map(|menu| menu.render(&theme, window, cx)),
            )
    }
}

impl Sidebar {
    fn key_context(&self) -> &'static str {
        if self.menu.is_some() {
            return context::FILE_TREE_MENU;
        }
        match self.panel.prompt() {
            Some(Prompt::ConfirmDelete { .. }) => context::FILE_TREE_CONFIRM,
            Some(_) => context::FILE_TREE_INPUT,
            None => context::FILE_TREE,
        }
    }

    fn focus(&self, window: &mut Window, cx: &mut Context<Self>) {
        window.focus(&self.focus_handle, cx);
    }

    fn register_actions(root: Div, cx: &mut Context<Self>) -> Div {
        root.on_action(cx.listener(Self::on_dispatch))
    }

    /// Si la carpeta o el tag elegido es el focus de ahora.
    fn selected_is_focus(&self, cx: &App) -> bool {
        let current = self.session.read(cx).focus();
        self.panel
            .selected_focus()
            .zip(current)
            .is_some_and(|(chosen, current)| chosen.same_scope(current))
    }

    /// Con focus, su alcance bajo el encabezado, como en la barra de estado.
    fn focus_row(&self, theme: &Theme, cx: &App) -> Option<Div> {
        let label = self.session.read(cx).focus()?.label();
        Some(
            div()
                .flex()
                .flex_none()
                .items_center()
                .h(theme.panel_row_height)
                .px(theme.layout.panel_padding_x)
                .child(focus_indicator("panel-focus", label.into(), theme)),
        )
    }

    fn header(&self, theme: &Theme, cx: &mut Context<Self>) -> Div {
        let title = match self.panel.view {
            PanelView::Folders => "Carpetas",
            PanelView::Tags => "Tags",
            PanelView::Backlinks => "Backlinks",
        };
        let buttons = div()
            .flex()
            .gap(theme.layout.panel_gap)
            .when(self.panel.view == PanelView::Folders, |buttons| {
                buttons
                    .child(button("new-note", "+nota", PanelAction::NewNote, theme, cx))
                    .child(button(
                        "new-folder",
                        "+carpeta",
                        PanelAction::NewFolder,
                        theme,
                        cx,
                    ))
            })
            .child(button(
                "toggle-view",
                "T",
                PanelAction::ToggleView,
                theme,
                cx,
            ));
        div()
            .flex()
            .flex_none()
            .justify_between()
            .items_center()
            .h(theme.panel_row_height)
            .px(theme.layout.panel_padding_x)
            .text_color(theme.ui.panel_muted)
            .child(title)
            .child(buttons)
    }

    fn body(&self, theme: &Theme, focused: bool, cx: &mut Context<Self>) -> AnyElement {
        let empty = |text: &'static str| {
            div()
                .flex_1()
                .px(theme.layout.panel_padding_x)
                .py(theme.layout.panel_padding_y)
                .text_color(theme.ui.panel_muted)
                .child(text)
                .into_any_element()
        };
        match self.panel.view {
            PanelView::Tags => return self.tag_body(theme, focused, cx),
            PanelView::Backlinks => return self.backlink_body(theme, focused, cx),
            PanelView::Folders => {}
        }
        let rows = self.panel.tree.rows().len();
        if rows == 0 {
            return empty(if self.panel.tree.is_filtering() {
                "Nada coincide con el filtro."
            } else if self.session.read(cx).focus().is_some() {
                "No hay nada en el focus."
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
        // Una fila cortada por el borde se ve entera al elegirla, como con
        // `j` y `k`.
        let click = self.panel.click(at, event.click_count());
        self.scroll_to_selection();
        if let Some(click) = click {
            let placement = crate::workspace::Placement::Current;
            self.open(&click.path, click.at, placement, click.focus_editor, cx);
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
        self.panel.outline_mut().select_row(at);
        self.scroll_to_selection();
        let focus = self.selected_is_focus(cx);
        self.menu = match self.panel.view {
            PanelView::Folders => self.panel.tree.selected_entry().map(|row| {
                let menu = ContextMenu::new(position, row.path.clone(), row.kind);
                if row.is_folder() {
                    menu.with_focus(focus)
                } else {
                    menu
                }
            }),
            PanelView::Tags => self.panel.tags.selected_entry().and_then(|row| {
                match (row.note(), row.tag_name()) {
                    (Some(note), _) => Some(ContextMenu::on_note(position, note.clone())),
                    (None, Some(tag)) => {
                        Some(ContextMenu::on_tag(position, tag.to_owned()).with_focus(focus))
                    }
                    (None, None) => None,
                }
            }),
            PanelView::Backlinks => self
                .panel
                .backlinks
                .selected_entry()
                .map(|row| ContextMenu::on_note(position, row.source().clone())),
        };
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
    let text_color = if row.kind == EntryKind::Attachment {
        theme.ui.panel_muted
    } else {
        theme.ui.panel_foreground
    };
    row_frame(at, row.depth, marks, theme, cx)
        .text_color(text_color)
        .child(icons::disclosure(row.kind, row.expanded, theme))
        .child(icons::entry(row.kind, theme))
        .child(div().min_w_0().truncate().child(name(row, theme)))
        .into_any_element()
}

/// Una fila de cualquiera de las dos vistas a `depth` niveles: su fondo y
/// borde según `marks`, el clic y el clic derecho.
pub(super) fn row_frame(
    at: usize,
    depth: usize,
    marks: RowMarks,
    theme: &Theme,
    cx: &mut Context<Sidebar>,
) -> Stateful<Div> {
    let depth = u16::try_from(depth).unwrap_or(u16::MAX);
    let indent = theme.layout.panel_padding_x + theme.layout.panel_indent * f32::from(depth);
    div()
        .id(("panel-row", at))
        .h(theme.panel_row_height)
        .w_full()
        .flex()
        .items_center()
        .gap(theme.layout.panel_gap)
        .pl(indent)
        .pr(theme.layout.panel_padding_x)
        .map(|element| match marks.fill {
            Some(Fill::Selection) => element.bg(theme.ui.panel_selection),
            Some(Fill::Active) => element.bg(theme.ui.panel_active),
            None => element.hover(|element| element.bg(theme.ui.panel_hover)),
        })
        .border(theme.stroke.border)
        .border_color(if marks.outline {
            theme.ui.panel_selection_unfocused
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
}

/// El nombre, con las letras que coinciden con el filtro resaltadas.
fn name(row: &Row, theme: &Theme) -> StyledText {
    let name = row.name();
    let style = HighlightStyle {
        color: Some(theme.ui.panel_match),
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
pub(super) fn button(
    id: &'static str,
    label: &'static str,
    action: impl Into<Action>,
    theme: &Theme,
    cx: &mut Context<Sidebar>,
) -> AnyElement {
    let action: Action = action.into();
    div()
        .id(id)
        .px(theme.layout.panel_button_padding_x)
        .rounded(theme.radius.md)
        .text_color(theme.ui.panel_muted)
        .hover(|button| {
            button
                .bg(theme.ui.panel_hover)
                .text_color(theme.ui.panel_foreground)
        })
        .on_click(cx.listener(move |sidebar, _, window, cx| {
            sidebar.focus(window, cx);
            window.dispatch_action(Box::new(Dispatch(action.into())), cx);
        }))
        .child(label)
        .into_any_element()
}
