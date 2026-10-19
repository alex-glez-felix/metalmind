//! El hilo de `space /`: cancela la búsqueda anterior y lanza la nueva, así
//! la UI nunca espera a que terminen los hilos de la búsqueda vieja. Si
//! llegan varias teclas mientras cancela, salta a la última. Las notas
//! abiertas con cambios se buscan aquí, en memoria; su versión en disco no
//! cuenta.
//!
//! La versión de la última tecla es compartida: en cuanto cambia, lo que
//! corre para una versión vieja (buscar en las notas abiertas, pasar notas a
//! la UI) se corta sin esperar a que el hilo la lea.

use std::collections::HashSet;
use std::io;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, mpsc};
use std::thread;

use metalmind_search::{NoteMatches, TextSearcher};
use metalmind_vault::VaultPath;

use super::OpenNote;
use super::model::{MAX_HITS, Search};

/// Lo que el hilo le cuenta a la UI, con la versión de su búsqueda.
#[derive(Debug, PartialEq, Eq)]
pub enum Event {
    Found {
        version: u64,
        notes: Vec<NoteMatches>,
    },
    /// Terminó o se canceló: no llega nada más de esa versión.
    Finished {
        version: u64,
    },
    Failed {
        version: u64,
        error: String,
    },
}

pub struct SearchWorker {
    requests: mpsc::Sender<Search>,
    /// La versión de la última búsqueda pedida.
    latest: Arc<AtomicU64>,
}

impl SearchWorker {
    /// Busca en el vault de raíz `root`. Los eventos salen por `events`; el
    /// hilo termina al soltar el worker, y cancela lo que corría.
    ///
    /// # Errors
    /// Si no se puede lanzar el hilo.
    pub fn spawn(
        root: PathBuf,
        open: Vec<OpenNote>,
        events: async_channel::Sender<Event>,
    ) -> io::Result<Self> {
        let (requests, received) = mpsc::channel();
        let latest = Arc::new(AtomicU64::new(0));
        let shared = Arc::clone(&latest);
        thread::Builder::new()
            .name("metalmind-ui búsqueda".to_owned())
            .spawn(move || Worker::new(root, open, shared, events).run(&received))?;
        Ok(Self { requests, latest })
    }

    /// Lanza `search` en lugar de la que corra; un patrón vacío solo la
    /// cancela.
    pub fn search(&self, search: Search) {
        self.latest.store(search.version, Ordering::Relaxed);
        if self.requests.send(search).is_err() {
            log::error!("el hilo de la búsqueda de texto ya no está");
        }
    }
}

struct Worker {
    searcher: TextSearcher,
    /// Las notas abiertas con cambios, con su texto.
    open: Vec<(VaultPath, String)>,
    skip: Arc<HashSet<VaultPath>>,
    latest: Arc<AtomicU64>,
    events: async_channel::Sender<Event>,
}

impl Worker {
    /// Aplana el texto de las notas abiertas aquí y no en el hilo de la UI.
    fn new(
        root: PathBuf,
        open: Vec<OpenNote>,
        latest: Arc<AtomicU64>,
        events: async_channel::Sender<Event>,
    ) -> Self {
        let open: Vec<(VaultPath, String)> = open
            .into_iter()
            .map(|note| (note.path, note.text.to_string()))
            .collect();
        Self {
            searcher: TextSearcher::new(root),
            skip: Arc::new(open.iter().map(|(path, _)| path.clone()).collect()),
            open,
            latest,
            events,
        }
    }

    fn run(mut self, requests: &mpsc::Receiver<Search>) {
        while let Ok(mut search) = requests.recv() {
            while let Ok(newer) = requests.try_recv() {
                search = newer;
            }
            if search.query.pattern.is_empty() {
                self.searcher.cancel();
            } else {
                self.start(&search);
            }
        }
    }

