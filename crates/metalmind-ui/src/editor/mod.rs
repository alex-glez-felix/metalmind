//! Vista de un panel: pinta solo lo visible de su documento y le pasa la
//! entrada al [`Workspace`], que es quien edita.

mod board_pane;
mod command_popup;
mod completion;
mod completion_menu;
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
mod path_header;
mod replaced;
mod rows;
mod screen;
mod scroll;
mod spans;
mod status_bar;
mod text;
mod which_key;

use std::collections::HashMap;
use std::ops::Range;
use std::path::PathBuf;
use std::sync::Arc;

use gpui_kit::{
    App, Context, Entity, FocusHandle, InteractiveElement as _, IntoElement, KeyDownEvent,
    ParentElement as _, Pixels, Point, Render, Styled as _, Subscription, Window, div,
};
use metalmind_buffer::ByteOffset;
use metalmind_keymap::{Action, EditorAction};
use metalmind_vault::VaultPath;
use metalmind_vim::{KeyOutcome, Mode, Prompt};

use self::keys::Route;
use self::layout::FrameLayout;
use self::links::Links;
use self::screen::TextScreen;
use self::scroll::{Anchor, Scroll};
use self::status_bar::{Status, status_bar};
use self::text::Faces;
use self::which_key::{WhichKey, which_key_panel};
use crate::actions::{self, Dispatch};
use crate::board::{BoardView, Camera};
use crate::clipboard::SystemClipboard;
use crate::finder::{FocusChanged, VaultChanged, VaultSession};
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
    /// Dónde está el ratón sobre el texto; `None` fuera o arrastrando. El
    /// tooltip de una fórmula rota se busca ahí en cada frame, con lo que se
    /// pinta en él: cambiar de nota sin mover el ratón no deja uno viejo.
    pointer: Option<Point<Pixels>>,
    /// La ruta del documento en el vault, de la última vez que se miró.
    source: Option<(PathBuf, Option<VaultPath>)>,
    focus_handle: FocusHandle,
    scroll: Scroll,
    /// Lo pintado en el último frame, para el ratón y el IME.
    layout: Option<FrameLayout>,
    /// Preedit del IME o de una tecla muerta.
    marked: Option<String>,
    drag_anchor: Option<ByteOffset>,
    /// El botón bajó sobre una casilla en Normal: se marca si se suelta sin
    /// arrastrar.
    checkbox_pressed: bool,
    /// Dónde estaba la ventana del panel en el workspace la última vez que
    /// se leyó o se escribió (ver [`Scroll::follow`]); `None` tras una
    /// tecla, para que la vista quede alineada a las filas del motor.
    synced: Option<Anchor>,
    /// Lo que la rueda corrió a lo ancho de menos de un píxel (ver
    /// [`self::scroll::sideways`]).
    across: f32,
    /// La línea con marks bajo el ratón, para su tooltip.
    mark_hover: Option<self::mark_hover::MarkHover>,
    completion: self::completion::Completion,
    /// Con qué mide el motor las líneas de este panel, mientras la letra
    /// sea la misma.
    text_screen: Option<Arc<TextScreen>>,
    which_key: WhichKey,
    /// Lo que pinta el panel si su documento es una pizarra.
    board: Option<Entity<BoardView>>,
    /// Dónde estaba la vista de cada pizarra que este panel dejó de mostrar.
    board_cameras: HashMap<PathBuf, Camera>,
    _repaint: Subscription,
    _vault_changed: Subscription,
    _focus_changed: Subscription,
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
        let repaint = cx.observe(&workspace, |editor, _, cx| {
            editor.refresh_completion(cx);
            cx.notify();
        });
        // Una nota nueva puede resolver un link que se ve o traer otros tags.
        let vault_changed = cx.subscribe(&session, |editor, _, _: &VaultChanged, cx| {
            editor.refresh_completion(cx);
            cx.notify();
        });
        let focus_changed = cx.subscribe(&session, |_, _, _: &FocusChanged, cx| cx.notify());
        // Llegó la imagen de una fórmula.
        let formulas_ready = cx.observe(&formulas, |_, _, cx| cx.notify());
        Self {
            workspace,
            session,
            formulas,
            pointer: None,
            source: None,
            pane,
            focus_handle: cx.focus_handle(),
            scroll: Scroll::default(),
            layout: None,
            marked: None,
            drag_anchor: None,
            checkbox_pressed: false,
            synced: None,
            across: 0.,
            mark_hover: None,
            completion: self::completion::Completion::default(),
            text_screen: None,
            which_key: WhichKey::default(),
            board: None,
            board_cameras: HashMap::new(),
            _repaint: repaint,
            _vault_changed: vault_changed,
            _focus_changed: focus_changed,
            _formulas: formulas_ready,
        }
    }

    pub(crate) fn board(&self) -> Option<&Entity<BoardView>> {
        self.board.as_ref()
    }

    /// Con qué saber si cada link del documento lleva a algo.
    fn links(&mut self, cx: &App) -> Links {
        if self
            .view(cx)
            .is_none_or(|view| view.document.path().is_none())
        {
            return Links::default();
        }
        let source = self.note_path(cx);
        Links::new(self.session.read(cx).vault().links(), source)
    }

    /// La ruta en el vault del documento del panel, guardada mientras sea
    /// el mismo archivo.
    fn note_path(&mut self, cx: &App) -> Option<VaultPath> {
        let path = self.view(cx)?.document.path()?.to_path_buf();
        match &self.source {
            Some((cached, source)) if *cached == path => source.clone(),
            _ => {
                let source = self.session.read(cx).vault_path(&path);
                self.source = Some((path, source.clone()));
                source
            }
        }
    }

    /// El medidor de las líneas con la letra `faces`: el de antes si es la
    /// misma, así no se pierde lo ya medido.
    fn text_screen(&mut self, faces: Faces, window: &Window) -> Arc<TextScreen> {
        match &self.text_screen {
            Some(screen) if *screen.faces() == faces => screen.clone(),
            _ => {
                let screen = Arc::new(TextScreen::new(window.text_system().clone(), faces));
                self.text_screen = Some(screen.clone());
                screen
            }
        }
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
        if self.feed_board_hint(keystroke, cx) {
            cx.stop_propagation();
            return;
        }
        let mode = self.mode(cx);
        let route = keys::route(keystroke, mode);
        let text_open = self.board_text_open(cx);
        if self.board.is_some() && !keys::reaches_engine_in_board(route, mode, text_open) {
            // Sin esto, su carácter le llegaría al motor por el input handler.
            cx.stop_propagation();
            return;
        }
        match route {
            Route::Engine(key) => {
                FrameClock::input(cx);
                cx.stop_propagation();
                self.marked = None;
                let outcome = self.update_workspace(cx, |workspace, pane, cx| {
                    workspace.feed(pane, key, &mut SystemClipboard(cx))
                });
                self.schedule_which_key(cx);
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
        // Un IME puede escribir sin pasar por una tecla.
        if self.board.is_some() && !self.board_text_open(cx) && self.mode(cx) == Mode::Normal {
            return;
        }
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
        if let Some(outcome) = outcome {
            self.handle_outcome(outcome, window, cx);
        }
        // Solo escribir abre la lista; las teclas del motor (`A`, borrar)
        // solo refrescan la abierta, al observar el workspace.
        self.update_completion(cx);
        cx.notify();
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
            Ok(KeyOutcome::Dispatch(invocation)) => {
                let name = invocation.action.as_str();
                if let Err(error) = actions::dispatch(name, invocation.argument, window, cx) {
                    self.report(&error.into(), cx);
                }
            }
            Ok(_) => {}
            Err(error) => self.report(&error, cx),
        }
        self.synced = None;
        self.across = 0.;
        cx.notify();
    }

    /// La línea donde se escribe, tal como la ve el IME.
    fn ime_line(&self, cx: &App) -> Option<ImeLine> {
        let marked = self.marked.as_deref().unwrap_or("");
        let view = self.view(cx)?;
        if let Some(CommandLine { typed, .. }) = view.command_line {
            return Some(ImeLine::new(typed.to_owned(), typed.len(), marked));
        }
        let (_, buffer) = view.document.engine_view();
        let line = buffer.line_of(view.cursor());
        let text = buffer.line_text(line).into_owned();
        let column = view.cursor().get() - buffer.line_start(line).get();
        Some(ImeLine::new(text, column, marked))
    }

    /// Lo que se escribe tras `:`, `/` o `?`, con el preedit del IME.
    fn command_line(&self, cx: &App) -> Option<(Prompt, status_bar::CommandLine)> {
        let line = self.view(cx)?.command_line?;
        Some((
            line.prompt,
            status_bar::CommandLine {
                symbol: line.prompt.symbol(),
                typed: line.typed.to_owned(),
                preedit: self.marked.clone().unwrap_or_default(),
            },
        ))
    }

    /// La búsqueda se escribe en la barra, como en vim, para no tapar lo que
    /// resalta; `:` va en su pop-up.
    fn status(&self, cx: &App) -> Option<Status> {
        let view = self.view(cx)?;
        let (_, buffer) = view.document.engine_view();
        let line = buffer.line_of(view.cursor());
        let command_line = self
            .command_line(cx)
            .filter(|(prompt, _)| *prompt != Prompt::Command)
            .map(|(_, line)| line);
        Some(Status {
            mode: view.focused.then_some(view.mode),
            recording: view.recording,
            file: view.document.display_name().into(),
            modified: view.document.is_modified(),
            save: view.save,
            command_line,
            position: self.position(line + 1, buffer.column_of(view.cursor()) + 1, cx),
            focus: self
                .session
                .read(cx)
                .focus()
                .map(|focus| focus.label().into()),
        })
    }
}

