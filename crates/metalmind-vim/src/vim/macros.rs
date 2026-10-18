//! Macros: `q{a-z}` graba, `q` termina, `@{a-z}` reproduce y `@@` repite.

use std::collections::VecDeque;
use std::rc::Rc;

use metalmind_buffer::{Buffer, Selections};

use super::Vim;
use crate::register::{Clipboard, RegisterName};
use crate::{CommandError, Key, KeyOutcome, VimError, macros};

/// Lo que se va grabando con `q`.
#[derive(Debug)]
pub(super) struct Recording {
    letter: u8,
    append: bool,
    keys: Vec<Key>,
}

/// Lo que repite `@@`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Replay {
    /// `@a`, por el índice de la letra.
    Register(u8),
    /// `@:`, la última línea de `:`.
    CommandLine,
}

/// Vim corta con `ctrl-c` una macro que se llama a sí misma sin fallar
/// nunca; aquí no hay cómo interrumpirla, así que se corta al llegar a esto.
const MAX_PLAYED_KEYS: usize = 1 << 20;

/// Teclas por reproducir, como el typebuf de vim: `@a` mete las de `"a`
/// al principio, así que una macro que se llama a sí misma no crece la pila.
/// Las repeticiones de `{count}@a` no se copian: un count enorme no ocupa
/// más memoria que uno chico.
#[derive(Debug, Default)]
pub(super) struct Playback {
    queue: VecDeque<Repeated>,
    /// Teclas reproducidas desde la tecla que empezó la macro, también entre
    /// las pausas para un comando del host.
    played: usize,
}

/// Las teclas de una macro, `times` veces, desde `next`.
#[derive(Debug)]
struct Repeated {
    keys: Rc<[Key]>,
    times: usize,
    next: usize,
}

impl Playback {
    fn push_front(&mut self, keys: Vec<Key>, times: usize) {
        if keys.is_empty() || times == 0 {
            return;
        }
        self.queue.push_front(Repeated {
            keys: keys.into(),
            times,
            next: 0,
        });
    }

    fn pop_front(&mut self) -> Option<Key> {
        let front = self.queue.front_mut()?;
        let key = front.keys[front.next];
        front.next += 1;
        if front.next == front.keys.len() {
            front.next = 0;
            front.times -= 1;
            if front.times == 0 {
                self.queue.pop_front();
            }
        }
        self.played += 1;
        Some(key)
    }

    fn is_empty(&self) -> bool {
        self.queue.is_empty()
    }

    fn clear(&mut self) {
        self.queue.clear();
    }
}

impl Vim {
    /// La letra del registro en que se graba, si se está grabando.
    #[must_use]
    pub fn recording(&self) -> Option<char> {
        let recording = self.recording.as_ref()?;
        Some(char::from(b'a' + recording.letter))
    }

    /// Lo tecleado por quien usa el editor, no lo que reproduce una macro.
    pub(super) fn record(&mut self, keys: impl IntoIterator<Item = Key>) {
        if let Some(recording) = &mut self.recording {
            recording.keys.extend(keys);
        }
    }

    /// `q{a-z}`, o `q{A-Z}` para agregar al final.
    pub(super) fn start_recording(&mut self, c: char) {
        self.reset();
        match RegisterName::parse(c).and_then(RegisterName::letter) {
            Some((letter, append)) => {
                self.recording = Some(Recording {
                    letter,
                    append,
                    keys: Vec::new(),
                });
            }
            None => self.fail(),
        }
    }

    pub(super) fn stop_recording(&mut self) {
        self.reset();
        if let Some(recording) = self.recording.take() {
            let keys = macros::encode(&recording.keys);
            self.registers
                .record(recording.letter, recording.append, keys);
        }
    }

    /// `{count}@{a-z}`, `@:` para repetir la última línea de `:`, o `@@`
    /// para lo último que se reprodujo. `@:` sin una línea anterior es
    /// `E30`; un registro vacío falla sin mensaje, como en vim.
    pub(super) fn play_macro(
        &mut self,
        c: char,
        clipboard: &mut dyn Clipboard,
    ) -> Option<CommandError> {
        let times = self.take_count().unwrap_or(1);
        self.reset();
        let replay = match c {
            '@' => self.last_played,
            ':' => Some(Replay::CommandLine),
            _ => RegisterName::parse(c)
                .and_then(RegisterName::letter)
                .map(|(letter, _)| Replay::Register(letter)),
        };
        let keys = replay.and_then(|replay| self.replayed_keys(replay, clipboard));
        let (Some(replay), Some(keys)) = (replay, keys) else {
            self.fail();
            return (replay == Some(Replay::CommandLine))
                .then_some(CommandError::NoPreviousCommandLine);
        };
        self.last_played = Some(replay);
        self.playback.push_front(keys, times);
        None
    }

