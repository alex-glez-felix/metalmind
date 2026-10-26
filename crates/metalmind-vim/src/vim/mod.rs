mod awaiting;
mod block;
mod checkbox;
mod command_line;
mod dispatch;
mod ex;
mod find;
mod insert;
mod jumps;
mod macros;
mod marks;
mod operate;
mod outcome;
mod repeat;
mod scroll;
mod search;
mod substitute;
mod table;
mod visual;

use std::collections::HashSet;

use self::awaiting::Awaiting;
use self::ex::GlobalRun;
use self::macros::{Playback, Recording, Replay};
pub use self::outcome::KeyOutcome;
use self::search::{AwaitingSearch, SearchMemory};
use self::substitute::LastSubstitute;

use std::ops::Range;

use metalmind_buffer::{Buffer, ByteOffset};

use crate::block::Pending;
use crate::dot::Dot;
use crate::find::Find;
use crate::history::History;
use crate::insert::{Session, Typed};
use crate::jumplist::FileId;
use crate::keymap::{Bound, Lookup};
use crate::mark::FileMarks;
use crate::register::{RegisterName, Registers, SystemRegister};
use crate::scroll::Viewport;
use crate::visual::{LastVisual, Reselect};
use crate::{Action, Indent, Key, Keymap, Markdown, Mode, Prompt, Target, VimError};

/// Lo de vim que es de todo el editor: el modo, lo pendiente, los
/// registros, las macros, las marks `A`–`Z` y los historiales. No hace I/O:
/// cada tecla edita el [`Target`] que le presta quien lo aloja.
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
    /// `A`–`Z`, de todos los buffers.
    file_marks: FileMarks,
    /// Los archivos que ya no existen: `ctrl-o` salta sus entradas.
    missing_files: HashSet<FileId>,
    /// La última macro reproducida, para `@@`.
    last_played: Option<Replay>,
    /// La última tecla falló como un `beep_flush` de vim: corta la macro.
    failed: bool,
    /// Lo que vuelve a seleccionar `gv`.
    last_visual: Option<LastVisual>,
    /// El tamaño de la última selección sobre la que se operó, para
    /// `[count]v`, `[count]V` y `[count]ctrl-v`.
    last_visual_size: Option<Reselect>,
    /// Tras `o` en un bloque, el `$` sigue valiendo para operar pero no para
    /// la próxima motion, como el `w_set_curswant` de vim.
    stale_line_end: bool,
    /// Lo que `I`, `A` o `c` sobre un bloque repiten al salir de Insert, por
    /// selección.
    block_insert: Option<Vec<Pending>>,
    scrolloff: usize,
    indent: Indent,
    markdown: Markdown,
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
            file_marks: FileMarks::default(),
            missing_files: HashSet::new(),
            last_played: None,
            failed: false,
            last_visual: None,
            last_visual_size: None,
            stale_line_end: false,
            block_insert: None,
            scrolloff: 0,
            indent: Indent::default(),
            markdown: Markdown::default(),
        }
    }

    /// El ancho y los caracteres con que indentan `>` y `<`.
    pub fn set_indent(&mut self, indent: Indent) {
        self.indent = indent;
    }

    /// Qué partes de la edición de markdown están encendidas.
    pub fn set_markdown(&mut self, markdown: Markdown) {
        self.markdown = markdown;
    }

    #[must_use]
    pub fn mode(&self) -> Mode {
        self.mode
    }

    /// En Insert sobre un bloque, lo escrito hasta ahora en la primera
    /// línea del primero: lo que `<Esc>` repetirá en las demás, medido como
    /// al repetirlo. Vacío si se borró de más. `None` fuera de esa pasada.
    #[must_use]
    pub fn block_insert_written(
        &self,
        buffer: &Buffer,
        viewport: &Viewport,
    ) -> Option<Range<ByteOffset>> {
        let pending = self.block_insert.as_ref()?.first()?;
        Some(pending.written(buffer, viewport.columns()))
    }

    /// Pasa una tecla al motor. Si la tecla reproduce una macro, la
    /// reproduce entera.
    ///
    /// # Errors
    /// Solo si una edición resulta inválida para el buffer, lo que sería un bug.
    pub fn feed(&mut self, key: Key, t: &mut Target<'_>) -> Result<KeyOutcome, VimError> {
        let recording = self.recording.is_some();
        let outcome = self.process(key, t)?;
        // Así no se graba la `q` que termina la grabación.
        if recording {
            self.record([key]);
        }
        self.play_queued(outcome, t)
    }

    /// Una tecla, tecleada o de una macro. Como en vim, una búsqueda o un
    /// `:s` que no encuentran nada fallan.
    fn process(&mut self, key: Key, t: &mut Target<'_>) -> Result<KeyOutcome, VimError> {
        self.failed = false;
        let version = t.buffer.version();
        let outcome = self.interpret(key, t)?;
        self.follow_cursor(t, version);
        if matches!(
            outcome,
            KeyOutcome::SearchFailed(_) | KeyOutcome::InvalidCommand(_)
        ) {
            self.failed = true;
        }
        Ok(outcome)
    }

    fn interpret(&mut self, key: Key, t: &mut Target<'_>) -> Result<KeyOutcome, VimError> {
        if let Some(awaiting) = self.awaiting.take() {
            return self.complete(awaiting, key, t);
        }
        if self.push_count_digit(key) {
            return Ok(KeyOutcome::Pending);
        }
        self.pending.push(key);
        match self.keymap.lookup(self.mode, &self.pending) {
            Lookup::Pending => Ok(KeyOutcome::Pending),
            Lookup::Action(Bound::Engine(action)) => {
                self.pending.clear();
                self.run(action, t)
            }
            Lookup::Action(Bound::Host(action)) => {
                self.reset();
                Ok(KeyOutcome::Dispatch(action.into()))
            }
            Lookup::Unbound => self.unbound(t),
        }
    }

    /// Ejecuta una acción por nombre ya resuelto, como harán la paleta y `:`.
    /// Las acciones que no aplican al modo actual solo cancelan lo pendiente.
    /// Enviar la línea de comandos devuelve el comando para quien aloja el
    /// editor.
    ///
    /// # Errors
    /// Solo si una edición resulta inválida para el buffer, lo que sería un bug.
    pub fn execute(&mut self, action: Action, t: &mut Target<'_>) -> Result<KeyOutcome, VimError> {
        let version = t.buffer.version();
        let outcome = self.run(action, t)?;
        self.follow_cursor(t, version);
        Ok(outcome)
    }

    /// Lo siguiente usa `"+` o `"*`, como teclear `"+`, pero antes descarta
    /// lo pendiente (un count, `r`, `f`, otro registro) para no completarlo.
    /// Así copia y pega la UI con su portapapeles en Visual (`"+y`, `"+P`).
    pub fn use_register(&mut self, register: SystemRegister) {
        self.reset();
        self.selected = Some(RegisterName::System(register));
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
            self.stale_line_end = false;
        }
        self.block_insert = None;
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

    fn unbound(&mut self, t: &mut Target<'_>) -> Result<KeyOutcome, VimError> {
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
        self.write_text(&text, t)
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
