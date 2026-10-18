//! Un workspace manejado desde los tests como lo maneja la ventana.

use std::path::Path;

use nota_buffer::ByteOffset;
use nota_vault::FileError;
use nota_vim::{Clipboard, Key, KeyOutcome, Mode, SystemRegister};

use super::{Effect, LineNumbers, PaneId, PaneView, Workspace};
use crate::document::{Document, FileStats};

/// Guarda lo que se escribe en `"+`; `"*` siempre está vacío.
#[derive(Default)]
pub(crate) struct TestClipboard(pub Option<String>);

impl Clipboard for TestClipboard {
    fn read(&mut self, register: SystemRegister) -> Option<String> {
        (register == SystemRegister::Clipboard)
            .then(|| self.0.clone())
            .flatten()
    }

    fn write(&mut self, register: SystemRegister, text: String) {
        if register == SystemRegister::Clipboard {
            self.0 = Some(text);
        }
    }
}

/// Un workspace manejado como lo hace la ventana: las teclas van al panel
/// enfocado y los comandos que devuelve el motor se ejecutan.
pub(crate) struct Session {
    pub workspace: Workspace,
    /// Lo que pidió el último comando ejecutado.
    pub effect: Effect,
    pub clipboard: TestClipboard,
}

impl Session {
    pub fn new(text: &str) -> Self {
        Self::with(Document::new(Path::new("prueba.md"), text))
    }

    pub fn open(path: &Path) -> Self {
        Self::with(Document::open(path).unwrap())
    }

    fn with(document: Document) -> Self {
        Self {
            workspace: Workspace::new(document, LineNumbers::HYBRID),
            effect: Effect::Stay,
            clipboard: TestClipboard::default(),
        }
    }

    pub fn pane(&self) -> PaneId {
        self.workspace.focused()
    }

    /// Teclea en el panel enfocado; `⎋`, `⏎`, `⌫` y `⌃w` son Escape, Enter,
    /// Backspace y `ctrl-w`. Devuelve lo que pasó con la última tecla y
    /// ejecuta cada comando que llega.
    pub fn keys(&mut self, text: &str) -> Option<KeyOutcome> {
        let mut last = None;
        let mut chars = text.chars();
        while let Some(c) = chars.next() {
            let key = match c {
                '⎋' => Key::Escape,
                '⏎' => Key::Enter,
                '⌫' => Key::Backspace,
                '⌃' => Key::Ctrl(chars.next().unwrap()),
                _ => Key::Char(c),
            };
            let outcome = self
                .workspace
                .feed(self.pane(), key, &mut self.clipboard)
                .unwrap();
            if let KeyOutcome::Command(invocation) = &outcome {
                self.effect = self.workspace.run(invocation, &mut self.clipboard);
            }
            last = Some(outcome);
        }
        last
    }

    pub fn type_text(&mut self, text: &str) -> Option<KeyOutcome> {
        self.workspace
            .type_text(self.pane(), text, &mut self.clipboard)
            .unwrap()
    }

    pub fn paste(&mut self, text: &str) {
        self.workspace.paste(self.pane(), text).unwrap();
    }

    pub fn select(&mut self, anchor: ByteOffset, head: ByteOffset) {
        self.workspace.select(self.pane(), anchor, head).unwrap();
    }

    pub fn selected_text(&self) -> Option<String> {
        self.workspace.selected_text(self.pane()).unwrap()
    }

    pub fn current_line(&self) -> String {
        self.workspace.current_line(self.pane()).unwrap()
    }

    pub fn view(&self, pane: PaneId) -> PaneView<'_> {
        self.workspace.view(pane).unwrap()
    }

    pub fn text(&self) -> String {
        self.view(self.pane()).buffer().to_string()
    }

    pub fn cursor(&self) -> ByteOffset {
        self.view(self.pane()).cursor()
    }

    pub fn mode(&self) -> Mode {
        self.view(self.pane()).mode
    }

    pub fn command_line(&self) -> Option<&str> {
        self.view(self.pane()).command_line.map(|line| line.typed)
    }

    pub fn is_modified(&self) -> bool {
        self.view(self.pane()).document.is_modified()
    }

    pub fn exists_on_disk(&self) -> bool {
        self.view(self.pane()).document.exists_on_disk()
    }

    /// Guarda directo, sin pasar por `:w`, para probar a mitad de un Insert.
    pub fn save(&mut self) -> Result<FileStats, FileError> {
        let id = self.workspace.panes[&self.pane()].document;
        self.workspace.documents.get_mut(id).unwrap().save()
    }
}
