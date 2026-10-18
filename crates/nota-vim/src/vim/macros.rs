//! Macros: `q{a-z}` graba, `q` termina, `@{a-z}` reproduce y `@@` repite.

use std::collections::VecDeque;
use std::rc::Rc;

use nota_buffer::{Buffer, Selections};

use super::Vim;
use crate::register::{Clipboard, RegisterName};
use crate::{Key, KeyOutcome, VimError, macros};

/// Lo que se va grabando con `q`.
#[derive(Debug)]
pub(super) struct Recording {
    letter: u8,
    append: bool,
    keys: Vec<Key>,
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

    /// `{count}@{a-z}`, o `@@` para la última macro reproducida.
    pub(super) fn play_macro(&mut self, c: char, clipboard: &mut dyn Clipboard) {
        let times = self.take_count().unwrap_or(1);
        self.reset();
        let letter = match c {
            '@' => self.last_played,
            _ => RegisterName::parse(c)
                .and_then(RegisterName::letter)
                .map(|(letter, _)| letter),
        };
        let Some(letter) = letter else {
            return self.fail();
        };
        let register = self
            .registers
            .read(Some(RegisterName::Named(letter)), clipboard);
        let Some(register) = register.filter(|register| !register.text.is_empty()) else {
            return self.fail();
        };
        self.last_played = Some(letter);
        self.playback.push_front(macros::decode(&register), times);
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
            if matches!(outcome, KeyOutcome::Command(_)) {
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
