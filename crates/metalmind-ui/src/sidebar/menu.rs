//! El menú del clic derecho sobre una fila. Cada entrada es una acción con
//! nombre: elegirla es lo mismo que su tecla en el árbol.

use gpui_kit::{
    AnyElement, Context, Div, InteractiveElement as _, IntoElement, MouseButton,
    ParentElement as _, Pixels, Point, Stateful, StatefulInteractiveElement as _, Styled as _,
    anchored, deferred, div, prelude::FluentBuilder as _,
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
    fn keys(self, keymap: &Keymap) -> Option<String> {
        keymap.keys_for(self.action().into(), |view| view == View::FileTree)
    }

    pub fn action(self) -> PanelAction {
        match self {
            Self::NewNote => PanelAction::NewNote,
            Self::NewFolder => PanelAction::NewFolder,
            Self::OpenSplitRight => PanelAction::OpenSplitRight,
            Self::OpenSplitDown => PanelAction::OpenSplitDown,
            Self::Rename => PanelAction::Rename,
            Self::Move => PanelAction::Move,
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
        let list = self
            .list(theme, cx, |item, row, cx| {
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

    /// Las entradas con sus separadores; `wire` le pone a cada fila lo que
    /// hace al elegirla.
    fn list<V: 'static>(
        &self,
        theme: &Theme,
        cx: &mut Context<V>,
        wire: impl Fn(MenuItem, Stateful<Div>, &mut Context<V>) -> Stateful<Div>,
    ) -> Div {
        let mut list = div()
            .min_w(theme.layout.menu_min_width)
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
            .gap(theme.layout.menu_gap)
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
                    .child(item.label()),
            )
            .child(
                div()
                    .debug_selector(|| format!("{item:?}-keys"))
                    .map(|element| theme.ui_small_text(element))
                    .text_color(theme.ui.panel_muted)
                    .children(keys),
            )
    }
}

#[cfg(test)]
mod tests {
    use gpui_kit::{Render, TestAppContext, Window};
    use metalmind_config::Config;

    use super::*;
    use crate::actions::bind_keys;

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

    /// Cada entrada muestra la tecla que hace lo mismo en el árbol.
    #[test]
    fn cada_entrada_muestra_la_tecla_de_su_accion() {
        let keymap = Keymap::default();
        let keys: Vec<Option<String>> =
            ContextMenu::new(Point::default(), path("a.md"), EntryKind::Note)
                .items()
                .map(|item| item.keys(&keymap))
                .collect();
        let expected =
            ["a", "A", "ctrl-v", "ctrl-s", "r", "m", "d"].map(|key| Some(key.to_owned()));
        assert_eq!(keys, expected);
    }

    struct MenuProbe(ContextMenu);

    impl Render for MenuProbe {
        fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
            let theme = Theme::with_font(&Config::defaults().unwrap(), "Mono".into());
            anchored().child(self.0.list(&theme, cx, |_, row, _| row))
        }
    }

    /// Entre la etiqueta y la tecla de cada entrada queda el hueco del tema,
    /// y el menú es tan ancho como su fila más larga: «Abrir a la derecha»
    /// con `ctrl-v`, que no cabe en el ancho mínimo.
    #[test]
    fn la_etiqueta_no_choca_con_su_tecla() {
        let mut cx = TestAppContext::single();
        cx.update(|cx| bind_keys(Keymap::default(), cx));
        let menu = menu_on("a.md");
        let items: Vec<MenuItem> = menu.items().collect();
        let (_, cx) = cx.add_window_view(|_, _| MenuProbe(menu));
        cx.run_until_parked();
        let theme = Theme::with_font(&Config::defaults().unwrap(), "Mono".into());
        let mut bounds = |item: MenuItem, part: &str| {
            let selector: &'static str = format!("{item:?}{part}").leak();
            cx.debug_bounds(selector).unwrap()
        };
        for &item in &items {
            let (label, keys) = (bounds(item, "-label"), bounds(item, "-keys"));
            assert!(
                keys.left() - label.right() >= theme.layout.menu_gap,
                "{item:?}: {label:?} y {keys:?}"
            );
        }
        let item = MenuItem::OpenSplitRight;
        let (label, keys) = (bounds(item, "-label"), bounds(item, "-keys"));
        let padding = theme.layout.menu_item_padding_x;
        let needed = padding + label.size.width + theme.layout.menu_gap + keys.size.width + padding;
        assert!(
            needed > theme.layout.menu_min_width,
            "no cabe en el ancho mínimo"
        );
        assert_eq!(bounds(item, "").size.width, needed);
    }
}