    /// Como vim, `@:` teclea otra vez `:`, la línea y `<CR>`.
    fn replayed_keys(&self, replay: Replay, clipboard: &mut dyn Clipboard) -> Option<Vec<Key>> {
        match replay {
            Replay::Register(letter) => {
                let register = self
                    .registers
                    .read(Some(RegisterName::Named(letter)), clipboard)?;
                (!register.text.is_empty()).then(|| macros::decode(&register))
            }
            Replay::CommandLine => {
                let line = self.last_command_line.as_ref()?;
                let typed = line.chars().map(Key::Char);
                Some(
                    std::iter::once(Key::Char(':'))
                        .chain(typed)
                        .chain([Key::Enter])
                        .collect(),
                )
            }
        }
    }

    /// Corre `keys` en su propia cola, como `:normal`: lo que reproduzca una
    /// macro que las llamó sigue después, y un fallo solo descarta estas.
    pub(super) fn play_isolated(
        &mut self,
        keys: &[Key],
        buffer: &mut Buffer,
        selections: &mut Selections,
        clipboard: &mut dyn Clipboard,
    ) -> Result<Option<KeyOutcome>, VimError> {
        let outer = std::mem::take(&mut self.playback);
        self.playback.push_front(keys.to_vec(), 1);
        let played = self.play(buffer, selections, clipboard);
        self.playback = outer;
        played
    }

    /// Sigue con la macro que se pausó en un comando del host (como
    /// `ctrl-w l` o `:e`), ahora sobre el buffer y los cursores del panel
    /// que quedó con el foco. `None` si no queda nada por reproducir.
    ///
    /// # Errors
    /// Solo si una edición resulta inválida para el buffer, lo que sería un bug.
    pub fn resume_macro(
        &mut self,
        buffer: &mut Buffer,
        selections: &mut Selections,
        clipboard: &mut dyn Clipboard,
    ) -> Result<Option<KeyOutcome>, VimError> {
        if self.playback.is_empty() {
            return Ok(None);
        }
        buffer.begin_undo_group();
        let played = self.play(buffer, selections, clipboard);
        buffer.end_undo_group();
        played
    }

    /// Descarta lo que queda de la macro, como cuando falla un comando.
    pub fn abort_macro(&mut self) {
        self.playback.clear();
    }

    /// Reproduce lo que dejó `@` en la cola. Como en vim, todo cuenta como
    /// un solo cambio para `u` en cada buffer, y un comando que falla
    /// descarta lo que queda. Devuelve lo último que quien aloja el editor
    /// tiene que avisar, como una búsqueda que dio la vuelta; un comando
    /// suyo, como `:w` o `ctrl-w l`, pausa la macro para que lo ejecute antes
    /// de seguir con [`Vim::resume_macro`].
    pub(super) fn play_queued(
        &mut self,
        typed: KeyOutcome,
        buffer: &mut Buffer,
        selections: &mut Selections,
        clipboard: &mut dyn Clipboard,
    ) -> Result<KeyOutcome, VimError> {
        if self.playback.is_empty() {
            return Ok(typed);
        }
        self.playback.played = 0;
        buffer.begin_undo_group();
        let played = self.play(buffer, selections, clipboard);
        buffer.end_undo_group();
        Ok(played?.unwrap_or(typed))
    }

    fn play(
        &mut self,
        buffer: &mut Buffer,
        selections: &mut Selections,
        clipboard: &mut dyn Clipboard,
    ) -> Result<Option<KeyOutcome>, VimError> {
        let mut for_host = None;
        while let Some(key) = self.playback.pop_front() {
            let outcome = self.process(key, buffer, selections, clipboard)?;
            if self.playback.played >= MAX_PLAYED_KEYS {
                self.playback.clear();
            }
            if matches!(outcome, KeyOutcome::Command(_) | KeyOutcome::SwitchFile(_)) {
                return Ok(Some(outcome));
            }
            if !matches!(
                outcome,
                KeyOutcome::Pending
                    | KeyOutcome::Executed(_)
                    | KeyOutcome::Inserted
                    | KeyOutcome::Unbound
            ) {
                for_host = Some(outcome);
            }
            if self.failed {
                self.playback.clear();
            }
        }
        Ok(for_host)
    }
}
