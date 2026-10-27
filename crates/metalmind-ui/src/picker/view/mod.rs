//! La vista de un picker: el input, la lista virtual y las acciones
//! `picker::`, iguales para cualquier picker. Qué se lista y cómo se busca
//! lo decide su [`PickerDelegate`], como en el `Picker` de Zed. Lo que se
//! elige sale como [`PickerEvent`]; qué hacer con eso lo decide quien lo
//! abrió.

mod input;
mod render;
mod row_line;

use std::ops::Range;

use gpui_kit::{
    Context, EventEmitter, FocusHandle, Focusable, KeyContext, ScrollHandle, ScrollStrategy,
    SharedString, UniformListScrollHandle, Window,
};

use metalmind_keymap::{Action, PickerAction, context};

use super::{Chosen, InputLine, LineEdit, Open};
use crate::actions::Dispatch;

/// Lo que se lista y cómo se busca. Sus métodos que reciben el contexto de
/// la vista pueden lanzar trabajo de fondo y avisar al volver con
/// [`PickerView::update_delegate`].
pub trait PickerDelegate: Sized + 'static {
    fn line(&self) -> &InputLine;

    /// Los bytes de lo escrito que se marcan como aviso, como un tag que no
    /// existe.
    fn warnings(&self) -> Vec<Range<usize>> {
        Vec::new()
    }

    /// Cuántas filas tiene la lista.
    fn len(&self) -> usize;

    fn selected(&self) -> usize;

    /// Marca la fila `row`, si existe.
    fn select(&mut self, row: usize);

    /// Baja una fila; de la última vuelve a la primera.
    fn select_next(&mut self);

    /// Sube una fila; de la primera va a la última.
    fn select_previous(&mut self);

    /// Edita lo escrito y lanza la búsqueda que haga falta.
    fn edit(&mut self, edit: LineEdit<'_>, cx: &mut Context<PickerView<Self>>);

    /// Elige la fila marcada; `None` si no hay o si la elección espera a
    /// resultados que el delegado avisará con [`PickerEvent::Chosen`].
    fn choose(&mut self, open: Open, cx: &mut Context<PickerView<Self>>) -> Option<Chosen>;

    /// Lo que se pinta en la fila `row`. Se pide solo para las visibles.
    fn row(&mut self, row: usize) -> PickerRow;

    /// Lo que dice la lista vacía.
    fn empty(&self) -> SharedString;

    /// Lo que va a la derecha del input, como cuántos resultados hay.
    fn status(&self) -> Option<SharedString> {
        None
    }
}

/// Una fila de la lista. Siempre ocupa una línea: lo que no cabe se
/// recorta con elipsis.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PickerRow {
    pub text: String,
    /// Los bytes de `text` que coinciden, crecientes y sin solaparse.
    pub highlights: Vec<Range<usize>>,
    /// Cómo se recorta `text` si no cabe.
    pub overflow: Overflow,
    /// Lo que va a la derecha, apagado: el atajo de una acción o dónde está.
    /// Si no cabe, se recorta como una ruta.
    pub detail: Option<String>,
}

/// Cómo se recorta el texto de una fila que no cabe.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Overflow {
    /// Alrededor del primer resaltado, que queda a la vista y centrado.
    #[default]
    Around,
    /// Como una ruta: lo último que se pierde es el nombre y, si el primer
    /// resaltado cae en la carpeta, también se ve él.
    Path,
}

