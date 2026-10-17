//! Guardar y cerrar: las acciones que invocan `:w`, `:q`, `:q!` y `:wq`.

use gpui_kit::{App, Context, Window};

use super::Editor;
use super::status_bar::Message;
use crate::actions::{ForceQuit, Quit, Save, SaveAndQuit};

const UNSAVED: &str = "E37: hay cambios sin guardar (usa :q! para cerrar igual)";

impl Editor {
    pub(super) fn save(&mut self, _: &Save, _window: &mut Window, cx: &mut Context<Self>) {
        self.write_file();
        cx.notify();
    }

    pub(super) fn quit(&mut self, _: &Quit, window: &mut Window, cx: &mut Context<Self>) {
        if self.may_close() {
            window.remove_window();
        }
        cx.notify();
    }

    pub(super) fn save_and_quit(
        &mut self,
        _: &SaveAndQuit,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.write_file() {
            window.remove_window();
        }
        cx.notify();
    }

    /// Lo consulta la ventana cuando el compositor pide cerrarla.
    pub fn may_close(&mut self) -> bool {
        if self.document.is_modified() {
            self.message = Some(Message::Error(UNSAVED.to_owned()));
            return false;
        }
        true
    }

    fn write_file(&mut self) -> bool {
        match self.document.save() {
            Ok(stats) => {
                let path = self.document.path().display();
                self.message = Some(Message::Info(format!("\"{path}\" {stats} escritos")));
                true
            }
            Err(error) => {
                log::error!("{error}");
                self.message = Some(Message::Error(error.to_string()));
                false
            }
        }
    }
}

/// No consulta al editor: cierra aunque haya cambios.
pub(super) fn force_quit(_: &ForceQuit, window: &mut Window, _cx: &mut App) {
    window.remove_window();
}
