//! Un workspace manejado desde los tests como lo maneja la ventana.

use std::path::Path;

use metalmind_buffer::ByteOffset;
use metalmind_keymap::{Command, Invocation, Keymap};
use metalmind_vault::SaveKind;
use metalmind_vim::{ActionName, Clipboard, Key, KeyOutcome, Markdown, Mode, SystemRegister};

use super::panes_oracle::{Panes, Window};
use super::{Effect, LineNumbers, Message, Options, PaneId, PaneOptions, PaneView, Workspace};
use crate::disk::Files;
use crate::document::{Document, FileStats, SaveError};

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

/// El de `nvim --clean`, para comparar con el oráculo.
const VIM_TABSTOP: usize = 8;

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
        Self::with_scrolloff(text, 0)
    }

    pub fn with_scrolloff(text: &str, scrolloff: usize) -> Self {
        Self::with(
            Document::new(Path::new("prueba.md"), text),
            scrolloff,
            VIM_TABSTOP,
        )
    }

    pub fn with_tabstop(text: &str, tabstop: usize) -> Self {
        Self::with(Document::new(Path::new("prueba.md"), text), 0, tabstop)
    }

    pub fn unnamed() -> Self {
        Self::with(Document::unnamed(), 0, VIM_TABSTOP)
    }

    pub fn open(path: &Path) -> Self {
        Self::with(Document::open(path).unwrap(), 0, VIM_TABSTOP)
    }

    pub fn without_gutter_marks(text: &str) -> Self {
        Self::with_options(
            Document::new(Path::new("prueba.md"), text),
            Options {
                scrolloff: 0,
                tabstop: VIM_TABSTOP,
                expandtab: false,
                gutter_marks: false,
                markdown: Markdown::default(),
            },
        )
    }

    fn with(document: Document, scrolloff: usize, tabstop: usize) -> Self {
        let options = Options {
            scrolloff,
            tabstop,
            expandtab: false,
            gutter_marks: true,
            markdown: Markdown::default(),
        };
        Self::with_options(document, options)
    }

    fn with_options(document: Document, options: Options) -> Self {
        Self {
            workspace: Workspace::new(
                document,
                PaneOptions {
                    numbers: LineNumbers::HYBRID,
                    wrap: true,
                },
                options,
                &Keymap::default(),
            ),
            effect: Effect::Stay,
            clipboard: TestClipboard::default(),
        }
    }

    pub fn pane(&self) -> PaneId {
        self.workspace.focused()
    }

    /// Teclea en el panel enfocado; `⎋`, `⏎`, `⌫`, `⇥` y `⌃w` son Escape,
    /// Enter, Backspace, Tab y `ctrl-w`. Devuelve lo que pasó con la última tecla y
    /// ejecuta cada comando que llega.
    pub fn keys(&mut self, text: &str) -> Option<KeyOutcome> {
        let mut last = None;
        let mut chars = text.chars();
        while let Some(c) = chars.next() {
            let key = match c {
                '⎋' => Key::Escape,
                '⏎' => Key::Enter,
                '⌫' => Key::Backspace,
                '⇥' => Key::Tab,
                '⌃' => Key::Ctrl(chars.next().unwrap()),
                _ => Key::Char(c),
            };
            let outcome = self
                .workspace
                .feed(self.pane(), key, &mut self.clipboard)
                .unwrap();
            if let KeyOutcome::Dispatch(invocation) = &outcome {
                let invocation = Invocation::try_from(invocation.clone()).unwrap();
                self.effect = self.workspace.run(&invocation, &mut self.clipboard);
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

    /// `ctrl-shift-c`.
    pub fn copy(&mut self) {
        self.workspace
            .copy(self.pane(), &mut self.clipboard)
            .unwrap();
    }

    /// `ctrl-shift-v`.
    pub fn paste_clipboard(&mut self) {
        self.workspace
            .paste_clipboard(self.pane(), &mut self.clipboard)
            .unwrap();
    }

    /// Un clic sin arrastrar, como lo manda el editor.
    pub fn click(&mut self, offset: ByteOffset) {
        let pane = self.pane();
        if self.workspace.press(pane, offset).unwrap() {
            self.workspace
                .toggle_checkbox(pane, &mut self.clipboard)
                .unwrap();
        }
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

    /// Primera línea visible de `pane`, desde 1 como en vim.
    pub fn top(&self, pane: PaneId) -> usize {
        self.view(pane).viewport.top() + 1
    }

    /// Línea del cursor de `pane`, desde 1.
    pub fn line(&self, pane: PaneId) -> usize {
        let view = self.view(pane);
        view.buffer().line_of(view.cursor()) + 1
    }

    /// Los paneles como los describe el oráculo de nvim, en el mismo orden,
    /// con los nombres de los archivos de `dir` sin la carpeta.
    pub fn panes(&self, dir: &Path) -> Panes {
        let mut windows: Vec<Window> = self
            .workspace
            .tree()
            .panes()
            .into_iter()
            .map(|pane| {
                let view = self.view(pane);
                let buffer = view.buffer();
                let line = buffer.line_of(view.cursor());
                Window {
                    file: view
                        .document
                        .path()
                        .unwrap()
                        .file_name()
                        .unwrap()
                        .to_string_lossy()
                        .into_owned(),
                    focused: view.focused,
                    line: line + 1,
                    column: view.cursor().get() - buffer.line_start(line).get(),
                }
            })
            .collect();
        windows.sort();
        let error = match self.workspace.message() {
            Some(Message::Error(text)) => text.replace(&format!("{}/", dir.display()), ""),
            _ => String::new(),
        };
        let mut modified: Vec<String> = self
            .workspace
            .documents
            .iter()
            .filter(|(_, document)| document.is_modified())
            .filter_map(|(_, document)| document.path()?.file_name())
            .map(|name| name.to_string_lossy().into_owned())
            .collect();
        modified.sort();
        Panes {
            mode: vim_mode(self.mode()).to_owned(),
            error,
            modified,
            windows,
        }
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

    /// El nombre del archivo del panel enfocado.
    pub fn file(&self) -> String {
        self.view(self.pane())
            .document
            .path()
            .and_then(|path| path.file_name())
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_default()
    }

    pub fn is_modified(&self) -> bool {
        self.view(self.pane()).document.is_modified()
    }

    pub fn exists_on_disk(&self) -> bool {
        self.view(self.pane()).document.exists_on_disk()
    }

    /// Guarda directo, sin pasar por `:w`, para probar a mitad de un Insert.
    pub fn save(&mut self) -> Result<FileStats, SaveError> {
        let id = self.workspace.panes[&self.pane()].document;
        self.workspace
            .documents
            .get_mut(id)
            .unwrap()
            .save(&Files, SaveKind::Explicit)
    }
}

/// Como `mode()` de vim.
fn vim_mode(mode: Mode) -> &'static str {
    match mode {
        Mode::Normal => "n",
        Mode::Insert => "i",
        Mode::CommandLine => "c",
        Mode::OperatorPending(_) => "no",
        Mode::Visual(metalmind_vim::VisualKind::Char) => "v",
        Mode::Visual(metalmind_vim::VisualKind::Line) => "V",
        Mode::Visual(metalmind_vim::VisualKind::Block) => "\u{16}",
    }
}

/// Lo que devuelve el motor cuando se teclea `command` sin argumento.
pub fn dispatched(command: Command) -> KeyOutcome {
    KeyOutcome::Dispatch(ActionName::new(command.name()).into())
}
