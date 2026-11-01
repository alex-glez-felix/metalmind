//! El panel de las teclas que siguen a un prefijo como `space` (#86): sale
//! abajo a la derecha del panel con el foco tras `which_key.delay_ms` sin
//! teclear, y se va con la siguiente tecla, que lo completa o lo cancela.

use gpui_kit::{
    Anchor, AnyElement, App, Context, Div, InteractiveElement as _, IntoElement as _,
    ParentElement as _, Styled as _, Task, Window, anchored, deferred, div,
    prelude::FluentBuilder as _,
};
use metalmind_keymap::NextKey;

use super::Editor;
use crate::actions::AppKeymap;
use crate::palette;
use crate::theme::Theme;
use crate::workspace::NextKeys;

/// Si el panel se ve; esperando, el temporizador que lo muestra, que se
/// cancela al soltarlo.
#[derive(Default)]
pub(super) enum WhichKey {
    #[default]
    Hidden,
    Waiting {
        _timer: Task<()>,
    },
    Shown,
}

impl Editor {
    /// Tras cada tecla del motor: si quedó un prefijo a medias, el panel
    /// sale tras el retraso; si no, se esconde.
    pub(super) fn schedule_which_key(&mut self, cx: &mut Context<Self>) {
        self.which_key = WhichKey::Hidden;
        let workspace = self.workspace.read(cx);
        let Some(delay) = workspace.which_key_delay() else {
            return;
        };
        if workspace.next_keys(AppKeymap::get(cx)).is_none() {
            return;
        }
        if delay.is_zero() {
            self.which_key = WhichKey::Shown;
            return;
        }
        let timer = cx.spawn(async move |editor, cx| {
            cx.background_executor().timer(delay).await;
            // Sin el editor, no queda panel que mostrar.
            let _ = editor.update(cx, |editor, cx| {
                editor.which_key = WhichKey::Shown;
                cx.notify();
            });
        });
        self.which_key = WhichKey::Waiting { _timer: timer };
    }

    /// Lo que muestra el panel en este frame. Un clic o un comando también
    /// cancelan el prefijo: entonces ya no hay nada que mostrar. Si el foco
    /// se fue a otra vista, se esconde hasta la próxima tecla del editor.
    pub(super) fn next_keys(&mut self, window: &Window, cx: &App) -> Option<NextKeys> {
        if !self.focus_handle.is_focused(window) {
            self.which_key = WhichKey::Hidden;
        }
        if !matches!(self.which_key, WhichKey::Shown) {
            return None;
        }
        self.view(cx).filter(|view| view.focused)?;
        self.workspace.read(cx).next_keys(AppKeymap::get(cx))
    }
}

/// Lo tecleado arriba y, en columnas de `which_key_rows` filas, cada tecla
/// que sigue con su acción. Crece hacia arriba y a la izquierda desde la
/// esquina del panel: en uno angosto pasa sobre los vecinos sin recortarse.
pub(super) fn which_key_panel(next_keys: NextKeys, theme: &Theme) -> AnyElement {
    let panel = keys_box(next_keys, theme);
    div()
        .absolute()
        .bottom(theme.layout.which_key_margin)
        .right(theme.layout.which_key_margin)
        .child(deferred(
            anchored()
                .anchor(Anchor::BottomRight)
                .snap_to_window()
                .child(panel),
        ))
        .into_any_element()
}

fn keys_box(next_keys: NextKeys, theme: &Theme) -> Div {
    let rows = usize::from(theme.layout.which_key_rows).max(1);
    let rows_of_keys: Vec<(&NextKey, String)> =
        next_keys.next.iter().zip(labels(&next_keys.next)).collect();
    let columns = rows_of_keys
        .chunks(rows)
        .map(|column| key_column(column, theme));
    div()
        .debug_selector(|| "which-key".to_owned())
        .occlude()
        .flex()
        .flex_col()
        .gap(theme.layout.which_key_padding_y)
        .px(theme.layout.which_key_padding_x)
        .py(theme.layout.which_key_padding_y)
        .bg(theme.ui.which_key_background)
        .border(theme.stroke.border)
        .border_color(theme.ui.which_key_border)
        .rounded(theme.radius.lg)
        .map(|panel| theme.ui_text(panel))
        .whitespace_nowrap()
        .child(
            div()
                .text_color(theme.ui.which_key_prefix)
                .child(next_keys.typed),
        )
        .child(
            div()
                .flex()
                .flex_row()
                .gap(theme.layout.which_key_gap)
                .children(columns),
        )
}

/// Las teclas en una columna y las acciones en otra, para que se alineen.
fn key_column(column: &[(&NextKey, String)], theme: &Theme) -> Div {
    let keys = column.iter().map(|(next, _)| {
        div()
            .text_color(theme.ui.which_key_key)
            .child(next.keys.join(" / "))
    });
    let actions = column
        .iter()
        .map(|(_, label)| div().text_color(theme.ui.foreground).child(label.clone()));
    div()
        .flex()
        .flex_row()
        .gap(theme.layout.which_key_gap)
        .child(div().flex().flex_col().items_end().children(keys))
        .child(div().flex().flex_col().children(actions))
}

/// Cada acción como en la paleta (`panel: toggle folders`), sin el espacio
/// de nombres si todas lo comparten: tras `g`, `vim:` se repetiría en cada
/// fila.
fn labels(next: &[NextKey]) -> Vec<String> {
    let namespace = |name: &'static str| name.rsplit_once("::").map(|(namespace, _)| namespace);
    let mut namespaces = next.iter().map(|next| namespace(next.action.name()));
    let first = namespaces.next().flatten();
    let shared = first.is_some() && namespaces.all(|other| other == first);
    next.iter()
        .map(|next| {
            let name = next.action.name();
            match name.rsplit_once("::") {
                Some((_, action)) if shared => action.replace('_', " "),
                _ => palette::label(name),
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use metalmind_keymap::Keymap;
    use metalmind_vim::{Key, Mode, Operator};

    use super::*;

    fn next(mode: Mode, keys: &[Key]) -> Vec<String> {
        labels(&Keymap::default().next_keys(mode, keys))
    }

    #[test]
    fn sin_el_espacio_de_nombres_solo_si_todas_lo_comparten() {
        let after_space = next(Mode::Normal, &[Key::Char(' ')]);
        assert_eq!(after_space[0], "workspace: open finder");
        assert_eq!(after_space[3], "panel: toggle folders");
        let after_ctrl_w = next(Mode::Normal, &[Key::Ctrl('w')]);
        assert_eq!(after_ctrl_w[0], "pane: split right");
        assert!(after_ctrl_w.contains(&"editor: quit".to_owned()));
        let objects = next(Mode::OperatorPending(Operator::Delete), &[Key::Char('i')]);
        assert!(objects.contains(&"inner word".to_owned()), "{objects:?}");
        assert!(
            objects.iter().all(|label| !label.contains(':')),
            "{objects:?}"
        );
    }
}

#[cfg(test)]
#[path = "which_key_tests.rs"]
mod view_tests;
