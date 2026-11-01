//! Las opciones que se cambian desde la paleta quedan en `config.yaml`
//! (#204): la ventana las guarda y dice cómo quedaron.

use gpui_kit::Context;
use metalmind_config::Toggle;

use super::WorkspaceView;
use crate::config_file::{self, SettingsError};
use crate::workspace::Message;

impl WorkspaceView {
    pub(super) fn save_toggle(&mut self, toggle: Toggle, on: bool, cx: &mut Context<Self>) {
        let saved = self
            .paths
            .as_ref()
            .map(|paths| config_file::save_toggle(paths, toggle, on));
        self.tell(toggle_message(toggle, on, saved.as_ref()), cx);
    }
}

/// Cómo quedó `toggle` y si se guardó; `None` si no hay `config.yaml`
/// donde guardarlo.
fn toggle_message(toggle: Toggle, on: bool, saved: Option<&Result<(), SettingsError>>) -> Message {
    let state = format!(
        "{}: {}",
        label(toggle),
        if on { "encendido" } else { "apagado" }
    );
    let [section, key] = toggle.path();
    match saved {
        None => Message::Info(state),
        Some(Ok(())) => Message::Info(format!("{state}; guardado en config.yaml")),
        Some(Err(error)) => Message::Error(format!(
            "{state} en esta ventana; no se guardó {section}.{key} en config.yaml: {error}"
        )),
    }
}

fn label(toggle: Toggle) -> &'static str {
    match toggle {
        Toggle::Wrap => "Partir las líneas largas",
        Toggle::ContinueLists => "Continuar las listas",
        Toggle::IndentListChildren => "Mover los hijos de un ítem con >> y <<",
        Toggle::AlignTablesOnLeave => "Alinear la tabla al salir de Insert",
        Toggle::WhichKey => "Mostrar las teclas que siguen a un prefijo",
    }
}

#[cfg(test)]
mod tests {
    use metalmind_config::edit::EditError;
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn dice_como_quedo_y_si_se_guardo() {
        assert_eq!(
            toggle_message(Toggle::ContinueLists, false, Some(&Ok(()))),
            Message::Info("Continuar las listas: apagado; guardado en config.yaml".to_owned())
        );
        assert_eq!(
            toggle_message(Toggle::Wrap, true, None),
            Message::Info("Partir las líneas largas: encendido".to_owned())
        );
        let error = SettingsError::Edit(EditError::Unsupported("markdown".to_owned()));
        assert_eq!(
            toggle_message(Toggle::AlignTablesOnLeave, true, Some(&Err(error))),
            Message::Error(
                "Alinear la tabla al salir de Insert: encendido en esta ventana; no se guardó \
                 markdown.align_tables_on_leave en config.yaml: `markdown` está escrito de una \
                 forma que Metalmind no edita; cámbialo a mano"
                    .to_owned()
            )
        );
    }
}
