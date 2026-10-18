//! `space f` en la ventana: abre el vault del finder si hace falta, le pasa
//! las notas al picker cada vez que el watcher trae otro índice y abre lo
//! elegido como `:e` o `:vsplit`.

use std::path::{Path, PathBuf};

use gpui_kit::{AppContext as _, Context, Entity, Subscription, Task, Window};
use nota_vim::{Command, Invocation};

use crate::workspace::Axis;

use super::WorkspaceView;
use super::overlay::Overlay;
use crate::finder::{Note, VaultSession};
use crate::picker::{Haystack, Open, PickerEvent, PickerOptions, PickerState, PickerView};
use crate::workspace::Message;

/// El vault sobre el que busca el finder. Hasta que nota abra vaults, es la
/// carpeta del archivo enfocado, y se abre la primera vez que se pide.
#[derive(Default)]
pub struct FinderVault {
    open: Option<OpenVault>,
    /// La carpeta que se está abriendo.
    opening: Option<(PathBuf, Task<()>)>,
    /// La última nota que se marcó como reciente.
    touched: Option<PathBuf>,
}

struct OpenVault {
    session: Entity<VaultSession>,
    _changes: Subscription,
}

impl FinderVault {
    fn session(&self) -> Option<&Entity<VaultSession>> {
        self.open.as_ref().map(|open| &open.session)
    }
}

fn options() -> PickerOptions {
    PickerOptions {
        haystack: Haystack::Paths,
        placeholder: "Buscar una nota…".into(),
        empty: "No hay notas".into(),
        splits: true,
    }
}

impl WorkspaceView {
    pub(super) fn open_finder(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.ensure_vault(window, cx) {
            return;
        }
        let mut options = options();
        let state = if let Some(session) = self.vault.session() {
            PickerState::new(session.read(cx).notes())
        } else {
            options.empty = "Indexando…".into();
            PickerState::loading()
        };
        let picker = cx.new(|cx| PickerView::new(state, options, cx));
        let events = cx.subscribe_in(&picker, window, Self::on_finder_event);
        self.show(
            Overlay::Finder {
                picker,
                _events: events,
            },
            window,
            cx,
        );
    }

    fn on_finder_event(
        &mut self,
        picker: &Entity<PickerView<Note>>,
        event: &PickerEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let chosen = match event {
            PickerEvent::Dismissed => None,
            PickerEvent::Chosen(chosen) => picker
                .read(cx)
                .items()
                .get(chosen.item)
                .map(|note| (note.clone(), chosen.open)),
        };
        self.close_overlay(window, cx);
        if let Some((note, open)) = chosen {
            self.open_note(&note, open, window, cx);
        }
    }

    /// Como `:e ruta`, o `:vsplit ruta` y `:split ruta` si va en split.
    fn open_note(&mut self, note: &Note, open: Open, window: &mut Window, cx: &mut Context<Self>) {
        let Some(session) = self.vault.session() else {
            return;
        };
        let path = shown_path(&session.read(cx).root().join(note.0.as_str()));
        let axis = match open {
            Open::Here => return self.edit_note(&path, window, cx),
            Open::SplitRight => Axis::Row,
            Open::SplitDown => Axis::Column,
        };
        let opened = self.workspace.update(cx, |workspace, cx| {
            cx.notify();
            workspace.open_in_split(axis, &path)
        });
        if let Err(error) = opened {
            self.workspace
                .update(cx, |workspace, _| workspace.report(&error));
        }
        self.sync_editors(cx);
        self.focus(window, cx);
        self.touch_focused(cx);
    }

    fn edit_note(&mut self, path: &Path, window: &mut Window, cx: &mut Context<Self>) {
        let Some(argument) = path.to_str().map(str::to_owned) else {
            let error = format!("{} no es una ruta UTF-8", path.display());
            self.workspace.update(cx, |workspace, _| {
                workspace.set_message(Message::Error(error));
            });
            return;
        };
        let edit = Invocation {
            command: Command::Edit,
            argument: Some(argument),
        };
        self.run(&edit, window, cx);
    }

