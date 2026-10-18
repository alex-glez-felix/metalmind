mod awaiting;
mod command_line;
mod dispatch;
mod ex;
mod find;
mod insert;
mod macros;
mod marks;
mod operate;
mod outcome;
mod repeat;
mod scroll;
mod search;
mod substitute;
mod visual;

use nota_buffer::{Buffer, Selections};

use self::awaiting::Awaiting;
use self::ex::GlobalRun;
use self::macros::{Playback, Recording, Replay};
pub use self::outcome::KeyOutcome;
use self::search::{AwaitingSearch, SearchMemory};
use self::substitute::LastSubstitute;

use crate::dot::Dot;
use crate::find::Find;
use crate::history::History;
use crate::insert::{Session, Typed};
use crate::keymap::Lookup;
use crate::mark::Marks;
use crate::register::{Clipboard, Detached, RegisterName, Registers, SystemRegister};
use crate::visual::{LastVisual, VisualSize};
use crate::{Action, Key, Keymap, Mode, Prompt, Viewport, VimError};

/// Estado del modo vim de un editor. No hace I/O: recibe teclas y edita el
/// buffer y las selecciones que le pasan.
#[derive(Debug)]
pub struct Vim {
    keymap: Keymap,
    mode: Mode,
    pending: Vec<Key>,
    count: Option<usize>,
    /// El count tecleado antes de `"x` o del operador; se multiplica con
    /// el de después.
    operator_count: Option<usize>,
    /// El registro elegido con `"x` para lo siguiente.
    selected: Option<RegisterName>,
    registers: Registers,
    /// `"+` y `"*` cuando quien aloja el motor no da un portapapeles.
    detached: Detached,
    command_line: String,
    prompt: Prompt,
    command_history: History,
    /// La última línea de `:` enviada, que repite `@:`.
    last_command_line: Option<String>,
    /// Cuántos `:g` y `:normal` se están corriendo uno dentro de otro.
    ex_depth: usize,
    /// Cuántas líneas de `:` corren una dentro de otra (`:normal @:`), como
    /// `cmdline_call_depth` de vim.
    command_depth: usize,
    /// El `:g` en curso.
    global: Option<GlobalRun>,
    /// Tras `d/` o `3/`, el operador y el count esperan al patrón.
    awaiting_search: Option<AwaitingSearch>,
    search: SearchMemory,
    last_substitute: Option<LastSubstitute>,
    /// La pasada por Insert en curso, si se está en Insert.
    insert: Option<Session>,
    /// Lo que repite `.`.
    last_change: Option<Dot>,
    /// Lo tecleado en la última pasada por Insert: el registro `".`.
    last_insert: Option<Vec<Typed>>,
    /// Lo que espera la tecla siguiente como carácter, como el de `f`.
    awaiting: Option<Awaiting>,
    last_find: Option<Find>,
    recording: Option<Recording>,
    playback: Playback,
    marks: Marks,
    /// La última macro reproducida, para `@@`.
    last_played: Option<Replay>,
    /// La última tecla falló como un `beep_flush` de vim: corta la macro.
    failed: bool,
    /// Lo que vuelve a seleccionar `gv`.
    last_visual: Option<LastVisual>,
    /// El tamaño de la última selección sobre la que se operó, para
    /// `[count]v` y `[count]V`.
    last_visual_size: Option<VisualSize>,
    /// La ventana del panel que se edita, prestada por quien aloja el motor.
    viewport: Viewport,
    scrolloff: usize,
}

impl Default for Vim {
    fn default() -> Self {
        Self::new()
    }
}

impl Vim {
    #[must_use]
    pub fn new() -> Self {
        Self::with_keymap(Keymap::vim())
    }

    #[must_use]
    pub fn with_keymap(keymap: Keymap) -> Self {
        Self {
            keymap,
            mode: Mode::Normal,
            pending: Vec::new(),
            count: None,
            operator_count: None,
            selected: None,
            registers: Registers::default(),
            detached: Detached::default(),
            command_line: String::new(),
            prompt: Prompt::Command,
            command_history: History::default(),
            last_command_line: None,
            ex_depth: 0,
            command_depth: 0,
            global: None,
            awaiting_search: None,
            search: SearchMemory::default(),
            last_substitute: None,
            insert: None,
            last_change: None,
            last_insert: None,
            awaiting: None,
            last_find: None,
            recording: None,
            playback: Playback::default(),
            marks: Marks::default(),
            last_played: None,
            failed: false,
            last_visual: None,
            last_visual_size: None,
            viewport: Viewport::default(),
            scrolloff: 0,
        }
    }

