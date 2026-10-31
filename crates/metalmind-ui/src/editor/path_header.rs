//! La ruta de la nota sobre cada panel, en lugar de pestañas (#164, paso
//! 2): cada carpeta es una miga que la muestra en la lista de notas, como
//! `space c`, y el nombre va al final. Un clic en la ruta enfoca el panel;
//! en el enfocado, si hay varios, la línea de abajo lleva el acento.

use gpui_kit::{
    Context, Div, InteractiveElement as _, IntoElement, MouseButton, ParentElement as _,
    SharedString, StatefulInteractiveElement as _, Styled as _, div, prelude::FluentBuilder as _,
    relative,
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
        let (name_color, folder_color) = if focused {
            (theme.ui.pane_header_foreground, theme.ui.pane_header_folder)
        } else {
            (
                theme.ui.pane_header_unfocused,
                theme.ui.pane_header_unfocused,
            )
        };
        let nearest = path.folders.len().checked_sub(1);
        let crumbs = path.folders.iter().enumerate().map(|(at, folder)| {
            let hint = key.as_deref().filter(|_| Some(at) == nearest);
            crumb(self.pane, at, folder, hint, folder_color, theme)
        });
        let full: SharedString = note
            .map_or_else(|| path.name.clone(), |note| note.to_string())
            .into();
        div()
            .flex()
            .flex_none()
            .min_w_0()
            .items_center()
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
            .child(
                // Lo que no cabe se pierde por la izquierda: quedan a la vista
                // las carpetas más cercanas a la nota.
                div()
                    .flex()
                    .min_w_0()
                    .overflow_hidden()
                    .justify_end()
                    .children(crumbs),
            )
            .child(
                div()
                    .id(SharedString::from(format!("pane-path-name-{}", self.pane)))
                    .flex_shrink_0()
                    .max_w(relative(1.))
                    .truncate()
                    .text_color(name_color)
                    .font_weight(if focused {
                        theme.text.strong
                    } else {
                        theme.text.normal
                    })
                    .tooltip(move |_, cx| Tooltip::view(full.clone(), cx))
                    .child(path.name),
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
) -> impl IntoElement {
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
        .text_color(color)
        .child(
            div()
                .id(SharedString::from(format!("pane-path-{pane}-{at}")))
                .rounded(theme.radius.sm)
                .cursor_pointer()
                .hover(|crumb| crumb.bg(theme.ui.pane_header_hover))
                .on_click(move |_, window, cx| {
                    window.dispatch_action(Box::new(Dispatch(invocation.clone())), cx);
                })
                .tooltip(move |_, cx| Tooltip::view(tip.clone(), cx))
                .child(folder.name().to_owned()),
        )
        .child("/")
}

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
