//! GPUI como transporte del registro de acciones: los atajos de la ventana
//! salen del keymap de la app y todas las acciones viajan por el foco como
//! [`Dispatch`]. Los atajos, la paleta y `:` solo las invocan por su nombre.

use gpui_kit::{Action, App, Global, KeyBinding, Window};
use metalmind_keymap::{Invocation, KeyName, Keymap, Keystroke, UnknownAction, context};

/// Contexto de teclas de la vista de editor.
pub const EDITOR_CONTEXT: &str = context::EDITOR;

/// El keymap de la app, del que salen los atajos, la paleta y las teclas que
/// muestran los menús.
pub struct AppKeymap(Keymap);

impl Global for AppKeymap {}

impl AppKeymap {
    #[must_use]
    pub fn get(cx: &App) -> &Keymap {
        &cx.global::<Self>().0
    }
}

/// Cualquier acción del registro como acción de GPUI, que solo la
/// transporta por el foco hasta la vista que la atiende. Se llama como la
/// acción; no se registra por nombre porque los nombres son del registro.
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

/// Le da a GPUI los atajos de las vistas de `keymap` y lo deja como el de
/// la app. Los del editor que son de vim los resuelve el motor.
pub fn bind_keys(keymap: Keymap, cx: &mut App) {
    let bindings: Vec<KeyBinding> = keymap
        .view_bindings()
        .map(|(view, keys, action)| {
            let keys: Vec<String> = keys.iter().map(|&key| gpui_keystroke(key)).collect();
            KeyBinding::new(
                &keys.join(" "),
                Dispatch(action.into()),
                Some(view.predicate()),
            )
        })
        .collect();
    cx.bind_keys(bindings);
    cx.set_global(AppKeymap(keymap));
}

/// Como escribe GPUI una tecla: una mayúscula es `shift-` y su minúscula.
fn gpui_keystroke(keystroke: Keystroke) -> String {
    let mut text = String::new();
    if keystroke.ctrl {
        text.push_str("ctrl-");
    }
    let upper = matches!(keystroke.key, KeyName::Char(c) if c.is_ascii_uppercase());
    if keystroke.shift || upper {
        text.push_str("shift-");
    }
    match keystroke.key {
        KeyName::Char(c) if upper => text.push(c.to_ascii_lowercase()),
        key => text.push_str(&key.label()),
    }
    text
}

#[cfg(test)]
mod tests {
    use gpui_kit::{
        Context, FocusHandle, InteractiveElement as _, IntoElement, KeyContext, ParentElement as _,
        Render, TestAppContext, VisualTestContext, div,
    };
    use metalmind_keymap::{Action as AnyAction, Command};

    use super::*;

    /// Atiende [`Dispatch`] como la raíz de la ventana y anota lo que le
    /// llega.
    struct Probe {
        received: Vec<Invocation>,
        focus: FocusHandle,
        context: KeyContext,
    }

    impl Render for Probe {
        fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
            div()
                .key_context(self.context.clone())
                .track_focus(&self.focus)
                .on_action(cx.listener(|probe, action: &Dispatch, _, _| {
                    probe.received.push(action.0.clone());
                }))
                .child("probe")
        }
    }

    fn probe(cx: &mut TestAppContext) -> (gpui_kit::Entity<Probe>, &mut VisualTestContext) {
        probe_in(cx, EDITOR_CONTEXT)
    }

    /// Una vista con los identificadores de `context` separados por espacio,
    /// como `Picker splits`.
    fn probe_in<'a>(
        cx: &'a mut TestAppContext,
        context: &str,
    ) -> (gpui_kit::Entity<Probe>, &'a mut VisualTestContext) {
        cx.update(|cx| bind_keys(Keymap::default(), cx));
        let context = KeyContext::parse(context).unwrap();
        let (probe, cx) = cx.add_window_view(|_, cx| Probe {
            received: Vec::new(),
            focus: cx.focus_handle(),
            context,
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

    /// Cada atajo de una vista, con las teclas como las escribe GPUI, llega
    /// a esa vista como su acción.
    #[test]
    fn cada_atajo_de_una_vista_llega_como_su_accion() {
        for (view, keys, action) in Keymap::default().view_bindings() {
            let mut cx = TestAppContext::single();
            let (probe, cx) = probe_in(&mut cx, &view.predicate().replace(" && ", " "));
            let keys: Vec<String> = keys.iter().map(|&key| gpui_keystroke(key)).collect();
            cx.simulate_keystrokes(&keys.join(" "));
            let received = probe.read_with(cx, |probe, _| probe.received.clone());
            assert_eq!(received, [action.into()], "{keys:?} en {view:?}");
        }
    }

    #[test]
    fn una_mayuscula_es_shift_y_su_minuscula() {
        let cases = [
            (Keystroke::plain(KeyName::Char('G')), "shift-g"),
            (Keystroke::plain(KeyName::Char(' ')), "space"),
            (Keystroke::plain(KeyName::Char('/')), "/"),
            (Keystroke::ctrl(KeyName::Char('w')), "ctrl-w"),
            (Keystroke::ctrl_shift(KeyName::Char('p')), "ctrl-shift-p"),
            (Keystroke::ctrl(KeyName::Backspace), "ctrl-backspace"),
            (Keystroke::plain(KeyName::Home), "home"),
        ];
        for (keystroke, text) in cases {
            assert_eq!(gpui_keystroke(keystroke), text);
        }
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
