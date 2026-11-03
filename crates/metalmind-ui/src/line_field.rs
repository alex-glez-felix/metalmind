//! El campo de una línea de texto: el del picker, el de la lista de notas y
//! el de las preguntas del panel. Es dueño de lo que la plataforma le manda
//! (el foco donde escucha, el texto, el preedit del IME y las teclas muertas)
//! y de cómo se ve: el cursor de barra, el preedit subrayado y el recorte. La
//! línea sigue siendo de quien la tiene ([`LineHost`]), y cada edición va a él.

use std::ops::Range;

use gpui_kit::{
    App, Bounds, Context, Div, ElementInputHandler, Entity, EntityInputHandler, FocusHandle,
    HighlightStyle, Hsla, InteractiveElement as _, IntoElement, ParentElement as _, Pixels, Point,
    SharedString, Styled as _, StyledText, UTF16Selection, WeakEntity, Window, canvas, div,
    prelude::FluentBuilder as _,
};

use crate::ime::{ImeLine, commit};
use crate::picker::{InputLine, LineEdit};
use crate::theme::Theme;

/// La vista que tiene la línea que se escribe.
pub trait LineHost: Sized + 'static {
    /// La línea que se escribe ahora; `None` si no hay ningún campo abierto.
    fn field_line(&self) -> Option<&InputLine>;

    fn edit_field(&mut self, edit: LineEdit<'_>, cx: &mut Context<Self>);
}

/// Lo que cambia de un campo a otro al pintarlo.
#[derive(Clone, Debug, Default)]
pub struct FieldLook {
    /// Los bytes de lo escrito que se marcan como aviso.
    pub warnings: Vec<Range<usize>>,
    /// Lo que dice el campo vacío.
    pub placeholder: Option<SharedString>,
}

pub struct LineField<H> {
    host: WeakEntity<H>,
    /// El del host: el campo recibe texto mientras el host tiene el foco.
    focus: FocusHandle,
    /// El preedit del IME o de una tecla muerta.
    marked: Option<String>,
}

impl<H: LineHost> LineField<H> {
    pub fn new(host: WeakEntity<H>, focus: FocusHandle) -> Self {
        Self {
            host,
            focus,
            marked: None,
        }
    }

    #[must_use]
    pub fn marked(&self) -> &str {
        self.marked.as_deref().unwrap_or("")
    }

    /// Un preedit sin confirmar no pasa a la pregunta siguiente.
    pub fn clear_preedit(&mut self) {
        self.marked = None;
    }

    /// El campo con `line`, que el host ya tiene a la mano mientras se pinta.
    #[must_use]
    pub fn element(
        this: &Entity<Self>,
        line: &InputLine,
        look: FieldLook,
        theme: &Theme,
        cx: &App,
    ) -> Div {
        let field = this.read(cx);
        let marked = field.marked();
        let (before, after) = line.text().split_at(line.cursor());
        let warning = theme.ui.picker_warning;
        let placeholder = look
            .placeholder
            .filter(|_| line.text().is_empty() && marked.is_empty());
        let up_to_caret = div()
            .flex()
            .items_center()
            .min_w_0()
            .child(
                clipped()
                    .debug_selector(|| "line-field-before".to_owned())
                    .text_ellipsis_start()
                    .child(warned_text(before, 0, &look.warnings, warning)),
            )
            .when(!marked.is_empty(), |text| {
                text.child(
                    clipped()
                        .text_ellipsis_start()
                        .underline()
                        .child(marked.to_owned()),
                )
            })
            .child(
                div()
                    .debug_selector(|| "line-field-caret".to_owned())
                    .w(theme.stroke.caret)
                    .h(theme.ui_line())
                    .flex_none()
                    .bg(theme.cursor.insert),
            );
        // Sin ancho propio: solo ocupa lo que deja lo anterior al cursor, así
        // lo primero que se recorta es lo que le sigue y el cursor siempre se
        // ve.
        let rest = clipped()
            .debug_selector(|| "line-field-after".to_owned())
            .flex_1()
            .text_ellipsis();
        let rest = match placeholder {
            Some(placeholder) => rest.text_color(theme.ui.picker_muted).child(placeholder),
            None => rest.child(warned_text(after, before.len(), &look.warnings, warning)),
        };
        div()
            .debug_selector(|| "line-field".to_owned())
            .relative()
            .flex()
            .flex_1()
            .min_w_0()
            .items_center()
            .child(up_to_caret)
            .child(rest)
            .child(Self::input_handler(this, field.focus.clone()))
    }

