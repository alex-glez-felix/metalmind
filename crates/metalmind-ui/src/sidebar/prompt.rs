//! Lo que el panel le pregunta al usuario, arriba del árbol: un nombre,
//! el filtro o confirmar un borrado.

use gpui_kit::{
    Context, Div, InteractiveElement as _, ParentElement as _, Styled as _, div,
    prelude::FluentBuilder as _,
};
use metalmind_vault::VaultPath;

use super::Sidebar;
use super::render::button;
use crate::panel::{NewEntry, Prompt};
use crate::picker::InputLine;
use crate::theme::Theme;
use metalmind_keymap::PanelAction;

impl Sidebar {
    pub(super) fn prompt_view(&self, theme: &Theme, cx: &mut Context<Self>) -> Option<Div> {
        let prompt = self.panel.prompt()?;
        let frame = div()
            .flex()
            .flex_col()
            .flex_none()
            .gap(theme.layout.panel_gap)
            .px(theme.layout.panel_padding_x)
            .py(theme.layout.panel_padding_y)
            .border_b(theme.stroke.border)
            .border_color(theme.ui.panel_border);
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
            Prompt::RenameTag { tag, input } => (format!("Renombrar #{tag}"), input),
            Prompt::MergeTag { tag, input } => (format!("Fusionar #{tag} con"), input),
            Prompt::TagField { tag, field, input } => {
                let label = format!("{} de #{tag}: {}", field.label(), field.hint());
                (label, input)
            }
            Prompt::ConfirmDelete { path, unsaved } => {
                return Some(frame.child(confirm_delete(path, *unsaved, theme, cx)));
            }
        };
        Some(
            frame
                .child(div().text_color(theme.ui.panel_muted).child(label))
                .child(input_field(input, theme)),
        )
    }
}

fn folder_name(path: &VaultPath) -> String {
    if path.is_root() {
        "la raíz del vault".to_owned()
    } else {
        format!("{path}/")
    }
}

/// El texto con el cursor de barra. Si no cabe, se recorta como en la
/// línea de comandos: primero lo que sigue al cursor y luego el principio de
/// lo que lo antecede, así el cursor siempre se ve.
fn input_field(input: &InputLine, theme: &Theme) -> Div {
    let (before, after) = input.text().split_at(input.cursor());
    let up_to_caret = div()
        .flex()
        .items_center()
        .min_w_0()
        .child(
            clipped()
                .debug_selector(|| "panel-input-before".to_owned())
                .text_ellipsis_start()
                .child(before.to_owned()),
        )
        .child(
            div()
                .debug_selector(|| "panel-caret".to_owned())
                .w(theme.stroke.caret)
                .h(theme.ui_font_size)
                .flex_none()
                .bg(theme.cursor.insert),
        );
    div()
        .debug_selector(|| "panel-input".to_owned())
        .flex()
        .items_center()
        .h(theme.panel_row_height)
        .px(theme.layout.panel_padding_x)
        .bg(theme.ui.background)
        .border(theme.stroke.border)
        .border_color(theme.ui.panel_border)
        .rounded(theme.radius.md)
        .overflow_hidden()
        .child(up_to_caret)
        // Sin ancho propio: solo ocupa lo que deja lo anterior al cursor.
        .child(
            clipped()
                .debug_selector(|| "panel-input-after".to_owned())
                .flex_1()
                .text_ellipsis()
                .child(after.to_owned()),
        )
}

fn clipped() -> Div {
    div().min_w_0().overflow_hidden().whitespace_nowrap()
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
        .gap(theme.layout.panel_gap)
        .child(format!("¿Mandar «{path}» a la papelera del vault?"))
        .when(unsaved, |question| {
            question.child(
                div()
                    .text_color(theme.ui.error)
                    .child("Tiene cambios sin guardar: se pierden."),
            )
        })
        .child(
            div()
                .flex()
                .gap(theme.layout.panel_gap)
                .child(button(
                    "confirm-delete",
                    "Borrar (y)",
                    PanelAction::Confirm,
                    theme,
                    cx,
                ))
                .child(button(
                    "cancel-delete",
                    "Cancelar (n)",
                    PanelAction::Cancel,
                    theme,
                    cx,
                )),
        )
}

#[cfg(test)]
#[path = "prompt_tests.rs"]
mod tests;
