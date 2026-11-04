//! Una acción que subió hasta la raíz sin que la atendiera nadie: su vista
//! no estaba en el foco, como una del panel lateral tecleada en el editor o
//! elegida en la paleta. La raíz le mueve el foco a esa vista y la vuelve a
//! despachar.

use gpui_kit::{Context, Window};
use metalmind_keymap::{Invocation, Owner};

use super::WorkspaceView;
use crate::actions::Dispatch;
use crate::workspace::Message;

/// Qué vistas de fuera de los paneles se pintan.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct Visible {
    pub sidebar: bool,
    pub list: bool,
    pub context: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Reroute {
    Sidebar,
    List,
    Context,
    Editor,
    /// Su vista no se ve: no hay a quién dársela.
    Nowhere,
}

/// Solo apunta a una vista que se está pintando y que atiende todas las
/// acciones de su dueño, así que la acción no vuelve a la raíz.
pub(super) fn reroute(owner: Owner, visible: Visible, overlay_open: bool) -> Reroute {
    match owner {
        Owner::Panel if visible.sidebar => Reroute::Sidebar,
        Owner::List if visible.list => Reroute::List,
        Owner::Context if visible.context => Reroute::Context,
        // El panel enfocado dice qué hacer si no muestra una pizarra.
        Owner::Editor | Owner::Board if !overlay_open => Reroute::Editor,
        // Un picker o la vista previa de un cambio abiertos ya tienen el foco; la
        // raíz atiende las de la ventana antes de llegar aquí.
        Owner::Panel
        | Owner::List
        | Owner::Context
        | Owner::Editor
        | Owner::Board
        | Owner::Picker
        | Owner::ChangePreview
        | Owner::Window => Reroute::Nowhere,
    }
}

impl WorkspaceView {
    /// Una acción que llegó a la raíz sin que la atendiera su vista.
    pub(super) fn reroute(
        &mut self,
        invocation: &Invocation,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let visible = Visible {
            sidebar: self.sidebar.read(cx).is_visible(),
            list: self.list.read(cx).is_visible(),
            context: self.context.read(cx).is_visible(),
        };
        match reroute(invocation.action.owner(), visible, self.overlay.is_some()) {
            Reroute::Sidebar => self.focus_sidebar(window, cx),
            Reroute::List => self.focus_list(window, cx),
            Reroute::Context => self.focus_context(window, cx),
            Reroute::Editor => self.focus(window, cx),
            Reroute::Nowhere => {
                let name = invocation.action.name();
                let error = format!("{name} no tiene a quién llegar: su vista no está abierta");
                self.workspace.update(cx, |workspace, _| {
                    workspace.show_message(Message::Error(error));
                });
                cx.notify();
                return;
            }
        }
        window.dispatch_action(Box::new(Dispatch(invocation.clone())), cx);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cada_accion_va_a_su_vista_si_se_ve() {
        let shown = |sidebar, list, context| Visible {
            sidebar,
            list,
            context,
        };
        let cases = [
            (
                Owner::Panel,
                shown(true, false, false),
                false,
                Reroute::Sidebar,
            ),
            (
                Owner::Panel,
                shown(true, false, false),
                true,
                Reroute::Sidebar,
            ),
            (
                Owner::Panel,
                shown(false, true, true),
                false,
                Reroute::Nowhere,
            ),
            (Owner::List, shown(false, true, false), false, Reroute::List),
            (
                Owner::List,
                shown(true, false, true),
                false,
                Reroute::Nowhere,
            ),
            (
                Owner::Context,
                shown(false, false, true),
                false,
                Reroute::Context,
            ),
            (
                Owner::Context,
                shown(true, true, false),
                false,
                Reroute::Nowhere,
            ),
            (
                Owner::Editor,
                shown(false, false, false),
                false,
                Reroute::Editor,
            ),
            (
                Owner::Editor,
                shown(true, true, true),
                true,
                Reroute::Nowhere,
            ),
            (
                Owner::Picker,
                shown(true, true, true),
                false,
                Reroute::Nowhere,
            ),
            (
                Owner::Window,
                shown(true, true, true),
                false,
                Reroute::Nowhere,
            ),
        ];
        for (owner, visible, overlay, expected) in cases {
            assert_eq!(reroute(owner, visible, overlay), expected, "{owner:?}");
        }
    }
}
