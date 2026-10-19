//! Vista de un panel: pinta solo lo visible de su documento y le pasa la
//! entrada al [`Workspace`], que es quien edita.

mod cursor;
mod element;
mod formula_tooltip;
mod gutter;
mod highlight;
mod input;
mod keys;
mod layout;
mod lines;
mod links;
mod mark_hover;
mod markdown;
mod mouse;
mod replaced;
mod rows;
mod scroll;
mod spans;
mod status_bar;
mod text;

use std::ops::Range;
use std::path::{Path, PathBuf};

use gpui_kit::{
    App, Context, CursorStyle, Entity, FocusHandle, InteractiveElement as _, IntoElement,
    KeyDownEvent, MouseButton, ParentElement as _, Render, Styled as _, Subscription, Window, div,
    prelude::FluentBuilder as _,
};
use metalmind_buffer::ByteOffset;
use metalmind_vault::VaultPath;
use metalmind_vim::{KeyOutcome, Mode};

use self::element::EditorElement;
use self::keys::Route;
use self::layout::FrameLayout;
use self::links::Links;
use self::scroll::Scroll;
use self::status_bar::{Status, status_bar};
use crate::actions::{self, EDITOR_CONTEXT};
use crate::clipboard::SystemClipboard;
use crate::finder::{VaultChanged, VaultSession};
use crate::frames::FrameClock;
use crate::ime::ImeLine;
use crate::math::Formulas;
use crate::theme::Theme;
use crate::workspace::{CommandLine, PaneId, PaneView, Workspace, WorkspaceError};

pub struct Editor {
    workspace: Entity<Workspace>,
    pane: PaneId,
    /// El vault de la ventana, para saber qué links no llevan a nada.
    session: Entity<VaultSession>,
    /// Las fórmulas de la ventana: su layout y sus imágenes.
    formulas: Entity<Formulas>,
    /// El mensaje de la fórmula rota bajo el ratón.
    formula_error: Option<formula_tooltip::Hover>,
    /// La ruta del documento en el vault, de la última vez que se miró.
    source: Option<(PathBuf, Option<VaultPath>)>,
    focus_handle: FocusHandle,
    scroll: Scroll,
    /// Lo pintado en el último frame, para el ratón y el IME.
    layout: Option<FrameLayout>,
    /// Preedit del IME o de una tecla muerta.
    marked: Option<String>,
    drag_anchor: Option<ByteOffset>,
    /// La primera línea del panel en el workspace la última vez que se
    /// leyó o se escribió (ver [`Scroll::follow`]); `None` tras una tecla,
    /// para que la vista quede alineada a las filas del motor.
    synced_top: Option<usize>,
    /// La línea con marks bajo el ratón, para su tooltip.
    mark_hover: Option<self::mark_hover::MarkHover>,
    _repaint: Subscription,
    _vault_changed: Subscription,
    _formulas: Subscription,
}

impl Editor {
    pub fn new(
        workspace: Entity<Workspace>,
        session: Entity<VaultSession>,
        formulas: Entity<Formulas>,
        pane: PaneId,
        cx: &mut Context<Self>,
    ) -> Self {
        let repaint = cx.observe(&workspace, |_, _, cx| cx.notify());
        // Una nota nueva puede resolver un link que se ve.
        let vault_changed = cx.subscribe(&session, |_, _, _: &VaultChanged, cx| cx.notify());
        // Llegó la imagen de una fórmula.
        let formulas_ready = cx.observe(&formulas, |_, _, cx| cx.notify());
        Self {
            workspace,
            session,
            formulas,
            formula_error: None,
            source: None,
            pane,
            focus_handle: cx.focus_handle(),
            scroll: Scroll::default(),
            layout: None,
            marked: None,
            drag_anchor: None,
            synced_top: None,
            mark_hover: None,
            _repaint: repaint,
            _vault_changed: vault_changed,
            _formulas: formulas_ready,
        }
    }

    /// Con qué saber si cada link del documento lleva a algo.
    fn links(&mut self, cx: &App) -> Links {
        let path = self
            .view(cx)
            .and_then(|view| view.document.path().map(Path::to_path_buf));
        let Some(path) = path else {
            return Links::default();
        };
        let session = self.session.read(cx);
        let source = match &self.source {
            Some((cached, source)) if *cached == path => source.clone(),
            _ => {
                let source = session.vault_path(&path);
                self.source = Some((path, source.clone()));
                source
            }
        };
        Links::new(session.vault().links(), source)
    }

    #[must_use]
    pub fn focus_handle(&self) -> &FocusHandle {
        &self.focus_handle
    }

