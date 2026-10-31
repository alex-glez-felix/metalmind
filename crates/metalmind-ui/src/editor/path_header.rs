//! La ruta de la nota sobre cada panel, en lugar de pestañas (#164, paso
//! 2): cada carpeta es una miga que la muestra en la lista de notas, como
//! `space c`, y el nombre va al final. Un clic en la ruta enfoca el panel;
//! en el enfocado, si hay varios, la línea de abajo lleva el acento.

use gpui_kit::{
    Context, Div, InteractiveElement as _, MouseButton, ParentElement as _, SharedString,
    StatefulInteractiveElement as _, Styled as _, div, prelude::FluentBuilder as _, relative,
};
use metalmind_keymap::{Command, Invocation, View};
use metalmind_vault::VaultPath;

use super::Editor;
use crate::actions::{AppKeymap, Dispatch};
use crate::theme::Theme;
use crate::tooltip::Tooltip;
use crate::workspace::PaneId;

/// Lo que dice la ruta de un panel.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PanePath {
    /// Las carpetas de la nota en el vault, de la raíz hacia adentro, sin
    /// la raíz.
    pub folders: Vec<VaultPath>,
    pub name: String,
}

impl PanePath {
    /// La ruta de `note` si es del vault; si no, solo `display_name`, sin
    /// carpetas que abrir en la lista.
    #[must_use]
    pub fn new(note: Option<&VaultPath>, display_name: &str) -> Self {
        let Some(note) = note else {
            return Self {
                folders: Vec::new(),
                name: display_name.to_owned(),
            };
        };
        let mut folders: Vec<VaultPath> = note
            .ancestors()
            .filter(|folder| !folder.is_root())
            .collect();
        folders.reverse();
        Self {
            folders,
            name: note.name().to_owned(),
        }
    }
}

impl Editor {
    pub(super) fn path_header(&mut self, theme: &Theme, cx: &mut Context<Self>) -> Div {
        let note = self.note_path(cx);
        let (path, focused) = match self.view(cx) {
            Some(view) => (
                PanePath::new(note.as_ref(), &view.document.display_name()),
                view.focused,
            ),
            None => (PanePath::new(None, ""), false),
        };
        let split = self.workspace.read(cx).tree().len() > 1;
        let key =
            AppKeymap::get(cx).keys_for(Command::ShowFolder.into(), |view| view == View::Editor);
        let full: SharedString = note
            .map_or_else(|| path.name.clone(), |note| note.to_string())
            .into();
        let line = PathLine {
            pane: self.pane,
            path,
            full,
            focused,
            key,
        };
        div()
            .flex_none()
            .min_w_0()
            .px(theme.layout.pane_header_padding_x)
            .py(theme.layout.pane_header_padding_y)
            .border_b(theme.stroke.border)
            .border_color(if focused && split {
                theme.ui.pane_header_focus
            } else {
                theme.ui.pane_header_border
            })
            .map(|header| theme.ui_small_text(header))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|editor, _, window, cx| editor.take_focus(window, cx)),
            )
            .child(line.render(theme))
    }
}

/// Las migas y el nombre, en una línea.
struct PathLine {
    pane: PaneId,
    path: PanePath,
    /// La ruta entera, para el tooltip del nombre.
    full: SharedString,
    focused: bool,
    /// La tecla de `list::show_folder`, en el tooltip de la carpeta de la nota.
    key: Option<String>,
}

