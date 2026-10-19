//! Acciones de la ventana que no son de vim. Los atajos, la paleta y `:`
//! solo las invocan por su nombre.

use gpui_kit::{Action, App, Global, KeyBinding, Window};
use metalmind_keymap::{Command, Invocation, Keymap, UnknownAction};

/// Contexto de teclas de la vista de editor.
pub const EDITOR_CONTEXT: &str = "Editor";

/// El keymap de la app, del que salen la tabla del motor y las teclas que
/// muestra la paleta.
pub struct AppKeymap(Keymap);

impl Global for AppKeymap {}

impl AppKeymap {
    #[must_use]
    pub fn get(cx: &App) -> &Keymap {
        &cx.global::<Self>().0
    }
}

/// Una acción del registro como acción de GPUI, que solo la transporta por
/// el foco hasta la vista que la atiende. Se llama como la acción; no se
/// registra por nombre porque los nombres son del registro.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Dispatch(pub Invocation);

impl Action for Dispatch {
    fn boxed_clone(&self) -> Box<dyn Action> {
        Box::new(self.clone())
    }

    fn partial_eq(&self, action: &dyn Action) -> bool {
        action.as_any().downcast_ref::<Self>() == Some(self)
    }

    fn name(&self) -> &'static str {
        self.0.action.name()
    }

    fn name_for_type() -> &'static str {
        "metalmind::dispatch"
    }

    fn build(_: serde_json::Value) -> anyhow::Result<Box<dyn Action>> {
        Err(anyhow::anyhow!(
            "una acción se arma con `actions::dispatch`, desde el registro"
        ))
    }
}

/// Invoca la acción que se llama `name` desde el elemento enfocado de
/// `window`: el mismo camino para `:`, la paleta y los atajos.
///
/// # Errors
/// [`UnknownAction`] si ninguna acción se llama así.
pub fn dispatch(
    name: &str,
    argument: Option<String>,
    window: &mut Window,
    cx: &mut App,
) -> Result<(), UnknownAction> {
    let invocation = Invocation::resolve(name, argument)?;
    window.dispatch_action(Box::new(Dispatch(invocation)), cx);
    Ok(())
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
/// sistema y para la paleta, con el atajo de Zed. Deja `keymap` como el de
/// la app.
pub fn bind_keys(keymap: Keymap, cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("ctrl-shift-c", Copy, Some(EDITOR_CONTEXT)),
        KeyBinding::new("ctrl-shift-v", Paste, Some(EDITOR_CONTEXT)),
        KeyBinding::new(
            "ctrl-shift-p",
            Dispatch(Command::OpenCommandPalette.into()),
            Some(EDITOR_CONTEXT),
        ),
    ]);
    crate::sidebar::bind_keys(cx);
    cx.set_global(AppKeymap(keymap));
}

#[cfg(test)]
mod tests {
    use gpui_kit::{
        Context, FocusHandle, InteractiveElement as _, IntoElement, ParentElement as _, Render,
        TestAppContext, VisualTestContext, div,
    };
    use metalmind_keymap::Action as AnyAction;

    use super::*;

    /// Atiende [`Dispatch`] como la raíz de la ventana y anota lo que le
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
                .on_action(cx.listener(|probe, action: &Dispatch, _, _| {
                    probe.received.push(action.0.clone());
                }))
                .child("probe")
        }
    }

    fn probe(cx: &mut TestAppContext) -> (gpui_kit::Entity<Probe>, &mut VisualTestContext) {
        cx.update(|cx| bind_keys(Keymap::default(), cx));
        let (probe, cx) = cx.add_window_view(|_, cx| Probe {
            received: Vec::new(),
            focus: cx.focus_handle(),
        });
        cx.update(|window, cx| window.focus(&probe.read(cx).focus.clone(), cx));
        (probe, cx)
    }

    #[test]
    fn cada_accion_llega_por_su_nombre_con_su_argumento() {
        let mut cx = TestAppContext::single();
        let (probe, cx) = probe(&mut cx);
        for action in AnyAction::all() {
            let argument = Some("notas/a.md".to_owned());
            cx.update(|window, cx| dispatch(action.name(), argument.clone(), window, cx))
                .unwrap();
            let received = probe.update(cx, |probe, _| probe.received.pop());
            assert_eq!(received, Some(Invocation { action, argument }));
        }
    }

    #[test]
    fn un_nombre_desconocido_no_llega() {
        let mut cx = TestAppContext::single();
        let (probe, cx) = probe(&mut cx);
        let sent = cx.update(|window, cx| dispatch("editor::nada", None, window, cx));
        assert_eq!(sent, Err(UnknownAction("editor::nada".to_owned())));
        assert!(probe.read_with(cx, |probe, _| probe.received.is_empty()));
    }

    #[test]
    fn el_atajo_de_un_comando_llega_como_el_comando() {
        let mut cx = TestAppContext::single();
        let (probe, cx) = probe(&mut cx);
        cx.simulate_keystrokes("ctrl-shift-p");
        let received = probe.read_with(cx, |probe, _| probe.received.clone());
        assert_eq!(received, [Command::OpenCommandPalette.into()]);
    }

    /// Los nombres son del registro: GPUI no construye ninguno.
    #[test]
    fn gpui_no_construye_acciones_del_registro() {
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