    /// Lo que muestra el panel; `None` solo si el workspace lo perdió, lo
    /// que sería un bug.
    fn view<'a>(&self, cx: &'a App) -> Option<PaneView<'a>> {
        self.workspace
            .read(cx)
            .view(self.pane)
            .inspect_err(|error| log::error!("{error}"))
            .ok()
    }

    fn mode(&self, cx: &App) -> Mode {
        self.view(cx).map_or(Mode::Normal, |view| view.mode)
    }

    /// Corre `edit` en el workspace con este panel y repinta todos. `edit`
    /// recibe la app para el portapapeles del sistema.
    fn update_workspace<T>(
        &self,
        cx: &mut Context<Self>,
        edit: impl FnOnce(&mut Workspace, PaneId, &App) -> T,
    ) -> T {
        let pane = self.pane;
        self.workspace.update(cx, |workspace, cx| {
            cx.notify();
            edit(workspace, pane, cx)
        })
    }

    fn report(&self, error: &WorkspaceError, cx: &mut Context<Self>) {
        self.update_workspace(cx, |workspace, _, _| workspace.report(error));
    }

    fn on_key_down(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        let keystroke = &event.keystroke;
        match keys::route(keystroke, self.mode(cx)) {
            Route::Engine(key) => {
                FrameClock::input(cx);
                cx.stop_propagation();
                self.marked = None;
                let outcome = self.update_workspace(cx, |workspace, pane, cx| {
                    workspace.feed(pane, key, &mut SystemClipboard(cx))
                });
                self.handle_outcome(outcome, window, cx);
            }
            Route::TextInput => FrameClock::input(cx),
            Route::Ignored => log::debug!("tecla sin equivalente en metalmind-vim: {keystroke}"),
        }
    }

    /// Texto que llega por el input handler: teclado compuesto o IME.
    fn commit_text(
        &mut self,
        range: Option<Range<usize>>,
        text: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let marked_range = self
            .ime_line(cx)
            .filter(|_| self.marked.is_some())
            .map(|ime| ime.marked_range());
        self.marked = None;
        if let Some(range) = range.filter(|range| Some(range) != marked_range.as_ref()) {
            log::warn!("el IME pidió reemplazar {range:?}; solo se escribe en el cursor");
        }
        let outcome = self
            .update_workspace(cx, |workspace, pane, cx| {
                workspace.type_text(pane, text, &mut SystemClipboard(cx))
            })
            .transpose();
        match outcome {
            Some(outcome) => self.handle_outcome(outcome, window, cx),
            None => cx.notify(),
        }
    }

    fn set_preedit(&mut self, preedit: Option<String>, cx: &mut Context<Self>) {
        self.marked = preedit;
        cx.notify();
    }

    /// Los comandos van como acción con nombre: los atrapa la vista de la
    /// ventana, que es la dueña de los paneles.
    fn handle_outcome(
        &mut self,
        outcome: Result<KeyOutcome, WorkspaceError>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match outcome {
            Ok(KeyOutcome::Command(invocation)) => {
                window.dispatch_action(actions::for_invocation(invocation), cx);
            }
            Ok(_) => {}
            Err(error) => self.report(&error, cx),
        }
        self.synced_top = None;
        cx.notify();
    }

    /// La línea donde se escribe, tal como la ve el IME.
    fn ime_line(&self, cx: &App) -> Option<ImeLine> {
        let marked = self.marked.as_deref().unwrap_or("");
        let view = self.view(cx)?;
        if let Some(CommandLine { typed, .. }) = view.command_line {
            return Some(ImeLine::new(typed.to_owned(), typed.len(), marked));
        }
        let buffer = view.buffer();
        let line = buffer.line_of(view.cursor());
        let text = view.document.line_text(line).ok()?;
        let column = view.cursor().get() - buffer.line_start(line).get();
        Some(ImeLine::new(text, column, marked))
    }

    fn status(&self, cx: &App) -> Option<Status> {
        let view = self.view(cx)?;
        let buffer = view.buffer();
        let line = buffer.line_of(view.cursor());
        let command_line = view.command_line.map(|line| status_bar::CommandLine {
            symbol: line.prompt.symbol(),
            typed: line.typed.to_owned(),
            preedit: self.marked.clone().unwrap_or_default(),
        });
        let message = view
            .focused
            .then(|| self.workspace.read(cx).message().cloned())
            .flatten();
        Some(Status {
            mode: view.focused.then_some(view.mode),
            recording: view.recording,
            file: view.document.display_name().into(),
            modified: view.document.is_modified(),
            command_line,
            message,
            position: (line + 1, buffer.column_of(view.cursor()) + 1),
        })
    }
}

impl Render for Editor {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = Theme::get(cx).clone();
        let status = self.status(cx).map(|status| status_bar(status, &theme));
        div()
            .key_context(EDITOR_CONTEXT)
            .track_focus(&self.focus_handle)
            .size_full()
            .flex()
            .flex_col()
            .on_key_down(cx.listener(Self::on_key_down))
            .on_action(cx.listener(Self::copy))
            .on_action(cx.listener(Self::paste))
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .overflow_hidden()
                    .map(|element| theme.editor_text(element))
                    .cursor(CursorStyle::IBeam)
                    .on_scroll_wheel(cx.listener(Self::on_scroll))
                    .on_mouse_down(MouseButton::Left, cx.listener(Self::on_left_down))
                    .on_mouse_move(cx.listener(Self::on_mouse_move))
                    .on_mouse_up(MouseButton::Left, cx.listener(Self::on_left_up))
                    .on_mouse_up_out(MouseButton::Left, cx.listener(Self::on_left_up))
                    .on_mouse_down(MouseButton::Middle, cx.listener(Self::on_middle_down))
                    .child(EditorElement::new(cx.entity(), theme.clone())),
            )
            .children(status)
    }
}
