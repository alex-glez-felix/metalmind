//! Acciones de la ventana que no son de vim. Los atajos, la paleta y `:`
//! solo las invocan por su nombre.

use gpui_kit::{Action, App, KeyBinding};
use metalmind_vim::{Command, Invocation};

/// Contexto de teclas de la vista de editor.
pub const EDITOR_CONTEXT: &str = "Editor";

/// Cualquier [`Command`] como acción de GPUI, con el nombre del comando.
/// Es un solo tipo para que la ventana lo atienda con un solo handler; no
/// se registra por nombre porque la paleta lista los comandos de
/// [`Command::ALL`].
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct RunCommand(pub Invocation);

impl From<Command> for RunCommand {
    fn from(command: Command) -> Self {
        Self(command.into())
    }
}

impl Action for RunCommand {
    fn boxed_clone(&self) -> Box<dyn Action> {
        Box::new(self.clone())
    }

    fn partial_eq(&self, action: &dyn Action) -> bool {
        action.as_any().downcast_ref::<Self>() == Some(self)
    }

    fn name(&self) -> &'static str {
        self.0.command.name()
    }

    fn name_for_type() -> &'static str {
        "workspace::run_command"
    }

    fn build(_: serde_json::Value) -> anyhow::Result<Box<dyn Action>> {
        Err(anyhow::anyhow!(
            "un comando se arma desde su `Command`, no por nombre"
        ))
    }
}

/// Copia la selección de Visual o del ratón, o la línea del cursor si no
/// hay ninguna.
#[derive(Clone, PartialEq, Eq, Default, Debug, Action)]
#[action(namespace = editor, name = "copy")]
pub struct Copy;

/// Pega en el cursor; en Visual, reemplaza la selección.
#[derive(Clone, PartialEq, Eq, Default, Debug, Action)]
#[action(namespace = editor, name = "paste")]
pub struct Paste;

/// `ctrl-shift-*` no lo usa vim, así que queda para el portapapeles del
/// sistema y para la paleta, con el atajo de Zed.
pub fn bind_keys(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("ctrl-shift-c", Copy, Some(EDITOR_CONTEXT)),
        KeyBinding::new("ctrl-shift-v", Paste, Some(EDITOR_CONTEXT)),
        KeyBinding::new(
            "ctrl-shift-p",
            RunCommand::from(Command::OpenCommandPalette),
            Some(EDITOR_CONTEXT),
        ),
    ]);
    crate::sidebar::bind_keys(cx);
}

#[cfg(test)]
mod tests {
    use gpui_kit::{
        Context, FocusHandle, InteractiveElement as _, IntoElement, ParentElement as _, Render,
        TestAppContext, VisualTestContext, Window, div,
    };

    use super::*;

    /// Atiende [`RunCommand`] como la raíz de la ventana y anota lo que le
    /// llega.
    struct Probe {
        received: Vec<Invocation>,
        focus: FocusHandle,
    }

    impl Render for Probe {
        fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
            div()
                .key_context(EDITOR_CONTEXT)
                .track_focus(&self.focus)
                .on_action(cx.listener(|probe, action: &RunCommand, _, _| {
                    probe.received.push(action.0.clone());
                }))
                .child("probe")
        }
    }

    fn probe(cx: &mut TestAppContext) -> (gpui_kit::Entity<Probe>, &mut VisualTestContext) {
        cx.update(bind_keys);
        let (probe, cx) = cx.add_window_view(|_, cx| Probe {
            received: Vec::new(),
            focus: cx.focus_handle(),
        });
        cx.update(|window, cx| window.focus(&probe.read(cx).focus.clone(), cx));
        (probe, cx)
    }

    #[test]
    fn cada_comando_llega_con_su_nombre_y_su_argumento() {
        let mut cx = TestAppContext::single();
        let (probe, cx) = probe(&mut cx);
        for &command in Command::ALL {
            let invocation = Invocation {
                command,
                argument: command.takes_argument().then(|| "notas/a.md".to_owned()),
            };
            let action = RunCommand(invocation.clone());
            assert_eq!(action.name(), command.name());
            cx.update(|window, cx| window.dispatch_action(Box::new(action), cx));
            let received = probe.update(cx, |probe, _| probe.received.pop());
            assert_eq!(received, Some(invocation));
        }
    }

    #[test]
    fn el_atajo_de_un_comando_llega_como_el_comando() {
        let mut cx = TestAppContext::single();
        let (probe, cx) = probe(&mut cx);
        cx.simulate_keystrokes("ctrl-shift-p");
        let received = probe.read_with(cx, |probe, _| probe.received.clone());
        assert_eq!(received, [Command::OpenCommandPalette.into()]);
    }

    /// La paleta lista los comandos de [`Command::ALL`], no los de GPUI.
    #[test]
    fn los_comandos_no_se_construyen_por_nombre() {
        let cx = TestAppContext::single();
        cx.update(|cx| {
            for &command in Command::ALL {
                assert!(
                    cx.build_action(command.name(), None).is_err(),
                    "{command:?}"
                );
            }
        });
    }
}
