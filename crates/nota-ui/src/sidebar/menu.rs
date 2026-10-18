//! El menú del clic derecho sobre una fila. Cada entrada es una acción con
//! nombre: elegirla es lo mismo que su tecla en el árbol.

use gpui_kit::{
    Action, AnyElement, Context, InteractiveElement as _, IntoElement, MouseButton,
    ParentElement as _, Pixels, Point, StatefulInteractiveElement as _, Styled as _, anchored,
    deferred, div, prelude::FluentBuilder as _,
};
use nota_vault::{EntryKind, VaultChange, VaultPath};

use super::Sidebar;
use super::actions::{Delete, Move, NewFolder, NewNote, OpenSplitDown, OpenSplitRight, Rename};
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
        }
    }

    /// La tecla que hace lo mismo en el árbol.
    fn key(self) -> &'static str {
        match self {
            Self::NewNote => "a",
            Self::NewFolder => "A",
            Self::OpenSplitRight => "ctrl-v",
            Self::OpenSplitDown => "ctrl-s",
            Self::Rename => "r",
            Self::Move => "m",
            Self::Delete => "d",
        }
    }

    pub fn action(self) -> Box<dyn Action> {
        match self {
            Self::NewNote => Box::new(NewNote),
            Self::NewFolder => Box::new(NewFolder),
            Self::OpenSplitRight => Box::new(OpenSplitRight),
            Self::OpenSplitDown => Box::new(OpenSplitDown),
            Self::Rename => Box::new(Rename),
            Self::Move => Box::new(Move),
            Self::Delete => Box::new(Delete),
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

pub struct ContextMenu {
    position: Point<Pixels>,
    /// La entrada sobre la que se abrió: las acciones del menú van sobre
    /// la selección del árbol, que tiene que seguir siendo ella.
    target: VaultPath,
    groups: Vec<&'static [MenuItem]>,
    selected: usize,
}

impl ContextMenu {
    pub fn new(position: Point<Pixels>, target: VaultPath, kind: EntryKind) -> Self {
        Self {
            position,
            target,
            groups: MenuItem::groups(kind),
            selected: 0,
        }
    }

    /// Tras un lote del watcher: sigue a su entrada en los renombres y dice
    /// si el menú todavía vale, es decir, si su entrada existe y sigue
    /// seleccionada. Si no, una acción caería sobre otra entrada.
    pub fn still_on(&mut self, changes: &[VaultChange], selected: Option<&VaultPath>) -> bool {
        for change in changes {
            match change {
                VaultChange::Renamed { from, to, .. } => {
                    if let Some(moved) = self.target.rebase(from, to) {
                        self.target = moved;
                    }
                }
                VaultChange::Removed { path, .. } if self.target.starts_with(path) => {
                    return false;
                }
                _ => {}
            }
        }
        selected == Some(&self.target)
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

    pub fn render(&self, theme: &Theme, cx: &mut Context<Sidebar>) -> AnyElement {
        let mut list = div()
            .w(theme.menu_width)
            .p(theme.menu_padding)
            .flex()
            .flex_col()
            .bg(theme.menu_background)
            .border(theme.border_width)
            .border_color(theme.menu_border)
            .rounded(theme.radius)
            .on_mouse_down_out(cx.listener(|sidebar, _, _, cx| {
                sidebar.menu = None;
                cx.notify();
            }));
        let mut index = 0;
        for (group_index, group) in self.groups.iter().enumerate() {
            if group_index > 0 {
                list = list.child(
                    div()
                        .my(theme.menu_padding)
                        .h(theme.border_width)
                        .bg(theme.menu_border),
                );
            }
            for &item in *group {
                list = list.child(self.item(item, index, theme, cx));
                index += 1;
            }
        }
        deferred(
            anchored()
                .position(self.position)
                .snap_to_window()
                .child(list),
        )
        .with_priority(1)
        .into_any_element()
    }

    fn item(
        &self,
        item: MenuItem,
        index: usize,
        theme: &Theme,
        cx: &mut Context<Sidebar>,
    ) -> AnyElement {
        let color = if item == MenuItem::Delete {
            theme.error
        } else {
            theme.panel_foreground
        };
        div()
            .id(("menu-item", index))
            .flex()
            .justify_between()
            .px(theme.panel_padding_x)
            .h(theme.panel_row_height)
            .items_center()
            .rounded(theme.radius)
            .text_color(color)
            .when(index == self.selected, |row| row.bg(theme.menu_hover))
            .hover(|row| row.bg(theme.menu_hover))
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .on_click(cx.listener(move |sidebar, _, window, cx| {
                sidebar.menu = None;
                window.dispatch_action(item.action(), cx);
                cx.notify();
            }))
            .child(item.label())
            .child(div().text_color(theme.panel_muted).child(item.key()))
            .into_any_element()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn path(path: &str) -> VaultPath {
        VaultPath::new(path).unwrap()
    }

    fn menu_on(target: &str) -> ContextMenu {
        ContextMenu::new(Point::default(), path(target), EntryKind::Note)
    }

    /// Si la entrada del menú se borra fuera de la app, la selección pasa a
    /// otra fila: el menú no puede seguir abierto sobre ella.
    #[test]
    fn el_menu_se_cierra_si_su_entrada_desaparece() {
        let mut menu = menu_on("notas/a.md");
        let removed = VaultChange::Removed {
            path: path("notas"),
            kind: EntryKind::Folder,
        };
        assert!(!menu.still_on(&[removed], Some(&path("b.md"))));
    }

    #[test]
    fn el_menu_sigue_a_su_entrada_renombrada() {
        let mut menu = menu_on("notas/a.md");
        let renamed = VaultChange::Renamed {
            from: path("notas"),
            to: path("archivo"),
            kind: EntryKind::Folder,
        };
        assert!(menu.still_on(&[renamed], Some(&path("archivo/a.md"))));
        assert!(
            !menu.still_on(&[], Some(&path("notas/a.md"))),
            "ya no es esa"
        );
    }

    #[test]
    fn el_menu_se_cierra_si_la_seleccion_ya_no_es_su_entrada() {
        let mut menu = menu_on("a.md");
        let other = VaultChange::Created {
            path: path("c.md"),
            kind: EntryKind::Note,
        };
        assert!(menu.still_on(std::slice::from_ref(&other), Some(&path("a.md"))));
        assert!(!menu.still_on(std::slice::from_ref(&other), Some(&path("b.md"))));
        assert!(!menu.still_on(&[other], None));
    }

    #[test]
    fn una_carpeta_no_ofrece_abrir_en_split() {
        let point = Point::default();
        let file: Vec<MenuItem> = ContextMenu::new(point, path("a.md"), EntryKind::Note)
            .items()
            .collect();
        assert!(file.contains(&MenuItem::OpenSplitRight));
        let folder: Vec<MenuItem> = ContextMenu::new(point, path("d"), EntryKind::Folder)
            .items()
            .collect();
        assert!(!folder.contains(&MenuItem::OpenSplitRight));
        assert_eq!(folder.last(), Some(&MenuItem::Delete));
    }

    #[test]
    fn el_teclado_no_sale_del_menu() {
        let mut menu = ContextMenu::new(Point::default(), path("d"), EntryKind::Folder);
        menu.select_previous();
        assert_eq!(menu.selected_item(), Some(MenuItem::NewNote));
        for _ in 0..10 {
            menu.select_next();
        }
        assert_eq!(menu.selected_item(), Some(MenuItem::Delete));
    }

    #[test]
    fn cada_entrada_invoca_la_accion_de_su_tecla() {
        let names: Vec<&str> = ContextMenu::new(Point::default(), path("a.md"), EntryKind::Note)
            .items()
            .map(|item| item.action().name())
            .collect();
        assert_eq!(
            names,
            [
                "file_tree::new_note",
                "file_tree::new_folder",
                "file_tree::open_split_right",
                "file_tree::open_split_down",
                "file_tree::rename",
                "file_tree::move",
                "file_tree::delete",
            ]
        );
    }
}
