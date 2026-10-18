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
pub(super) struct Playback(VecDeque<Repeated>);

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
        self.0.push_front(Repeated {
            keys: keys.into(),
            times,
            next: 0,
        });
    }

    fn pop_front(&mut self) -> Option<Key> {
        let front = self.0.front_mut()?;
        let key = front.keys[front.next];
        front.next += 1;
        if front.next == front.keys.len() {
            front.next = 0;
            front.times -= 1;
            if front.times == 0 {
                self.0.pop_front();
            }
        }
        Some(key)
    }

    fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    fn clear(&mut self) {
        self.0.clear();
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

    /// Reproduce lo que dejó `@` en la cola. Como en vim, todo cuenta como
    /// un solo cambio para `u`, y un comando que falla descarta lo que
    /// queda. Devuelve lo último que quien aloja el editor tiene que hacer o
    /// avisar, como un `:w` o una búsqueda que dio la vuelta.
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
        let mut played = 0;
        while let Some(key) = self.playback.pop_front() {
            played += 1;
            let outcome = self.process(key, buffer, selections, clipboard)?;
            if !matches!(
                outcome,
                KeyOutcome::Pending
                    | KeyOutcome::Executed(_)
                    | KeyOutcome::Inserted
                    | KeyOutcome::Unbound
            ) {
                for_host = Some(outcome);
            }
            if self.failed || played >= MAX_PLAYED_KEYS {
                self.playback.clear();
            }
        }
        Ok(for_host)
    }
}