    /// Que haya un vault para el archivo enfocado, abierto o abriéndose: si
    /// el de ahora no lo contiene, se suelta (así nada muestra sus notas) y
    /// se abre la carpeta del archivo en el fondo. `false` si no hay
    /// carpeta que abrir, con el aviso en la barra.
    fn ensure_vault(&mut self, window: &Window, cx: &mut Context<Self>) -> bool {
        let Some(folder) = self.focused_folder(cx) else {
            self.workspace.update(cx, |workspace, _| {
                let error = "no se encontró la carpeta del archivo para buscar notas";
                workspace.set_message(Message::Error(error.to_owned()));
            });
            return false;
        };
        let inside = |root: &Path| folder.starts_with(root);
        let open = self
            .vault
            .session()
            .map(|session| session.read(cx).root().to_owned());
        let opening = self.vault.opening.as_ref().map(|(root, _)| root.clone());
        if open.as_deref().is_some_and(inside) || opening.as_deref().is_some_and(inside) {
            return true;
        }
        self.vault.open = None;
        let task = VaultSession::open(folder.clone(), self.paths.clone(), cx);
        let finish = cx.spawn_in(window, async move |this, cx| {
            let session = task.await;
            let _ = this.update_in(cx, |this, window, cx| {
                this.vault_opened(session, window, cx);
            });
        });
        self.vault.opening = Some((folder, finish));
        true
    }

    fn vault_opened(
        &mut self,
        session: Result<Entity<VaultSession>, crate::finder::SessionError>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.vault.opening = None;
        let session = match session {
            Ok(session) => session,
            Err(error) => {
                log::error!("{error}");
                self.workspace.update(cx, |workspace, _| {
                    workspace.set_message(Message::Error(error.to_string()));
                });
                // El finder que esperaba las notas no las va a recibir; el aviso
                // queda a la vista en la barra.
                if matches!(self.overlay, Some(Overlay::Finder { .. })) {
                    self.close_overlay(window, cx);
                }
                return;
            }
        };
        let changes = cx.observe(&session, |this, _, cx| this.refresh_finder(cx));
        self.vault.open = Some(OpenVault {
            session,
            _changes: changes,
        });
        self.vault.touched = None;
        self.touch_focused(cx);
        self.refresh_finder(cx);
    }

    fn refresh_finder(&mut self, cx: &mut Context<Self>) {
        let Some(session) = self.vault.session() else {
            return;
        };
        let notes = session.read(cx).notes();
        if let Some(Overlay::Finder { picker, .. }) = &self.overlay {
            picker.update(cx, |picker, cx| picker.set_items(notes, cx));
        }
    }

    /// La nota enfocada pasa a ser la más reciente del vault, si está en él.
    pub(super) fn touch_focused(&mut self, cx: &mut Context<Self>) {
        let Some(session) = self.vault.session().cloned() else {
            return;
        };
        let Ok(document) = self.workspace.read(cx).focused_document() else {
            return;
        };
        let Some(path) = document.path().map(Path::to_owned) else {
            return;
        };
        if self.vault.touched.as_ref() == Some(&path) {
            return;
        }
        if let Some(note) = session.read(cx).vault_path(&path) {
            session.update(cx, |session, cx| session.touch(note, cx));
        }
        self.vault.touched = Some(path);
    }

    /// La carpeta del archivo enfocado, sin enlaces simbólicos.
    fn focused_folder(&self, cx: &Context<Self>) -> Option<PathBuf> {
        let document = self.workspace.read(cx).focused_document().ok()?;
        let parent = document
            .path()?
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty());
        std::fs::canonicalize(parent.unwrap_or(Path::new("."))).ok()
    }
}

/// La ruta como la escribiría quien usa vim, y como la muestran el título
/// y la barra: relativa al directorio de trabajo si está dentro.
fn shown_path(path: &Path) -> PathBuf {
    std::env::current_dir()
        .ok()
        .and_then(|cwd| path.strip_prefix(cwd).ok().map(Path::to_owned))
        .unwrap_or_else(|| path.to_owned())
}
