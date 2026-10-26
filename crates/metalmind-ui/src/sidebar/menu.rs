//! El menú del clic derecho sobre una fila. Cada entrada es una acción con
//! nombre: elegirla es lo mismo que su tecla en el árbol.

use gpui_kit::{
    AnyElement, Context, Div, InteractiveElement as _, IntoElement, MouseButton,
    ParentElement as _, Pixels, Point, Stateful, StatefulInteractiveElement as _, Styled as _,
    Window, anchored, deferred, div, prelude::FluentBuilder as _,
};
use metalmind_vault::{EntryKind, VaultChange, VaultPath};

use metalmind_keymap::{Keymap, PanelAction, View};

use super::Sidebar;
use crate::actions::{AppKeymap, Dispatch};
use crate::theme::Theme;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MenuItem {
    NewNote,
    NewFolder,
    OpenSplitRight,
    OpenSplitDown,
    Rename,
    Move,
    Delete,
    /// En la vista de tags, lo que `Move` es en la de carpetas.
    MergeTag,
    /// En la vista de tags, lo que `NewNote` es en la de carpetas.
    NewTaggedNote,
}

impl MenuItem {
    fn label(self) -> &'static str {
        match self {
            Self::NewNote => "Nueva nota",
            Self::NewFolder => "Nueva carpeta",
            Self::OpenSplitRight => "Abrir a la derecha",
            Self::OpenSplitDown => "Abrir abajo",
            Self::Rename => "Renombrar",
            Self::Move => "Mover",
            Self::Delete => "Mandar a la papelera",
            Self::MergeTag => "Fusionar con…",
            Self::NewTaggedNote => "Nueva nota con este tag",
        }
    }

    /// La tecla que hace lo mismo en el árbol.
    fn keys(self, keymap: &Keymap) -> Option<String> {
        keymap.keys_for(self.action().into(), |view| view == View::FileTree)
    }

    pub fn action(self) -> PanelAction {
        match self {
            Self::NewNote | Self::NewTaggedNote => PanelAction::NewNote,
            Self::NewFolder => PanelAction::NewFolder,
            Self::OpenSplitRight => PanelAction::OpenSplitRight,
            Self::OpenSplitDown => PanelAction::OpenSplitDown,
            Self::Rename => PanelAction::Rename,
            Self::Move | Self::MergeTag => PanelAction::Move,
            Self::Delete => PanelAction::Delete,
        }
    }

    /// Las entradas en grupos, separados por una línea. Una carpeta no se
    /// abre en un split.
    fn groups(kind: EntryKind) -> Vec<&'static [Self]> {
        let mut groups: Vec<&'static [Self]> = vec![&[Self::NewNote, Self::NewFolder]];
        if kind != EntryKind::Folder {
            groups.push(&[Self::OpenSplitRight, Self::OpenSplitDown]);
        }
        groups.extend([&[Self::Rename, Self::Move][..], &[Self::Delete][..]]);
        groups
    }
}

/// Sobre qué se abrió el menú: sus acciones van sobre la selección del
/// panel, que tiene que seguir siendo esto.
#[derive(Clone, Debug, PartialEq, Eq)]
enum MenuTarget {
    Entry(VaultPath),
    /// Un tag de la vista de tags, por su forma establecida.
    Tag(String),
}

pub struct ContextMenu {
    position: Point<Pixels>,
    target: MenuTarget,
    groups: Vec<&'static [MenuItem]>,
    selected: usize,
}

impl ContextMenu {
    pub fn new(position: Point<Pixels>, target: VaultPath, kind: EntryKind) -> Self {
        Self {
            position,
            target: MenuTarget::Entry(target),
            groups: MenuItem::groups(kind),
            selected: 0,
        }
    }

    /// Sobre una nota de la vista de tags: solo abrirla en un split.
    pub fn on_note(position: Point<Pixels>, target: VaultPath) -> Self {
        Self {
            position,
            target: MenuTarget::Entry(target),
            groups: vec![&[MenuItem::OpenSplitRight, MenuItem::OpenSplitDown]],
            selected: 0,
        }
    }

    /// Sobre un tag de la vista de tags: una nota nueva con él, o
    /// renombrarlo o fusionarlo (05 §5).
    pub fn on_tag(position: Point<Pixels>, tag: String) -> Self {
        Self {
            position,
            target: MenuTarget::Tag(tag),
            groups: vec![
                &[MenuItem::NewTaggedNote],
                &[MenuItem::Rename, MenuItem::MergeTag],
            ],
            selected: 0,
        }
    }