    fn start(&mut self, search: &Search) {
        let version = search.version;
        // Antes de buscar en memoria: así la búsqueda vieja no sigue corriendo.
        self.searcher.cancel();
        let Some(found) = self.search_open_notes(search) else {
            return;
        };
        if !found.is_empty()
            && !self.send(Event::Found {
                version,
                notes: found,
            })
        {
            return;
        }
        let forwarder = Forwarder {
            version,
            skip: Arc::clone(&self.skip),
            latest: Arc::clone(&self.latest),
            events: self.events.clone(),
        };
        let result = self
            .searcher
            .search(&search.query)
            .map_err(|error| error.to_string())
            .and_then(|found| {
                forwarder
                    .spawn(found)
                    .map_err(|error| format!("no se pudo lanzar el hilo de búsqueda: {error}"))
            });
        if let Err(error) = result {
            self.send(Event::Failed { version, error });
        }
    }

    /// Lo que coincide en las notas abiertas; `None` si llegó otra tecla
    /// mientras tanto.
    fn search_open_notes(&self, search: &Search) -> Option<Vec<NoteMatches>> {
        let matcher = match search.query.compile() {
            Ok(matcher) => matcher,
            Err(error) => {
                log::warn!("la búsqueda de texto no compila: {error}");
                return Some(Vec::new());
            }
        };
        let mut found = Vec::new();
        for (path, text) in &self.open {
            if self.latest.load(Ordering::Relaxed) != search.version {
                return None;
            }
            match matcher.find(text) {
                Ok(lines) if !lines.is_empty() => found.push(NoteMatches {
                    path: path.clone(),
                    lines,
                }),
                Ok(_) => {}
                Err(error) => log::warn!("al buscar en {path}: {error}"),
            }
        }
        Some(found)
    }

    /// Si la UI sigue escuchando.
    fn send(&self, event: Event) -> bool {
        self.events.send_blocking(event).is_ok()
    }
}

/// Pasa las notas de una búsqueda a la UI, juntando en un lote las que ya
/// llegaron y sin las que están en `skip`. Pasa a lo sumo una línea más que
/// [`MAX_HITS`], para que la UI sepa que hubo más. Termina con la búsqueda,
/// cuando hay otra más nueva o cuando la UI deja de escuchar, y lo avisa con
/// [`Event::Finished`]. Al terminar suelta el receptor, y eso corta la
/// búsqueda si seguía.
struct Forwarder {
    version: u64,
    skip: Arc<HashSet<VaultPath>>,
    latest: Arc<AtomicU64>,
    events: async_channel::Sender<Event>,
}

impl Forwarder {
    fn spawn(self, found: mpsc::Receiver<NoteMatches>) -> io::Result<()> {
        thread::Builder::new()
            .name("metalmind-ui resultados".to_owned())
            .spawn(move || self.run(&found))
            .map(drop)
    }