    #[must_use]
    pub fn mode(&self) -> Mode {
        self.mode
    }

    #[must_use]
    pub fn keymap(&self) -> &Keymap {
        &self.keymap
    }

    /// Como [`Vim::feed_with_clipboard`], con `"+` y `"*` como registros
    /// normales.
    ///
    /// # Errors
    /// Solo si una edición resulta inválida para el buffer, lo que sería un bug.
    pub fn feed(
        &mut self,
        key: Key,
        buffer: &mut Buffer,
        selections: &mut Selections,
    ) -> Result<KeyOutcome, VimError> {
        self.with_detached(|vim, clipboard| {
            vim.feed_with_clipboard(key, buffer, selections, clipboard)
        })
    }

    /// Pasa una tecla al motor; `clipboard` es lo que leen y escriben `"+`
    /// y `"*`. Si la tecla reproduce una macro, la reproduce entera.
    ///
    /// # Errors
    /// Solo si una edición resulta inválida para el buffer, lo que sería un bug.
    pub fn feed_with_clipboard(
        &mut self,
        key: Key,
        buffer: &mut Buffer,
        selections: &mut Selections,
        clipboard: &mut dyn Clipboard,
    ) -> Result<KeyOutcome, VimError> {
        let recording = self.recording.is_some();
        let outcome = self.process(key, buffer, selections, clipboard)?;
        // Así no se graba la `q` que termina la grabación.
        if recording {
            self.record([key]);
        }
        self.play_queued(outcome, buffer, selections, clipboard)
    }

    /// Una tecla, tecleada o de una macro. Como en vim, una búsqueda o un
    /// `:s` que no encuentran nada fallan.
    fn process(
        &mut self,
        key: Key,
        buffer: &mut Buffer,
        selections: &mut Selections,
        clipboard: &mut dyn Clipboard,
    ) -> Result<KeyOutcome, VimError> {
        self.failed = false;
        let version = buffer.version();
        let outcome = self.interpret(key, buffer, selections, clipboard)?;
        self.follow_cursor(buffer, selections, version);
        if matches!(
            outcome,
            KeyOutcome::SearchFailed(_) | KeyOutcome::InvalidCommand(_)
        ) {
            self.failed = true;
        }
        Ok(outcome)
    }

    fn interpret(
        &mut self,
        key: Key,
        buffer: &mut Buffer,
        selections: &mut Selections,
        clipboard: &mut dyn Clipboard,
    ) -> Result<KeyOutcome, VimError> {
        if let Some(awaiting) = self.awaiting.take() {
            return self.complete(awaiting, key, buffer, selections, clipboard);
        }
        if self.push_count_digit(key) {
            return Ok(KeyOutcome::Pending);
        }
        self.pending.push(key);
        match self.keymap.lookup(self.mode, &self.pending) {
            Lookup::Pending => Ok(KeyOutcome::Pending),
            Lookup::Action(action) => {
                self.pending.clear();
                self.run(action, buffer, selections, clipboard)
            }
            Lookup::Unbound => self.unbound(buffer, selections),
        }
    }

    /// Como [`Vim::execute_with_clipboard`], con `"+` y `"*` como registros
    /// normales.
    ///
    /// # Errors
    /// Solo si una edición resulta inválida para el buffer, lo que sería un bug.
    pub fn execute(
        &mut self,
        action: Action,
        buffer: &mut Buffer,
        selections: &mut Selections,
    ) -> Result<KeyOutcome, VimError> {
        self.with_detached(|vim, clipboard| {
            vim.execute_with_clipboard(action, buffer, selections, clipboard)
        })
    }

    /// Ejecuta una acción por nombre ya resuelto, como harán la paleta y `:`.
    /// Las acciones que no aplican al modo actual solo cancelan lo pendiente.
    /// Enviar la línea de comandos devuelve el comando para quien aloja el
    /// editor.
    ///
    /// # Errors
    /// Solo si una edición resulta inválida para el buffer, lo que sería un bug.
    pub fn execute_with_clipboard(
        &mut self,
        action: Action,
        buffer: &mut Buffer,
        selections: &mut Selections,
        clipboard: &mut dyn Clipboard,
    ) -> Result<KeyOutcome, VimError> {
        let version = buffer.version();
        let outcome = self.run(action, buffer, selections, clipboard)?;
        self.follow_cursor(buffer, selections, version);
        Ok(outcome)
    }

