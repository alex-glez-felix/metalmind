//! Una acción que subió hasta la raíz sin que la atendiera nadie: su vista
//! no estaba en el foco, como una del panel lateral tecleada en el editor o
//! elegida en la paleta. La raíz le mueve el foco a esa vista y la vuelve a
//! despachar.

use metalmind_keymap::Owner;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Reroute {
    Sidebar,
    List,
    Editor,
    /// Su vista no se ve: no hay a quién dársela.
    Nowhere,
}

/// Solo apunta a una vista que se está pintando y que atiende todas las
/// acciones de su dueño, así que la acción no vuelve a la raíz.
pub(super) fn reroute(
    owner: Owner,
    sidebar_visible: bool,
    list_visible: bool,
    overlay_open: bool,
) -> Reroute {
    match owner {
        Owner::Panel if sidebar_visible => Reroute::Sidebar,
        Owner::List if list_visible => Reroute::List,
        Owner::Editor if !overlay_open => Reroute::Editor,
        // Un picker o la vista previa de tags abiertos ya tienen el foco; la
        // raíz atiende las de la ventana antes de llegar aquí.
        Owner::Panel
        | Owner::List
        | Owner::Editor
        | Owner::Picker
        | Owner::TagChange
        | Owner::Window => Reroute::Nowhere,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cada_accion_va_a_su_vista_si_se_ve() {
        let cases = [
            (Owner::Panel, true, false, false, Reroute::Sidebar),
            (Owner::Panel, true, false, true, Reroute::Sidebar),
            (Owner::Panel, false, true, false, Reroute::Nowhere),
            (Owner::List, false, true, false, Reroute::List),
            (Owner::List, true, false, false, Reroute::Nowhere),
            (Owner::Editor, false, false, false, Reroute::Editor),
            (Owner::Editor, true, true, true, Reroute::Nowhere),
            (Owner::Picker, true, true, false, Reroute::Nowhere),
            (Owner::Window, true, true, false, Reroute::Nowhere),
        ];
        for (owner, sidebar, list, overlay, expected) in cases {
            assert_eq!(
                reroute(owner, sidebar, list, overlay),
                expected,
                "{owner:?}"
            );
        }
    }
}
