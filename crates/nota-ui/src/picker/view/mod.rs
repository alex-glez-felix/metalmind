//! La vista de un picker: pinta [`PickerState`], le pasa teclas, texto y
//! ratón, y corre cada búsqueda en el executor de fondo de GPUI. Lo que se
//! elige sale como [`PickerEvent`]; qué hacer con eso lo decide quien lo
//! abrió.

mod input;
mod render;

use std::sync::Arc;

use gpui_kit::{
    AppContext as _, Context, EventEmitter, FocusHandle, Focusable, KeyContext, ScrollStrategy,
    SharedString, Task, UniformListScrollHandle, Window,
};
use nota_search::FuzzyMatcher;

use super::actions::{
    Confirm, ConfirmSplitDown, ConfirmSplitRight, DeleteBackward, DeleteForward, DeleteToStart,
    DeleteWordBackward, Dismiss, MoveLeft, MoveRight, MoveToEnd, MoveToStart, PICKER_CONTEXT,
    SPLITS_CONTEXT, SelectNext, SelectPrevious,
};
use super::{Chosen, LineEdit, Matches, Open, PickerState, Step};

/// Lo que se puede elegir en un picker.
pub trait PickerItem: AsRef<str> + Send + Sync + 'static {
    /// Lo que va a la derecha de la fila, como el atajo de una acción.
    fn detail(&self) -> Option<&str> {
        None
    }
}

/// Qué clase de texto se busca: cambia los bonus del fuzzy.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Haystack {
    Paths,
    Text,
}

impl Haystack {
    fn matcher(self) -> FuzzyMatcher {
        match self {
            Self::Paths => FuzzyMatcher::paths(),
            Self::Text => FuzzyMatcher::text(),
        }
    }
}