    /// Tras un lote del watcher: sigue a su entrada en los renombres y dice
    /// si el menú todavía vale, es decir, si su entrada o su tag existe y
    /// sigue elegido. Si no, una acción caería sobre otra fila.
    pub fn still_on(
        &mut self,
        changes: &[VaultChange],
        selected: Option<&VaultPath>,
        selected_tag: Option<&str>,
    ) -> bool {
        let target = match &mut self.target {
            MenuTarget::Tag(tag) => return selected_tag == Some(tag.as_str()),
            MenuTarget::Entry(target) => target,
        };
        for change in changes {
            match change {
                VaultChange::Renamed { from, to, .. } => {
                    if let Some(moved) = target.rebase(from, to) {
                        *target = moved;
                    }
                }
                VaultChange::Removed { path, .. } if target.starts_with(path) => {
                    return false;
                }
                _ => {}
            }
        }
        selected == Some(&*target)
    }

    fn items(&self) -> impl Iterator<Item = MenuItem> + '_ {
        self.groups.iter().flat_map(|group| group.iter().copied())
    }

    pub fn selected_item(&self) -> Option<MenuItem> {
        self.items().nth(self.selected)
    }

    pub fn select_next(&mut self) {
        self.selected = (self.selected + 1).min(self.items().count().saturating_sub(1));
    }

    pub fn select_previous(&mut self) {
        self.selected = self.selected.saturating_sub(1);
    }

    pub fn render(&self, theme: &Theme, window: &Window, cx: &mut Context<Sidebar>) -> AnyElement {
        let list = self
            .list(theme, window.viewport_size().width, cx, |item, row, cx| {
                row.on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .on_click(cx.listener(move |sidebar, _, window, cx| {
                        sidebar.menu = None;
                        window.dispatch_action(Box::new(Dispatch(item.action().into())), cx);
                        cx.notify();
                    }))
            })
            .on_mouse_down_out(cx.listener(|sidebar, _, _, cx| {
                sidebar.menu = None;
                cx.notify();
            }));
        deferred(
            anchored()
                .position(self.position)
                .snap_to_window()
                .child(list),
        )
        .with_priority(1)
        .into_any_element()
    }

    /// Las entradas con sus separadores, en no más de `max_width`; `wire` le
    /// pone a cada fila lo que hace al elegirla.
    fn list<V: 'static>(
        &self,
        theme: &Theme,
        max_width: Pixels,
        cx: &mut Context<V>,
        wire: impl Fn(MenuItem, Stateful<Div>, &mut Context<V>) -> Stateful<Div>,
    ) -> Div {
        let mut list = div()
            .min_w(theme.layout.menu_width.min(max_width))
            .max_w(max_width)
            .p(theme.layout.menu_padding)
            .flex()
            .flex_col()
            .bg(theme.ui.menu_background)
            .border(theme.stroke.border)
            .border_color(theme.ui.menu_border)
            .rounded(theme.radius.md);
        let mut index = 0;
        for (group_index, group) in self.groups.iter().enumerate() {
            if group_index > 0 {
                list = list.child(
                    div()
                        .my(theme.layout.menu_padding)
                        .h(theme.stroke.border)
                        .bg(theme.ui.menu_border),
                );
            }
            for &item in *group {
                let keys = item.keys(AppKeymap::get(cx));
                let row = self.item(item, keys, index, theme);
                list = list.child(wire(item, row, cx));
                index += 1;
            }
        }
        list
    }

    fn item(
        &self,
        item: MenuItem,
        keys: Option<String>,
        index: usize,
        theme: &Theme,
    ) -> Stateful<Div> {
        let color = if item == MenuItem::Delete {
            theme.ui.error
        } else {
            theme.ui.panel_foreground
        };
        div()
            .id(("menu-item", index))
            .debug_selector(|| format!("{item:?}"))
            .flex()
            .justify_between()
            .gap(theme.layout.menu_item_padding_x)
            .px(theme.layout.menu_item_padding_x)
            .h(theme.menu_row_height)
            .items_center()
            .rounded(theme.radius.md)
            .text_color(color)
            .when(index == self.selected, |row| row.bg(theme.ui.menu_hover))
            .hover(|row| row.bg(theme.ui.menu_hover))
            .child(
                div()
                    .debug_selector(|| format!("{item:?}-label"))
                    .min_w_0()
                    .truncate()
                    .child(item.label()),
            )
            .child(
                div()
                    .debug_selector(|| format!("{item:?}-keys"))
                    .flex_none()
                    .map(|element| theme.ui_small_text(element))
                    .text_color(theme.ui.panel_muted)
                    .children(keys),
            )
    }
}

#[cfg(test)]
#[path = "menu_tests.rs"]
mod tests;