    /// `action` con `"+` o `"*`, como si se tecleara el registro y luego la
    /// acción, pero antes descarta lo pendiente (un count, `r`, `f`, otro
    /// registro) para no completarlo. Así copia y pega la UI con su
    /// portapapeles en Visual (`"+y`, `"+P`).
    ///
    /// # Errors
    /// Solo si una edición resulta inválida para el buffer, lo que sería un bug.
    pub fn execute_with_system_register(
        &mut self,
        register: SystemRegister,
        action: Action,
        buffer: &mut Buffer,
        selections: &mut Selections,
        clipboard: &mut dyn Clipboard,
    ) -> Result<KeyOutcome, VimError> {
        self.reset();
        self.selected = Some(RegisterName::System(register));
        self.execute_with_clipboard(action, buffer, selections, clipboard)
    }

    fn with_detached<T>(&mut self, run: impl FnOnce(&mut Self, &mut Detached) -> T) -> T {
        let mut detached = std::mem::take(&mut self.detached);
        let result = run(self, &mut detached);
        self.detached = detached;
        result
    }

    /// Cambia las marks del motor por `marks`: antes de editar otro buffer
    /// se le dan las suyas y después se recuperan las del anterior.
    pub fn swap_marks(&mut self, marks: &mut Marks) {
        std::mem::swap(&mut self.marks, marks);
    }

    /// El cursor se movió por fuera del motor, como con un clic o un pegado
    /// en Normal: se cancela lo pendiente (count, operador, `f`) y, como en
    /// vim, lo tecleado en Insert ya no se repite con el count y lo que se
    /// teclee después es otro cambio para `.`. Un clic en Visual termina la
    /// selección; quien aloja el editor pone después el cursor.
    pub fn cursor_moved(&mut self) {
        self.reset();
        if matches!(self.mode, Mode::Visual(_)) {
            self.mode = Mode::Normal;
        }
        if let Some(session) = &mut self.insert {
            session.cursor_moved();
        }
    }

    /// Un `0` sin count previo es la motion `0`, no un dígito.
    fn push_count_digit(&mut self, key: Key) -> bool {
        let Key::Char(c) = key else { return false };
        let starts_count = c != '0' || self.count.is_some();
        let accepts_count = !self.mode.takes_text() && self.pending.is_empty();
        let Some(digit) = c.to_digit(10).filter(|_| starts_count && accepts_count) else {
            return false;
        };
        let count = self.count.unwrap_or(0);
        self.count = Some(count.saturating_mul(10).saturating_add(digit as usize));
        true
    }

    fn unbound(
        &mut self,
        buffer: &mut Buffer,
        selections: &mut Selections,
    ) -> Result<KeyOutcome, VimError> {
        let keys = std::mem::take(&mut self.pending);
        if !self.mode.takes_text() {
            self.fail();
            return Ok(KeyOutcome::Unbound);
        }
        let text: String = keys
            .iter()
            .filter_map(|&key| typed_char(key, self.mode))
            .collect();
        if text.is_empty() {
            return Ok(KeyOutcome::Unbound);
        }
        self.write_text(&text, buffer, selections)
    }

    /// El count de lo que se va a ejecutar: `2"a3x` borra 6.
    fn take_count(&mut self) -> Option<usize> {
        combined(self.operator_count.take(), self.count.take())
    }

    /// Como el `clearopbeep` de vim: cancela lo pendiente y corta la macro
    /// que se esté reproduciendo.
    fn fail(&mut self) {
        self.reset();
        self.failed = true;
    }

    fn reset(&mut self) {
        self.pending.clear();
        self.count = None;
        self.operator_count = None;
        self.selected = None;
        self.awaiting = None;
        if matches!(self.mode, Mode::OperatorPending(_)) {
            self.mode = Mode::Normal;
        }
    }
}

/// `2d3w` opera sobre 6 palabras; sin ningún count, `None`.
fn combined(operator: Option<usize>, motion: Option<usize>) -> Option<usize> {
    match (operator, motion) {
        (None, None) => None,
        (a, b) => Some(a.unwrap_or(1).saturating_mul(b.unwrap_or(1))),
    }
}

fn typed_char(key: Key, mode: Mode) -> Option<char> {
    match key {
        Key::Char(c) => Some(c),
        Key::Tab if mode == Mode::Insert => Some('\t'),
        _ => None,
    }
}
