//! `space f` en la ventana: le pasa las notas del vault al picker cada vez
//! que el watcher trae otro índice y abre lo elegido como `:e` o `:vsplit`.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use gpui_kit::{AppContext as _, Context, Entity, Subscription, Window};
use metalmind_keymap::{Command, Invocation};

use super::WorkspaceView;
use super::overlay::Overlay;
use crate::finder::{Note, TagFilter, VaultChanged, VaultSession, WatchLost};
use crate::picker::{Fuzzy, Haystack, Open, PickerEvent, PickerOptions, PickerState, PickerView};
use crate::workspace::{Axis, Message, Workspace, shown_path};

/// El vault de la ventana visto por el finder y por el workspace, que
/// sigue los links y los archivos de la jumplist con él.
pub struct FinderVault {
    pub(super) session: Entity<VaultSession>,
    /// La última nota que se marcó como reciente.
    touched: Option<PathBuf>,
    _changes: Subscription,
    _files: Subscription,
    _lost: Subscription,
}

impl FinderVault {
    pub fn new(
        session: Entity<VaultSession>,
        workspace: &Entity<Workspace>,
        cx: &mut Context<WorkspaceView>,
    ) -> Self {
        let changes = cx.observe(&session, |view, _, cx| view.refresh_finder(cx));
        let files = cx.subscribe(&session, |view, session, VaultChanged(update), cx| {
            let root = session.read(cx).root().to_owned();
            view.workspace.update(cx, |workspace, _| {
                workspace.files_changed(&root, &update.changes);
                workspace.set_links(&root, Arc::clone(&update.links));
            });
        });
        let lost = cx.subscribe(&session, |view, _, WatchLost(reason), cx| {
            view.workspace.update(cx, |workspace, _| {
                workspace.set_message(Message::unwatched(*reason));
            });
            cx.notify();
        });
        let (root, links, unwatched) = {
            let session = session.read(cx);
            let unwatched = session.unwatched();
            (
                session.root().to_owned(),
                session.vault().links(),
                unwatched,
            )
        };
        workspace.update(cx, |workspace, _| {
            workspace.set_links(&root, links);
            if let Some(reason) = unwatched {
                workspace.set_message(Message::unwatched(reason));
            }
        });
        Self {
            session,
            touched: None,
            _changes: changes,
            _files: files,
            _lost: lost,
        }
    }
}

/// El picker de `space f`.
pub type Finder = PickerView<Fuzzy<Note>>;

fn options() -> PickerOptions {
    PickerOptions {
        placeholder: "Buscar una nota… (#tag, -#tag, -# sin tags)".into(),
        splits: true,
    }
}

impl WorkspaceView {
    pub(super) fn open_finder(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let session = self.vault.session.read(cx);
        let state = PickerState::new(session.notes())
            .with_narrowing(Arc::new(TagFilter::new(session.tags())));
        let fuzzy = Fuzzy::new(state, Haystack::Paths, "No hay notas".into());
        let picker = cx.new(|cx| PickerView::new(fuzzy, options(), cx));
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
        picker: &Entity<Finder>,
        event: &PickerEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let chosen = match event {
            PickerEvent::Dismissed => None,
            PickerEvent::Chosen(chosen) => picker
                .read(cx)
                .delegate()
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
        let root = self.vault.session.read(cx).root().to_owned();
        let path = shown_path(&root.join(note.0.as_str()));
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
            action: Command::Edit.into(),
            argument: Some(argument),
        };
        self.run(&edit, window, cx);
    }

    fn refresh_finder(&mut self, cx: &mut Context<Self>) {
        let session = self.vault.session.read(cx);
        let (notes, tags) = (session.notes(), session.tags());
        if let Some(Overlay::Finder { picker, .. }) = &self.overlay {
            picker.update(cx, |picker, cx| {
                picker.update_delegate(cx, |fuzzy, cx| {
                    fuzzy.set_narrowing(Arc::new(TagFilter::new(tags)), cx);
                    fuzzy.set_items(notes, cx);
                });
            });
        }
    }

    /// La nota enfocada pasa a ser la más reciente del vault, si está en él.
    pub(super) fn touch_focused(&mut self, cx: &mut Context<Self>) {
        let session = self.vault.session.clone();
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
}