    /// El texto, el IME y las teclas muertas entran al campo por aquí.
    fn input_handler(this: &Entity<Self>, focus: FocusHandle) -> impl IntoElement {
        let entity = this.clone();
        canvas(
            |_, _, _| (),
            move |bounds, (), window, cx| {
                window.handle_input(&focus, ElementInputHandler::new(bounds, entity), cx);
            },
        )
        .absolute()
        .size_full()
    }

    fn with_line<R>(&self, cx: &App, read: impl FnOnce(&InputLine) -> R) -> Option<R> {
        let host = self.host.upgrade()?;
        host.read(cx).field_line().map(read)
    }

    fn ime_line(&self, cx: &App) -> Option<ImeLine> {
        self.with_line(cx, |line| {
            ImeLine::new(line.text().to_owned(), line.cursor(), self.marked())
        })
    }

    fn repaint_host(&self, cx: &mut App) {
        // Sin host no hay quien pinte el campo.
        let _ = self.host.update(cx, |_, cx| cx.notify());
    }
}

/// La caja con borde de un campo en un panel.
#[must_use]
pub fn field_box(theme: &Theme) -> Div {
    div()
        .flex()
        .items_center()
        .gap(theme.layout.panel_gap)
        .h(theme.panel_row_height)
        .px(theme.layout.panel_padding_x)
        .bg(theme.ui.background)
        .border(theme.stroke.border)
        .border_color(theme.ui.panel_border)
        .rounded(theme.radius.md)
        .whitespace_nowrap()
        .overflow_hidden()
}

fn clipped() -> Div {
    div().min_w_0().overflow_hidden().whitespace_nowrap()
}

/// `text`, que empieza en el byte `offset` de lo escrito, con los avisos que
/// caen en él pintados de `color`.
#[must_use]
pub fn warned_text(
    text: &str,
    offset: usize,
    warnings: &[Range<usize>],
    color: Hsla,
) -> StyledText {
    let style = HighlightStyle {
        color: Some(color),
        ..HighlightStyle::default()
    };
    let highlights = warning_ranges(text, offset, warnings)
        .map(|range| (range, style))
        .collect::<Vec<_>>();
    StyledText::new(SharedString::from(text.to_owned())).with_highlights(highlights)
}

/// Los avisos que caen en `text`, relativos a él.
fn warning_ranges<'a>(
    text: &str,
    offset: usize,
    warnings: &'a [Range<usize>],
) -> impl Iterator<Item = Range<usize>> + 'a {
    let end = offset + text.len();
    warnings
        .iter()
        .map(move |warning| warning.start.max(offset)..warning.end.min(end))
        .filter(|range| range.start < range.end)
        .map(move |range| range.start - offset..range.end - offset)
}

impl<H: LineHost> EntityInputHandler for LineField<H> {
    fn text_for_range(
        &mut self,
        range: Range<usize>,
        adjusted_range: &mut Option<Range<usize>>,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<String> {
        let (text, adjusted) = self.ime_line(cx)?.slice(range);
        *adjusted_range = Some(adjusted);
        Some(text)
    }

    fn selected_text_range(
        &mut self,
        _ignore_disabled_input: bool,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<UTF16Selection> {
        let caret = self.ime_line(cx)?.caret();
        Some(UTF16Selection {
            range: caret..caret,
            reversed: false,
        })
    }

    fn marked_text_range(
        &self,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<Range<usize>> {
        self.marked.as_ref()?;
        Some(self.ime_line(cx)?.marked_range())
    }

    fn unmark_text(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        self.marked = None;
        self.repaint_host(cx);
    }

    fn replace_text_in_range(
        &mut self,
        range: Option<Range<usize>>,
        text: &str,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let had_preedit = self.marked.take().is_some();
        let Some(line) = self.with_line(cx, |line| line.text().to_owned()) else {
            return;
        };
        let edit = commit(&line, had_preedit, range, text);
        let _ = self.host.update(cx, |host, cx| host.edit_field(edit, cx));
    }

    fn replace_and_mark_text_in_range(
        &mut self,
        _range: Option<Range<usize>>,
        new_text: &str,
        _new_selected_range: Option<Range<usize>>,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.with_line(cx, |_| ()).is_some() {
            self.marked = (!new_text.is_empty()).then(|| new_text.to_owned());
            self.repaint_host(cx);
        }
    }

    fn bounds_for_range(
        &mut self,
        _range_utf16: Range<usize>,
        element_bounds: Bounds<Pixels>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<Bounds<Pixels>> {
        Some(element_bounds)
    }

    fn character_index_for_point(
        &mut self,
        _point: Point<Pixels>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<usize> {
        None
    }
}

#[cfg(test)]
#[path = "line_field_tests.rs"]
mod tests;