    fn run(&self, found: &mpsc::Receiver<NoteMatches>) {
        let mut room = MAX_HITS + 1;
        while let Ok(first) = found.recv() {
            if self.latest.load(Ordering::Relaxed) != self.version {
                break;
            }
            let mut notes = Vec::new();
            for mut note in std::iter::once(first).chain(found.try_iter()) {
                if room == 0 {
                    break;
                }
                if self.skip.contains(&note.path) {
                    continue;
                }
                note.lines.truncate(room);
                room -= note.lines.len();
                notes.push(note);
            }
            let event = Event::Found {
                version: self.version,
                notes,
            };
            if self.events.send_blocking(event).is_err() || room == 0 {
                break;
            }
        }
        // Si la UI ya no escucha, no hay a quién avisar.
        let _ = self.events.send_blocking(Event::Finished {
            version: self.version,
        });
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use metalmind_buffer::Buffer;
    use metalmind_search::TextQuery;
    use pretty_assertions::assert_eq;

    use super::*;

    fn search(version: u64, pattern: &str) -> Search {
        Search {
            version,
            query: TextQuery {
                pattern: pattern.to_owned(),
                ..TextQuery::default()
            },
        }
    }

    /// Los eventos de `version` hasta su `Finished`, como `ruta:línea`.
    fn found(events: &async_channel::Receiver<Event>, version: u64) -> Vec<String> {
        let mut lines = Vec::new();
        loop {
            match events.recv_blocking().unwrap() {
                Event::Found { version: v, notes } if v == version => {
                    for note in notes {
                        lines.extend(
                            note.lines
                                .iter()
                                .map(|line| format!("{}:{}", note.path, line.line + 1)),
                        );
                    }
                }
                Event::Finished { version: v } if v == version => break,
                Event::Failed { error, .. } => panic!("{error}"),
                _ => {}
            }
        }
        lines.sort();
        lines
    }

    fn vault() -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("a.md"), "una idea\n").unwrap();
        fs::write(dir.path().join("b.md"), "nada\nidea\n").unwrap();
        dir
    }

    fn spawn(
        dir: &tempfile::TempDir,
        open: Vec<OpenNote>,
    ) -> (SearchWorker, async_channel::Receiver<Event>) {
        let (sender, events) = async_channel::unbounded();
        let worker = SearchWorker::spawn(dir.path().to_owned(), open, sender).unwrap();
        (worker, events)
    }

    #[test]
    fn busca_en_el_disco_y_avisa_al_terminar() {
        let dir = vault();
        let (worker, events) = spawn(&dir, Vec::new());
        worker.search(search(1, "idea"));
        assert_eq!(found(&events, 1), ["a.md:1", "b.md:2"]);
        worker.search(search(2, "zzz"));
        assert_eq!(found(&events, 2), [""; 0]);
    }

    /// Lo que se ve de una nota con cambios sin guardar es el buffer, no el
    /// disco.
    #[test]
    fn las_notas_con_cambios_se_buscan_en_memoria() {
        let dir = vault();
        let buffer = Buffer::from("nada\nnada\nuna idea sin guardar");
        let open = vec![OpenNote {
            path: VaultPath::new("b.md").unwrap(),
            text: buffer.snapshot(),
        }];
        let (worker, events) = spawn(&dir, open);
        worker.search(search(1, "idea"));
        assert_eq!(found(&events, 1), ["a.md:1", "b.md:3"]);
    }

    #[test]
    fn un_patron_vacio_no_busca() {
        let dir = vault();
        let (worker, events) = spawn(&dir, Vec::new());
        worker.search(search(1, ""));
        worker.search(search(2, "idea"));
        assert_eq!(found(&events, 2), ["a.md:1", "b.md:2"]);
        assert!(events.try_recv().is_err(), "de la 1 no llegó nada");
    }

    /// Pasa una línea más que el tope, para que la UI sepa que se cortó, y
    /// ninguna más aunque la búsqueda encuentre miles.
    #[test]
    fn no_pasa_mas_lineas_que_el_tope() {
        let dir = tempfile::tempdir().unwrap();
        let note = "idea\n".repeat(MAX_HITS / 2);
        for name in ["a.md", "b.md", "c.md"] {
            fs::write(dir.path().join(name), &note).unwrap();
        }
        let (worker, events) = spawn(&dir, Vec::new());
        worker.search(search(1, "idea"));
        assert_eq!(found(&events, 1).len(), MAX_HITS + 1);
    }

    /// Lo que llega de una búsqueda vieja después de la nueva no pasa.
    #[test]
    fn una_busqueda_reemplazada_deja_de_pasar_notas() {
        let forward = |latest: u64| {
            let (sender, events) = async_channel::unbounded();
            let forwarder = Forwarder {
                version: 1,
                skip: Arc::default(),
                latest: Arc::new(AtomicU64::new(latest)),
                events: sender,
            };
            let (found, receiver) = mpsc::channel();
            let note = NoteMatches {
                path: VaultPath::new("a.md").unwrap(),
                lines: Vec::new(),
            };
            found.send(note).unwrap();
            drop(found);
            forwarder.run(&receiver);
            events
        };
        let current = forward(1);
        assert!(matches!(current.try_recv(), Ok(Event::Found { .. })));
        let replaced = forward(2);
        assert_eq!(
            replaced.try_recv().unwrap(),
            Event::Finished { version: 1 },
            "sin pasar nada"
        );
    }

    #[test]
    fn soltar_el_worker_termina_el_hilo() {
        let dir = vault();
        let (worker, events) = spawn(&dir, Vec::new());
        worker.search(search(1, "idea"));
        drop(worker);
        // Cuando el hilo termina suelta su extremo y el canal se cierra.
        while events.recv_blocking().is_ok() {}
    }
}
