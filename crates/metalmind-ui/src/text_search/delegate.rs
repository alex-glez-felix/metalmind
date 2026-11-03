//! `space /` en el picker: cada patrón nuevo va al hilo de [`SearchWorker`],
//! que cancela la búsqueda anterior y lanza la nueva. En el hilo de la UI
//! solo se valida el patrón y se reciben los resultados, de a lotes. La
//! vista previa de la fila marcada se lee en el executor de fondo.

use std::collections::HashSet;
use std::path::Path;
use std::sync::Arc;

use gpui_kit::{Context, SharedString, Task};
use metalmind_vault::VaultPath;

use super::context::row_text;
use super::model::{MAX_HITS, Search, Status, TextSearchState};
use super::preview::PreviewState;
use super::worker::{Event, SearchWorker};
use super::{Hit, OpenNote};
use crate::picker::{
    Chosen, InputLine, LineEdit, Open, Overflow, PickerDelegate, PickerEvent, PickerRow,
    PickerView, Preview, RowCursor,
};

pub struct TextSearch {
    state: TextSearchState,
    /// Por qué no se pudo lanzar el hilo, si no se pudo.
    worker: Result<SearchWorker, String>,
    preview: PreviewState,
    /// La lectura de la última vista previa pedida: reemplazarla suelta la
    /// anterior, que si no empezó ya no corre.
    loading: Option<Task<()>>,
    _events: Task<()>,
}

impl TextSearch {
    /// Busca en las notas del vault de raíz `root`, o solo en las de
    /// `scope` si hay focus; las de `open` tienen cambios sin guardar y se
    /// buscan en memoria.
    pub fn new(
        root: &Path,
        open: Vec<OpenNote>,
        scope: Option<Arc<HashSet<VaultPath>>>,
        cx: &mut Context<PickerView<Self>>,
    ) -> Self {
        let (sender, events) = async_channel::unbounded();
        let preview = PreviewState::new(root.to_owned(), &open);
        let worker = SearchWorker::spawn(root.to_owned(), open, sender)
            .map_err(|error| format!("no se pudo lanzar el hilo de búsqueda: {error}"));
        let events = cx.spawn(async move |this, cx| {
            while let Ok(event) = events.recv().await {
                let mut batch = vec![event];
                batch.extend(std::iter::from_fn(|| events.try_recv().ok()));
                let handled = this.update(cx, |view, cx| {
                    view.update_delegate(cx, |search, cx| {
                        for event in batch {
                            search.handle(event, cx);
                        }
                    });
                });
                // Si el picker ya se cerró, no hay nada que mostrar.
                if handled.is_err() {
                    return;
                }
            }
        });
        let state = TextSearchState::new();
        Self {
            state: match scope {
                Some(scope) => state.with_scope(scope),
                None => state,
            },
            worker,
            preview,
            loading: None,
            _events: events,
        }
    }

    #[must_use]
    pub fn hit(&self, item: usize) -> Option<&Hit> {
        self.state.hit(item)
    }

    /// Las notas del focus de otro lote del watcher: lo escrito se vuelve a
    /// buscar si cambiaron.
    pub fn set_scope(&mut self, scope: Option<Arc<HashSet<VaultPath>>>) {
        if let Some(search) = self.state.set_scope(scope) {
            self.launch(search);
        }
    }

    fn launch(&mut self, search: Search) {
        self.preview.searched_again();
        match &self.worker {
            Ok(worker) => worker.search(search),
            Err(error) => self.state.failed(search.version, error.clone()),
        }
    }

    fn handle(&mut self, event: Event, cx: &mut Context<PickerView<Self>>) {
        match event {
            Event::Found { version, notes } => {
                if let Some(chosen) = self.state.receive(version, notes) {
                    cx.emit(PickerEvent::Chosen(chosen));
                }
            }
            Event::Finished { version } => self.state.finished(version),
            Event::Failed { version, error } => self.state.failed(version, error),
        }
    }
}

impl PickerDelegate for TextSearch {
    fn line(&self) -> &InputLine {
        self.state.line()
    }

    fn cursor(&self) -> RowCursor {
        self.state.cursor()
    }

    fn select(&mut self, row: usize) {
        self.state.select(row);
    }

    fn edit(&mut self, edit: LineEdit<'_>, _cx: &mut Context<PickerView<Self>>) {
        if let Some(search) = self.state.edit(edit) {
            self.launch(search);
        }
    }

    fn choose(&mut self, open: Open, _cx: &mut Context<PickerView<Self>>) -> Option<Chosen> {
        self.state.choose(open)
    }

    fn row(&mut self, row: usize) -> PickerRow {
        let Some(hit) = self.state.hit(row) else {
            return PickerRow::default();
        };
        let shown = row_text(&hit.line);
        PickerRow {
            text: shown.text,
            highlights: shown.highlights,
            overflow: Overflow::Around,
            detail: Some(format!("{}:{}", hit.path, hit.line.line + 1)),
        }
    }

    fn empty(&self) -> SharedString {
        match self.state.status() {
            Status::Idle => "Escribe para buscar en las notas".into(),
            Status::Searching => "Buscando…".into(),
            Status::Done | Status::Truncated => "Sin resultados".into(),
            Status::Invalid(error) => format!("Patrón inválido: {}", reason(&error)).into(),
            Status::Failed(error) => error.into(),
        }
    }

    fn has_preview(&self) -> bool {
        true
    }

    fn preview(&self) -> Option<&Preview> {
        self.preview.shown()
    }

    fn selection_changed(&mut self, cx: &mut Context<PickerView<Self>>) {
        let hit = self.state.hit(self.state.selected());
        let Some(job) = self.preview.request(hit) else {
            return;
        };
        let loading = cx.background_executor().spawn(async move { job.run() });
        let task = cx.spawn(async move |this, cx| {
            let loaded = loading.await;
            // Si el picker ya se cerró, no hay nada que mostrar.
            let _ = this.update(cx, |view, cx| {
                view.update_delegate(cx, |search, _| search.preview.finish(loaded));
            });
        });
        drop(self.loading.replace(task));
    }

    fn status(&self) -> Option<SharedString> {
        let count = self.state.len();
        match self.state.status() {
            Status::Idle => None,
            // Lo que se ve es de la búsqueda anterior: su cuenta no vale.
            Status::Searching if self.state.is_stale() => Some("…".into()),
            Status::Searching => Some(format!("{count}…").into()),
            Status::Done if count == 0 => None,
            Status::Done if count == 1 => Some("1 resultado".into()),
            Status::Done => Some(format!("{count} resultados").into()),
            Status::Truncated => Some(format!("los primeros {MAX_HITS}").into()),
            Status::Invalid(_) => Some("patrón inválido".into()),
            Status::Failed(_) => Some("error".into()),
        }
    }
}

/// La última línea del error de `regex`, la que dice qué está mal: las de
/// antes repiten el patrón y lo señalan con `^`.
fn reason(error: &str) -> &str {
    let last = error.lines().last().unwrap_or(error);
    last.strip_prefix("error: ").unwrap_or(last)
}

#[cfg(test)]
#[path = "delegate_tests.rs"]
mod tests;