/// Cómo se ve y se comporta un picker.
#[derive(Clone, Debug)]
pub struct PickerOptions {
    pub haystack: Haystack,
    /// Lo que dice el input vacío.
    pub placeholder: SharedString,
    /// Lo que dice la lista si no hay ningún ítem, como mientras se indexa.
    pub empty: SharedString,
    /// Si `ctrl-v` y `ctrl-s` eligen abriendo en un split.
    pub splits: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PickerEvent {
    Chosen(Chosen),
    Dismissed,
}

pub struct PickerView<T> {
    state: PickerState<T>,
    options: PickerOptions,
    focus_handle: FocusHandle,
    scroll: UniformListScrollHandle,
    /// Se presta a la búsqueda en curso y vuelve con sus resultados.
    search_matcher: Option<FuzzyMatcher>,
    /// Para las posiciones de las filas que se pintan.
    paint_matcher: FuzzyMatcher,
    /// Preedit del IME o de una tecla muerta.
    marked: Option<String>,
    search: Option<Task<()>>,
}

impl<T: 'static> EventEmitter<PickerEvent> for PickerView<T> {}

impl<T: 'static> Focusable for PickerView<T> {
    fn focus_handle(&self, _cx: &gpui_kit::App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl<T: PickerItem> PickerView<T> {
    pub fn new(state: PickerState<T>, options: PickerOptions, cx: &mut Context<Self>) -> Self {
        Self {
            state,
            focus_handle: cx.focus_handle(),
            scroll: UniformListScrollHandle::new(),
            search_matcher: Some(options.haystack.matcher()),
            paint_matcher: options.haystack.matcher(),
            options,
            marked: None,
            search: None,
        }
    }

    #[must_use]
    pub fn items(&self) -> &Arc<[T]> {
        self.state.items()
    }

    /// Cambia los ítems y vuelve a filtrar con lo escrito.
    pub fn set_items(&mut self, items: Arc<[T]>, cx: &mut Context<Self>) {
        let step = self.state.set_items(items);
        self.step(step, cx);
    }

    fn edit(&mut self, edit: LineEdit<'_>, cx: &mut Context<Self>) {
        let step = self.state.edit(edit);
        self.step(step, cx);
    }

    /// Corre la búsqueda que pidió el modelo y avisa si se eligió algo.
    fn step(&mut self, step: Step<T>, cx: &mut Context<Self>) {
        if let Some(job) = step.job {
            let haystack = self.options.haystack;
            let mut matcher = self
                .search_matcher
                .take()
                .unwrap_or_else(|| haystack.matcher());
            let running = cx.background_spawn(async move {
                let found = job.run(&mut matcher);
                (found, matcher)
            });
            self.search = Some(cx.spawn(async move |this, cx| {
                let (found, matcher) = running.await;
                // Si el picker ya se cerró, no hay nada que mostrar.
                let _ = this.update(cx, |this, cx| this.finish(found, matcher, cx));
            }));
        }
        if let Some(chosen) = step.chosen {
            cx.emit(PickerEvent::Chosen(chosen));
        }
        self.reveal_selected();
        cx.notify();
    }

    fn finish(&mut self, found: Matches, matcher: FuzzyMatcher, cx: &mut Context<Self>) {
        self.search_matcher = Some(matcher);
        let step = self.state.finish(found);
        self.step(step, cx);
    }

    fn reveal_selected(&self) {
        self.scroll
            .scroll_to_item(self.state.selected(), ScrollStrategy::Nearest);
    }

    fn select_next(&mut self, _: &SelectNext, _: &mut Window, cx: &mut Context<Self>) {
        self.state.select_next();
        self.reveal_selected();
        cx.notify();
    }

    fn select_previous(&mut self, _: &SelectPrevious, _: &mut Window, cx: &mut Context<Self>) {
        self.state.select_previous();
        self.reveal_selected();
        cx.notify();
    }

    fn choose(&mut self, open: Open, cx: &mut Context<Self>) {
        if let Some(chosen) = self.state.choose(open) {
            cx.emit(PickerEvent::Chosen(chosen));
        }
    }

    fn confirm(&mut self, _: &Confirm, _: &mut Window, cx: &mut Context<Self>) {
        self.choose(Open::Here, cx);
    }

    fn confirm_split_right(
        &mut self,
        _: &ConfirmSplitRight,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.choose(Open::SplitRight, cx);
    }

    fn confirm_split_down(&mut self, _: &ConfirmSplitDown, _: &mut Window, cx: &mut Context<Self>) {
        self.choose(Open::SplitDown, cx);
    }

    fn dismiss(_: &mut Self, _: &Dismiss, _: &mut Window, cx: &mut Context<Self>) {
        cx.emit(PickerEvent::Dismissed);
    }

    fn click(&mut self, row: usize, cx: &mut Context<Self>) {
        self.state.select(row);
        self.choose(Open::Here, cx);
    }

    fn key_context(&self) -> KeyContext {
        let mut context = KeyContext::new_with_defaults();
        context.add(PICKER_CONTEXT);
        if self.options.splits {
            context.add(SPLITS_CONTEXT);
        }
        context
    }
}

/// Una acción `picker::` que solo edita la línea.
macro_rules! line_edits {
    ($($method:ident: $action:ident => $edit:expr;)*) => {
        impl<T: PickerItem> PickerView<T> {
            $(
                fn $method(&mut self, _: &$action, _: &mut Window, cx: &mut Context<Self>) {
                    self.edit($edit, cx);
                }
            )*
        }
    };
}

line_edits! {
    delete_backward: DeleteBackward => LineEdit::DeleteBackward;
    delete_forward: DeleteForward => LineEdit::DeleteForward;
    delete_word_backward: DeleteWordBackward => LineEdit::DeleteWordBackward;
    delete_to_start: DeleteToStart => LineEdit::DeleteToStart;
    move_left: MoveLeft => LineEdit::Left;
    move_right: MoveRight => LineEdit::Right;
    move_to_start: MoveToStart => LineEdit::Home;
    move_to_end: MoveToEnd => LineEdit::End;
}
