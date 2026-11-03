//! Lo que el panel le pregunta al usuario, arriba del árbol: un nombre,
//! el filtro o confirmar un borrado.

use gpui_kit::{
    Context, Div, InteractiveElement as _, ParentElement as _, Styled as _, div,
    prelude::FluentBuilder as _,
};
use metalmind_vault::VaultPath;

use super::Sidebar;
use super::render::button;
use crate::line_field::{FieldLook, LineField, field_box};
use crate::panel::{NewEntry, Prompt};
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
                .child(field_box(theme).child(LineField::element(
                    &self.field,
                    input,
                    FieldLook::default(),
                    theme,
                    cx,
                ))),
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
