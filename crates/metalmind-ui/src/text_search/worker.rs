//! El hilo de `space /`: cancela la búsqueda anterior y lanza la nueva, así
//! la UI nunca espera a que terminen los hilos de la búsqueda vieja. Si
//! llegan varias teclas mientras cancela, salta a la última. Las notas
//! abiertas con cambios se buscan aquí, en memoria; su versión en disco no
//! cuenta.

use std::collections::HashSet;
use std::io;
use std::path::PathBuf;
use std::sync::{Arc, mpsc};
use std::thread;

use metalmind_search::{NoteMatches, TextSearcher};
use metalmind_vault::VaultPath;

use super::OpenNote;
use super::model::Search;

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

enum Request {
    Search(Search),
    Cancel,
}

pub struct SearchWorker {
    requests: mpsc::Sender<Request>,
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
        let worker = Worker {
            searcher: TextSearcher::new(root),
            skip: Arc::new(open.iter().map(|note| note.path.clone()).collect()),
            open,
            events,
        };
        thread::Builder::new()
            .name("metalmind-ui búsqueda".to_owned())
            .spawn(move || worker.run(&received))?;
        Ok(Self { requests })
    }

    pub fn search(&self, search: Search) {
        self.send(Request::Search(search));
    }

    /// Corta la búsqueda en curso, como cuando ya hay demasiados resultados.
    pub fn cancel(&self) {
        self.send(Request::Cancel);
    }

    fn send(&self, request: Request) {
        if self.requests.send(request).is_err() {
            log::error!("el hilo de la búsqueda de texto ya no está");
        }
    }
}

struct Worker {
    searcher: TextSearcher,
    open: Vec<OpenNote>,
    skip: Arc<HashSet<VaultPath>>,
    events: async_channel::Sender<Event>,
}

impl Worker {
    fn run(mut self, requests: &mpsc::Receiver<Request>) {
        while let Ok(mut request) = requests.recv() {
            while let Ok(newer) = requests.try_recv() {
                request = newer;
            }
            match request {
                Request::Search(search) if !search.query.pattern.is_empty() => {
                    self.start(&search);
                }
                Request::Search(_) | Request::Cancel => self.searcher.cancel(),
            }
        }
    }

    fn start(&mut self, search: &Search) {
        let version = search.version;
        // Antes de buscar en memoria: así la búsqueda vieja no sigue corriendo.
        self.searcher.cancel();
        let found = self.search_open_notes(search);
        if !found.is_empty()
            && !self.send(Event::Found {
                version,
                notes: found,
            })
        {
            return;
        }
        let result = self
            .searcher
            .search(&search.query)
            .map_err(|error| error.to_string())
            .and_then(|found| {
                forward(found, version, Arc::clone(&self.skip), self.events.clone())
                    .map_err(|error| format!("no se pudo lanzar el hilo de búsqueda: {error}"))
            });
        if let Err(error) = result {
            self.send(Event::Failed { version, error });
        }
    }

    fn search_open_notes(&self, search: &Search) -> Vec<NoteMatches> {
        let matcher = match search.query.compile() {
            Ok(matcher) => matcher,
            Err(error) => {
                log::warn!("la búsqueda de texto no compila: {error}");
                return Vec::new();
            }
        };
        let mut found = Vec::new();
        for note in &self.open {
            match matcher.find(&note.text) {
                Ok(lines) if !lines.is_empty() => found.push(NoteMatches {
                    path: note.path.clone(),
                    lines,
                }),
                Ok(_) => {}
                Err(error) => log::warn!("al buscar en {}: {error}", note.path),
            }
        }
        found
    }

    /// Si la UI sigue escuchando.
    fn send(&self, event: Event) -> bool {
        self.events.send_blocking(event).is_ok()
    }
}

/// Pasa las notas de una búsqueda a la UI, juntando en un lote las que ya
/// llegaron y sin las que están en `skip`. Termina con la búsqueda, o cuando
/// la UI deja de escuchar, y lo avisa con [`Event::Finished`].
fn forward(
    found: mpsc::Receiver<NoteMatches>,
    version: u64,
    skip: Arc<HashSet<VaultPath>>,
    events: async_channel::Sender<Event>,
) -> io::Result<()> {
    thread::Builder::new()
        .name("metalmind-ui resultados".to_owned())
        .spawn(move || {
            while let Ok(first) = found.recv() {
                let notes: Vec<NoteMatches> = std::iter::once(first)
                    .chain(found.try_iter())
                    .filter(|note| !skip.contains(&note.path))
                    .collect();
                if !notes.is_empty()
                    && events
                        .send_blocking(Event::Found { version, notes })
                        .is_err()
                {
                    return;
                }
            }
            // Si la UI ya no escucha, no hay a quién avisar.
            let _ = events.send_blocking(Event::Finished { version });
        })
        .map(drop)
}

#[cfg(test)]
mod tests {
    use std::fs;

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

    #[test]
    fn busca_en_el_disco_y_avisa_al_terminar() {
        let dir = vault();
        let (sender, events) = async_channel::unbounded();
        let worker = SearchWorker::spawn(dir.path().to_owned(), Vec::new(), sender).unwrap();
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
        let open = vec![OpenNote {
            path: VaultPath::new("b.md").unwrap(),
            text: "nada\nnada\nuna idea sin guardar".to_owned(),
        }];
        let (sender, events) = async_channel::unbounded();
        let worker = SearchWorker::spawn(dir.path().to_owned(), open, sender).unwrap();
        worker.search(search(1, "idea"));
        assert_eq!(found(&events, 1), ["a.md:1", "b.md:3"]);
    }

    #[test]
    fn un_patron_vacio_o_cancelar_no_buscan() {
        let dir = vault();
        let (sender, events) = async_channel::unbounded();
        let worker = SearchWorker::spawn(dir.path().to_owned(), Vec::new(), sender).unwrap();
        worker.search(search(1, ""));
        worker.cancel();
        worker.search(search(2, "idea"));
        assert_eq!(found(&events, 2), ["a.md:1", "b.md:2"]);
        assert!(events.try_recv().is_err(), "de la 1 no llegó nada");
    }

    #[test]
    fn soltar_el_worker_termina_el_hilo() {
        let dir = vault();
        let (sender, events) = async_channel::unbounded();
        let worker = SearchWorker::spawn(dir.path().to_owned(), Vec::new(), sender).unwrap();
        worker.search(search(1, "idea"));
        drop(worker);
        // Cuando el hilo termina suelta su extremo y el canal se cierra.
        while events.recv_blocking().is_ok() {}
    }
}
