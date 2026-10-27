//! Los eventos de `notify` en lotes: lo que llega durante una ventana desde
//! el primero sale junto y tal cual. El vault relee del disco cada ruta que
//! nombra un lote, así que no necesita fusionar eventos; fusionarlos, como
//! `notify-debouncer-full`, perdía el origen de un rename (#43, #54).

use std::path::Path;
use std::sync::mpsc::{self, Receiver, RecvTimeoutError};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use notify::{Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher as _};

use crate::watcher::Watches;

/// Lo que llegó en una ventana, en el orden en que llegó.
#[derive(Debug, Default)]
pub(crate) struct Batch {
    pub events: Vec<Event>,
    pub errors: Vec<notify::Error>,
}

impl Batch {
    fn push(&mut self, raw: notify::Result<Event>) {
        match raw {
            Ok(event) => self.events.push(event),
            Err(error) => self.errors.push(error),
        }
    }
}

/// Un watcher de `notify` que entrega sus eventos en lotes a un hilo propio.
pub(crate) struct Debouncer {
    watcher: RecommendedWatcher,
    batches: JoinHandle<()>,
}

impl Debouncer {
    /// Empieza sin vigilar nada; `on_batch` recibe cada lote en el hilo del
    /// debouncer.
    pub(crate) fn new(
        window: Duration,
        on_batch: impl FnMut(Batch) + Send + 'static,
    ) -> notify::Result<Self> {
        let (raw, received) = mpsc::channel();
        let watcher = notify::recommended_watcher(move |event| {
            // Si el hilo de lotes ya terminó, el vault se está cerrando.
            let _ = raw.send(event);
        })?;
        let batches = thread::Builder::new()
            .name("metalmind-vault debounce".to_owned())
            .spawn(move || deliver(&received, window, on_batch))?;
        Ok(Self { watcher, batches })
    }
}

impl Watches for Debouncer {
    fn watch(&mut self, dir: &Path) -> notify::Result<()> {
        self.watcher.watch(dir, RecursiveMode::NonRecursive)
    }

    fn unwatch(&mut self, dir: &Path) -> notify::Result<()> {
        self.watcher.unwatch(dir)
    }

    fn stop(self) {
        // Sin el watcher se suelta el otro extremo del canal y el hilo sale.
        drop(self.watcher);
        if let Err(panic) = self.batches.join() {
            std::panic::resume_unwind(panic);
        }
    }
}

/// Junta lo que llega en `window` desde el primer evento que cuenta y se lo
/// da a `on_batch`, hasta que se cierre el canal; lo que quedaba sale igual.
fn deliver(
    received: &Receiver<notify::Result<Event>>,
    window: Duration,
    mut on_batch: impl FnMut(Batch),
) {
    while let Ok(first) = received.recv() {
        if is_read(&first) {
            continue;
        }
        let mut batch = Batch::default();
        batch.push(first);
        let deadline = Instant::now() + window;
        let open = loop {
            let left = deadline.saturating_duration_since(Instant::now());
            match received.recv_timeout(left) {
                Ok(raw) if is_read(&raw) => {}
                Ok(raw) => batch.push(raw),
                Err(RecvTimeoutError::Timeout) => break true,
                Err(RecvTimeoutError::Disconnected) => break false,
            }
        };
        on_batch(batch);
        if !open {
            return;
        }
    }
}

/// Abrir o leer no cambia qué hay en el vault, y el vault mismo lee al
/// releer: si eso armara un lote, uno llevaría al siguiente sin fin.
fn is_read(raw: &notify::Result<Event>) -> bool {
    matches!(raw, Ok(event) if matches!(event.kind, EventKind::Access(_)))
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use notify::event::{AccessKind, CreateKind};

    use super::*;

    fn created(path: &str) -> Event {
        Event::new(EventKind::Create(CreateKind::File)).add_path(PathBuf::from(path))
    }

    fn opened(path: &str) -> Event {
        Event::new(EventKind::Access(AccessKind::Any)).add_path(PathBuf::from(path))
    }

    /// Los lotes que salen de mandar `raw` y cerrar el canal.
    fn batches(raw: Vec<notify::Result<Event>>) -> Vec<Batch> {
        let (sender, received) = mpsc::channel();
        for event in raw {
            sender.send(event).unwrap();
        }
        drop(sender);
        let mut batches = Vec::new();
        deliver(&received, Duration::from_secs(60), |batch| {
            batches.push(batch);
        });
        batches
    }

    fn paths(batch: &Batch) -> Vec<&Path> {
        batch
            .events
            .iter()
            .flat_map(|event| &event.paths)
            .map(PathBuf::as_path)
            .collect()
    }

    #[test]
    fn lo_que_llega_en_la_ventana_sale_junto_y_en_orden() {
        let error = notify::Error::generic("cola llena");
        let [batch] = batches(vec![Ok(created("/v/b")), Err(error), Ok(created("/v/a"))])
            .try_into()
            .unwrap();
        assert_eq!(paths(&batch), [Path::new("/v/b"), Path::new("/v/a")]);
        assert_eq!(batch.errors.len(), 1);
    }

    #[test]
    fn leer_no_arma_un_lote_ni_entra_en_uno() {
        assert!(batches(vec![Ok(opened("/v")), Ok(opened("/v/a"))]).is_empty());
        let raw = vec![Ok(opened("/v")), Ok(created("/v/a")), Ok(opened("/v/a"))];
        let [batch] = batches(raw).try_into().unwrap();
        assert_eq!(paths(&batch), [Path::new("/v/a")]);
    }
}