/// Cómo se ve y se comporta un picker.
#[derive(Clone, Debug)]
pub struct PickerOptions {
    /// Lo que dice el input vacío.
    pub placeholder: SharedString,
    /// Si `ctrl-v` y `ctrl-s` eligen abriendo en un split.
    pub splits: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PickerEvent {
    Chosen(Chosen),
    Dismissed,
}

pub struct PickerView<D> {
    delegate: D,
    options: PickerOptions,
    focus_handle: FocusHandle,
    scroll: UniformListScrollHandle,
    /// El texto del input se corre a lo ancho para que el cursor siempre se
    /// vea, como la línea de comandos de vim.
    input_scroll: ScrollHandle,
    /// Preedit del IME o de una tecla muerta.
    marked: Option<String>,
}

impl<D: 'static> EventEmitter<PickerEvent> for PickerView<D> {}

impl<D: 'static> Focusable for PickerView<D> {
    fn focus_handle(&self, _cx: &gpui_kit::App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl<D: PickerDelegate> PickerView<D> {
    pub fn new(delegate: D, options: PickerOptions, cx: &mut Context<Self>) -> Self {
        Self {
            delegate,
            options,
            focus_handle: cx.focus_handle(),
            scroll: UniformListScrollHandle::new(),
            input_scroll: ScrollHandle::new(),
            marked: None,
        }
    }

    #[must_use]
    pub fn delegate(&self) -> &D {
        &self.delegate
    }

    /// Corre `update` sobre el delegado y vuelve a pintar con la fila
    /// marcada a la vista: es como vuelve el trabajo de fondo.
    pub fn update_delegate<R>(
        &mut self,
        cx: &mut Context<Self>,
        update: impl FnOnce(&mut D, &mut Context<Self>) -> R,
    ) -> R {
        let result = update(&mut self.delegate, cx);
        self.changed(cx);
        result
    }

    fn changed(&self, cx: &mut Context<Self>) {
        self.scroll
            .scroll_to_item(self.delegate.selected(), ScrollStrategy::Nearest);
        cx.notify();
    }

    fn edit(&mut self, edit: LineEdit<'_>, cx: &mut Context<Self>) {
        self.update_delegate(cx, |delegate, cx| delegate.edit(edit, cx));
    }

    /// Las acciones del picker; las demás siguen hacia la raíz de la
    /// ventana.
    fn on_dispatch(
        &mut self,
        Dispatch(invocation): &Dispatch,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Action::Picker(action) = invocation.action else {
            cx.propagate();
            return;
        };
        match action {
            PickerAction::SelectNext => {
                self.delegate.select_next();
                self.changed(cx);
            }
            PickerAction::SelectPrevious => {
                self.delegate.select_previous();
                self.changed(cx);
            }
            PickerAction::Confirm => self.choose(Open::Here, cx),
            PickerAction::ConfirmSplitRight => self.choose(Open::SplitRight, cx),
            PickerAction::ConfirmSplitDown => self.choose(Open::SplitDown, cx),
            PickerAction::Dismiss => cx.emit(PickerEvent::Dismissed),
            PickerAction::DeleteBackward => self.edit(LineEdit::DeleteBackward, cx),
            PickerAction::DeleteForward => self.edit(LineEdit::DeleteForward, cx),
            PickerAction::DeleteWordBackward => self.edit(LineEdit::DeleteWordBackward, cx),
            PickerAction::DeleteToStart => self.edit(LineEdit::DeleteToStart, cx),
            PickerAction::MoveLeft => self.edit(LineEdit::Left, cx),
            PickerAction::MoveRight => self.edit(LineEdit::Right, cx),
            PickerAction::MoveToStart => self.edit(LineEdit::Home, cx),
            PickerAction::MoveToEnd => self.edit(LineEdit::End, cx),
        }
    }

    fn choose(&mut self, open: Open, cx: &mut Context<Self>) {
        if let Some(chosen) = self.update_delegate(cx, |delegate, cx| delegate.choose(open, cx)) {
            cx.emit(PickerEvent::Chosen(chosen));
        }
    }

    fn click(&mut self, row: usize, cx: &mut Context<Self>) {
        self.delegate.select(row);
        self.choose(Open::Here, cx);
    }

    fn key_context(&self) -> KeyContext {
        let mut key_context = KeyContext::new_with_defaults();
        key_context.add(context::PICKER);
        if self.options.splits {
            key_context.add(context::SPLITS);
        }
        key_context
    }
}

#[cfg(test)]
mod tests;
