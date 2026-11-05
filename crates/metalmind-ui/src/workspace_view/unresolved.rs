//! `space u` en la ventana: el picker de los links sin destino del vault;
//! el elegido abre su nota con el cursor en el link, en el panel o en un
//! split, como `space /`.

use std::sync::Arc;
use std::time::Duration;

use gpui_kit::{AppContext as _, Context, Entity, Task, Window};
use metalmind_vault::LinkIndex;

use super::WorkspaceView;
use super::overlay::Overlay;
use crate::picker::{Fuzzy, Haystack, PickerEvent, PickerOptions, PickerState, PickerView};
use crate::unresolved::{UnresolvedLink, unresolved_links};

/// Cuánto espera la lista a que el watcher deje de traer lotes antes de
/// rehacerse: al guardar o al llegar varias notas, uno tras otro.
const REFRESH_DELAY: Duration = Duration::from_millis(150);

/// El picker de `space u`.
pub type UnresolvedPicker = PickerView<Fuzzy<UnresolvedLink>>;

/// El picker abierto con el índice del que salió su lista. `refresh` la rehace
/// para uno más nuevo; soltarlo la cancela.
pub struct UnresolvedLinks {
    picker: Entity<UnresolvedPicker>,
    index: Arc<LinkIndex>,
    refresh: Option<Task<()>>,
}

fn options() -> PickerOptions {
    PickerOptions {
        placeholder: "Links sin destino…".into(),
        splits: true,
        focus: None,
    }
}

impl WorkspaceView {
    /// La lista se arma fuera del hilo de la interfaz, como al rehacerse:
    /// con un vault grande, recorrer y ordenar todos los links trabaría la
    /// ventana. Mientras, el picker espera: lo que se escriba y se elija
    /// sale al llegar la lista.
    pub(super) fn open_unresolved_links(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let index = self.vault.session.read(cx).vault().links();
        let fuzzy = Fuzzy::new(
            PickerState::loading(),
            Haystack::Text,
            "Todos los links llevan a algo".into(),
        );
        let picker = cx.new(|cx| PickerView::new(fuzzy, options(), cx));
        let events = cx.subscribe_in(&picker, window, Self::on_unresolved_event);
        let refresh = Some(Self::build_unresolved_links(&index, None, cx));
        let mut overlay = Overlay::new(picker.clone(), vec![events], cx);
        overlay.unresolved = Some(UnresolvedLinks {
            picker,
            index,
            refresh,
        });
        self.show(overlay, window, cx);
    }

    fn open_unresolved(&self) -> Option<&UnresolvedLinks> {
        self.overlay.as_ref()?.unresolved.as_ref()
    }

    fn on_unresolved_event(
        &mut self,
        picker: &Entity<UnresolvedPicker>,
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
                .map(|link| (link.clone(), chosen.open)),
        };
        // Si llegó otro índice y la lista aún no se rehace, el link elegido
        // puede ya tener destino o no estar: entonces no se abre nada y el
        // picker sigue abierto hasta que llegue la lista nueva.
        if let Some((link, _)) = &chosen
            && let Some(open) = self.open_unresolved()
            && !link.is_unresolved_in(&open.index)
        {
            return;
        }
        self.close_overlay(window, cx);
        if let Some((link, open)) = chosen {
            self.open_note_at(&link.source, link.at, open, window, cx);
        }
    }

    /// Con el picker abierto, un lote del vault que trae otro índice de
    /// links (una nota nueva que resuelve un link, otra con links nuevos)
    /// cambia la lista. Se rehace fuera del hilo de la interfaz y tras
    /// [`REFRESH_DELAY`] sin lotes nuevos: con un vault grande, recorrer y
    /// ordenar todos los links en cada lote trabaría el picker.
    pub(super) fn refresh_unresolved_links(
        &mut self,
        latest: &Arc<LinkIndex>,
        cx: &mut Context<Self>,
    ) {
        let Some(open) = self.overlay.as_mut().and_then(|o| o.unresolved.as_mut()) else {
            return;
        };
        if Arc::ptr_eq(&open.index, latest) {
            return;
        }
        open.index = Arc::clone(latest);
        // Reemplazar la tarea cancela la del lote anterior, si no terminó.
        open.refresh = Some(Self::build_unresolved_links(
            latest,
            Some(REFRESH_DELAY),
            cx,
        ));
    }

    /// Arma la lista de `latest` en otro hilo, tras `delay` si se da, y la
    /// pone en el picker si sigue abierto para ese índice.
    fn build_unresolved_links(
        latest: &Arc<LinkIndex>,
        delay: Option<Duration>,
        cx: &mut Context<Self>,
    ) -> Task<()> {
        let latest = Arc::clone(latest);
        cx.spawn(async move |view, cx| {
            if let Some(delay) = delay {
                cx.background_executor().timer(delay).await;
            }
            let building = Arc::clone(&latest);
            let links = cx
                .background_executor()
                .spawn(async move { unresolved_links(&building) })
                .await;
            // Si la vista ya no existe, no hay lista que cambiar.
            let _ = view.update(cx, |view, cx| {
                view.show_unresolved_links(&latest, links, cx);
            });
        })
    }

    /// Pone en el picker la lista sacada de `built`, si sigue abierto y
    /// `built` es aún el índice más nuevo.
    fn show_unresolved_links(
        &mut self,
        built: &Arc<LinkIndex>,
        links: Arc<[UnresolvedLink]>,
        cx: &mut Context<Self>,
    ) {
        let Some(open) = self.open_unresolved() else {
            return;
        };
        if !Arc::ptr_eq(&open.index, built) {
            return;
        }
        open.picker.update(cx, |picker, cx| {
            picker.update_delegate(cx, |fuzzy, cx| fuzzy.set_items(links, cx));
        });
    }
}
