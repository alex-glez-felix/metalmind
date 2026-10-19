//! La ventana de Metalmind en la plataforma de pruebas de GPUI: sin display ni
//! GPU, con un ejecutor determinista y el sistema de texto de verdad, así que
//! el layout le da forma a cada línea como en la app. Sirve para contar las
//! instrucciones de una tecla hasta su frame (`render`, layout y `paint`).

use std::fs;
use std::path::Path;
use std::sync::Arc;

use gpui_kit::{AnyWindowHandle, HeadlessAppContext, Keystroke};
use gpui_wgpu::CosmicTextSystem;
use metalmind_config::Config;
use metalmind_ui::{Document, Startup};
use metalmind_vault::Vault;
use tempfile::TempDir;

/// La familia de reserva del sistema de texto, la misma que usa GPUI en Linux.
const FALLBACK_FONT: &str = "IBM Plex Sans";

/// La nota que abre el editor.
pub const NOTE: &str = "nota.md";

/// Un vault en una carpeta temporal, abierto, con una de sus notas para la ventana.
pub struct Prepared {
    pub cx: HeadlessAppContext,
    pub startup: Startup,
    pub config: Config,
    _dir: TempDir,
}

impl Prepared {
    /// `write` llena la carpeta del vault; `open` es la nota del editor.
    pub fn new(write: impl FnOnce(&Path), open: &str) -> Self {
        let dir = TempDir::new().unwrap();
        write(dir.path());
        let (vault, updates) = Vault::open(dir.path()).unwrap();
        let document = Document::open(&dir.path().join(open)).unwrap();
        // El de la app en Linux, sin la plataforma: sus hilos no entran en la cuenta.
        let text_system = Arc::new(CosmicTextSystem::new(FALLBACK_FONT));
        Self {
            cx: HeadlessAppContext::new(text_system),
            startup: Startup {
                vault,
                updates,
                document,
                paths: None,
            },
            config: Config::defaults().unwrap(),
            _dir: dir,
        }
    }

    /// Una nota sola, `nota.md`, en un vault que no tiene nada más.
    pub fn note(text: &str) -> Self {
        Self::new(|root| fs::write(root.join(NOTE), text).unwrap(), NOTE)
    }

    /// Abre la ventana, que pinta su primer frame, y deja correr lo que
    /// quedó pendiente: las imágenes de las fórmulas, el árbol del panel.
    pub fn open(self) -> Headless {
        let mut headless = self.first_frame();
        headless.settle();
        headless
    }

    /// Solo abrir la ventana: GPUI pinta el primer frame dentro de
    /// `open_window`, como al arrancar la app.
    pub fn first_frame(self) -> Headless {
        let Self {
            mut cx,
            startup,
            config,
            _dir: dir,
        } = self;
        let window = cx
            .update(|cx| metalmind_ui::open_window(startup, &config, cx))
            .unwrap();
        Headless {
            cx,
            window,
            _dir: dir,
        }
    }
}

pub struct Headless {
    cx: HeadlessAppContext,
    window: AnyWindowHandle,
    _dir: TempDir,
}

impl Headless {
    /// Teclea `keys` (en la notación de GPUI: `j`, `ctrl-d`, `shift-g`), cada
    /// una con su frame, y deja correr lo pendiente.
    pub fn type_keys(&mut self, keys: &[&str]) {
        for key in keys {
            self.key_to_frame(key);
        }
        self.settle();
    }

    /// Una tecla y el frame que la muestra, como en la app: lo que cuenta el
    /// benchmark del render.
    pub fn key_to_frame(&mut self, key: &str) {
        let keystroke = Keystroke::parse(key).unwrap();
        self.cx
            .update_window(self.window, |_, window, cx| {
                window.dispatch_keystroke(keystroke, cx);
            })
            .unwrap();
        self.draw();
    }

    /// Deja correr las tareas pendientes (las de fondo también, en este
    /// ejecutor) y pinta lo que hayan cambiado.
    pub fn settle(&mut self) {
        self.cx.run_until_parked();
        self.draw();
    }

    fn draw(&mut self) {
        self.cx
            .update_window(self.window, |_, window, cx| window.draw(cx).clear(cx))
            .unwrap();
    }
}
