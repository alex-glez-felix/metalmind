use std::collections::VecDeque;
use std::sync::mpsc;

use crate::debounce::Batch;
use crate::worker::{Message, Save};

/// Los mensajes del hilo, con los que ya se sacaron del canal y esperan su
/// turno en orden.
pub(crate) struct Inbox<'a> {
    receiver: &'a mpsc::Receiver<Message>,
    queue: VecDeque<Message>,
}

impl<'a> Inbox<'a> {
    pub(crate) fn new(receiver: &'a mpsc::Receiver<Message>) -> Self {
        Self {
            receiver,
            queue: VecDeque::new(),
        }
    }

    pub(crate) fn next(&mut self) -> Option<Message> {
        self.queue.pop_front().or_else(|| self.receiver.recv().ok())
    }

    /// El siguiente evento de disco si es lo próximo que espera, sin bloquear.
    pub(crate) fn next_disk(&mut self) -> Option<Batch> {
        let message = match self.queue.pop_front() {
            Some(message) => message,
            None => self.receiver.try_recv().ok()?,
        };
        match message {
            Message::Disk(result) => Some(result),
            other => {
                self.queue.push_front(other);
                None
            }
        }
    }

    /// Los guardados que ya llegaron, en orden; el resto sigue esperando.
    pub(crate) fn pending_saves(&mut self, wanted: impl Fn(&Save) -> bool) -> Vec<Save> {
        self.queue.extend(self.receiver.try_iter());
        let mut saves = Vec::new();
        let mut rest = VecDeque::with_capacity(self.queue.len());
        for message in self.queue.drain(..) {
            match message {
                Message::Save(save) if wanted(&save) => saves.push(save),
                other => rest.push_back(other),
            }
        }
        self.queue = rest;
        saves
    }
}