impl PathLine {
    /// Si no cabe, la carpeta de la nota no se encoge, para el clic, hasta
    /// `pane_header_folder_max_share` de la línea; lo demás se recorta en
    /// proporción a lo que mide: las otras carpetas por la izquierda y el
    /// nombre por la derecha.
    fn render(self, theme: &Theme) -> Div {
        let (name_color, folder_color) = if self.focused {
            (theme.ui.pane_header_foreground, theme.ui.pane_header_folder)
        } else {
            (
                theme.ui.pane_header_unfocused,
                theme.ui.pane_header_unfocused,
            )
        };
        let nearest = self.path.folders.len().checked_sub(1);
        let mut crumbs: Vec<_> = self
            .path
            .folders
            .iter()
            .enumerate()
            .map(|(at, folder)| {
                let hint = self.key.as_deref().filter(|_| Some(at) == nearest);
                crumb(self.pane, at, folder, hint, folder_color, theme)
            })
            .collect();
        let nearest = crumbs
            .pop()
            .map(|nearest| nearest.max_w(relative(theme.layout.pane_header_folder_max_share)));
        let full = self.full;
        div()
            .flex()
            .min_w_0()
            .items_center()
            .child(
                div()
                    .flex()
                    .min_w_0()
                    .overflow_hidden()
                    .justify_end()
                    .children(crumbs),
            )
            .children(nearest)
            .child(
                div()
                    .id(SharedString::from(format!("pane-path-name-{}", self.pane)))
                    .debug_selector(|| "pane-path-name".to_owned())
                    .min_w_0()
                    .truncate()
                    .text_color(name_color)
                    .font_weight(if self.focused {
                        theme.text.strong
                    } else {
                        theme.text.normal
                    })
                    .tooltip(move |_, cx| Tooltip::view(full.clone(), cx))
                    .child(self.path.name),
            )
    }
}

/// Una carpeta y su `/`: un clic la muestra en la lista.
fn crumb(
    pane: PaneId,
    at: usize,
    folder: &VaultPath,
    key: Option<&str>,
    color: gpui_kit::Hsla,
    theme: &Theme,
) -> Div {
    let tip: SharedString = match key {
        Some(key) => format!("{folder}/ en la lista · {key}"),
        None => format!("{folder}/ en la lista"),
    }
    .into();
    let invocation = Invocation {
        action: Command::ShowFolder.into(),
        argument: Some(folder.as_str().to_owned()),
    };
    div()
        .flex()
        .flex_shrink_0()
        .min_w_0()
        .text_color(color)
        .child(
            div()
                .id(SharedString::from(format!("pane-path-{pane}-{at}")))
                .debug_selector(move || format!("pane-path-crumb-{at}"))
                .min_w_0()
                .truncate()
                .rounded(theme.radius.sm)
                .cursor_pointer()
                .hover(|crumb| crumb.bg(theme.ui.pane_header_hover))
                .on_click(move |_, window, cx| {
                    window.dispatch_action(Box::new(Dispatch(invocation.clone())), cx);
                })
                .tooltip(move |_, cx| Tooltip::view(tip.clone(), cx))
                .child(folder.name().to_owned()),
        )
        .child(div().flex_none().child("/"))
}

#[cfg(test)]
#[path = "path_header_tests.rs"]
mod layout_tests;

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;

    use super::*;

    fn folders(path: &PanePath) -> Vec<&str> {
        path.folders.iter().map(VaultPath::as_str).collect()
    }

    #[test]
    fn cada_carpeta_de_la_nota_es_una_miga_de_la_raiz_hacia_adentro() {
        let note = VaultPath::new("proyectos/metalmind/gpui.md").unwrap();
        let path = PanePath::new(Some(&note), "gpui.md");
        assert_eq!(folders(&path), ["proyectos", "proyectos/metalmind"]);
        assert_eq!(path.name, "gpui.md");
    }

    #[test]
    fn una_nota_en_la_raiz_no_tiene_carpetas() {
        let note = VaultPath::new("idea.md").unwrap();
        assert_eq!(
            PanePath::new(Some(&note), "idea.md").folders,
            Vec::<VaultPath>::new()
        );
    }

    #[test]
    fn fuera_del_vault_solo_el_nombre() {
        let path = PanePath::new(None, "[No Name]");
        assert_eq!(path.folders, Vec::<VaultPath>::new());
        assert_eq!(path.name, "[No Name]");
    }
}