impl Render for Editor {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = Theme::get(cx).clone();
        self.sync_board(cx);
        let popup = self
            .command_line(cx)
            .filter(|(prompt, _)| *prompt == Prompt::Command)
            .map(|(_, line)| self.command_popup(line, window.viewport_size().width, &theme, cx));
        let status = self.status(cx).map(|status| status_bar(status, &theme));
        let header = self.path_header(&theme, cx);
        let which_key = self
            .next_keys(window, cx)
            .map(|next_keys| which_key_panel(next_keys, &theme));
        div()
            .key_context(self.key_context(cx))
            .track_focus(&self.focus_handle)
            .size_full()
            .flex()
            .flex_col()
            .on_key_down(cx.listener(Self::on_key_down))
            .on_action(
                cx.listener(
                    |editor, Dispatch(invocation): &Dispatch, window, cx| match invocation.action {
                        Action::Editor(EditorAction::Copy) => editor.copy(cx),
                        Action::Editor(EditorAction::Paste) => editor.paste(cx),
                        Action::Editor(EditorAction::AcceptCompletion) => {
                            editor.accept_completion(window, cx);
                        }
                        Action::Editor(EditorAction::NextCompletion) => {
                            editor.select_next_completion(cx);
                        }
                        Action::Editor(EditorAction::PreviousCompletion) => {
                            editor.select_previous_completion(cx);
                        }
                        Action::Board(action) => editor.on_board_action(action, window, cx),
                        _ => cx.propagate(),
                    },
                ),
            )
            .child(header)
            .child(self.body(&theme, cx).children(which_key))
            .children(status)
            .children(popup)
    }
}
