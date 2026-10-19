//! Lo que el panel le pregunta al usuario, arriba del árbol: un nombre,
//! el filtro o confirmar un borrado.

use gpui_kit::{Context, Div, ParentElement as _, Styled as _, div, prelude::FluentBuilder as _};
use metalmind_vault::VaultPath;

use super::Sidebar;
use super::actions::{Cancel, Confirm};
use super::render::button;
use crate::panel::{LineInput, NewEntry, Prompt};
use crate::theme::Theme;

impl Sidebar {
    pub(super) fn prompt_view(&self, theme: &Theme, cx: &mut Context<Self>) -> Option<Div> {
        let prompt = self.panel.prompt()?;
        let frame = div()
            .flex()
            .flex_col()
            .flex_none()
            .gap(theme.panel_gap)
            .px(theme.panel_padding_x)
            .py(theme.panel_padding_y)
            .border_b(theme.border_width)
            .border_color(theme.panel_border);
        let (label, input) = match prompt {
            Prompt::Filter(input) => ("Filtrar".to_owned(), input),
            Prompt::Create {
                parent,
                kind,
                input,
            } => {
                let what = match kind {
                    NewEntry::Note => "Nueva nota",
                    NewEntry::Folder => "Nueva carpeta",
                };
                (format!("{what} en {}", folder_name(parent)), input)
            }
            Prompt::Rename { path, input } => (format!("Renombrar {}", path.name()), input),
            Prompt::Move { path, input } => (format!("Mover {path} a"), input),
            Prompt::ConfirmDelete { path, unsaved } => {
                return Some(frame.child(confirm_delete(path, *unsaved, theme, cx)));
            }
        };
        Some(
            frame
                .child(div().text_color(theme.panel_muted).child(label))
                .child(input_field(input, theme)),
        )
    }
}

fn folder_name(path: &VaultPath) -> String {
    if path.is_root() {
        "la raíz".to_owned()
    } else {
        format!("{path}/")
    }
}

/// El texto con el cursor de barra, como la línea de comandos.
fn input_field(input: &LineInput, theme: &Theme) -> Div {
    let (before, after) = input.text().split_at(input.cursor());
    div()
        .flex()
        .items_center()
        .h(theme.panel_row_height)
        .px(theme.panel_padding_x)
        .bg(theme.background)
        .border(theme.border_width)
        .border_color(theme.panel_border)
        .rounded(theme.radius_md)
        .whitespace_nowrap()
        .overflow_hidden()
        .child(before.to_owned())
        .child(
            div()
                .w(theme.caret_width)
                .h(theme.ui_font_size)
                .flex_none()
                .bg(theme.caret),
        )
        .child(after.to_owned())
}

fn confirm_delete(
    path: &VaultPath,
    unsaved: bool,
    theme: &Theme,
    cx: &mut Context<Sidebar>,
) -> Div {
    div()
        .flex()
        .flex_col()
        .gap(theme.panel_gap)
        .child(format!("¿Mandar «{path}» a la papelera del vault?"))
        .when(unsaved, |question| {
            question.child(
                div()
                    .text_color(theme.error)
                    .child("Tiene cambios sin guardar: se pierden."),
            )
        })
        .child(
            div()
                .flex()
                .gap(theme.panel_gap)
                .child(button("confirm-delete", "Borrar (y)", Confirm, theme, cx))
                .child(button("cancel-delete", "Cancelar (n)", Cancel, theme, cx)),
        )
}
