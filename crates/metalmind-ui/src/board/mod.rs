//! La pizarra en un panel ([09](../../docs/research/09-pizarra.md)): lee el
//! `.canvas` del documento, lo guarda teselado y lo pinta con su cámara. El
//! documento sigue siendo el archivo: guardar, releer y `:w` son los de una nota.

mod camera;
mod elements;
mod input;
mod label;
mod paint;
mod plan;
mod scene;
mod shapes;
mod tint;

use std::path::PathBuf;

use gpui_kit::{
    App, Bounds, Context, CursorStyle, Entity, InteractiveElement as _, IntoElement, MouseButton,
    ParentElement as _, Pixels, Point as ScreenPoint, Render, Styled as _, Window, canvas, div,
    prelude::FluentBuilder as _,
};
use metalmind_board::{Board, Node, Point};
use metalmind_keymap::{BoardAction, View};

pub use self::camera::{AreaSize, Camera};
use self::paint::Frame;
pub use self::plan::plan;
pub use self::scene::{Scene, SceneStyle};
use crate::document::Revision;
use crate::finder::VaultSession;
use crate::frames::FrameClock;
use crate::key_button::key_button;
use crate::theme::Theme;
use crate::workspace::{PaneId, Workspace};

/// Cuánto se multiplica el zoom con `+` y `-`.
const ZOOM_STEP: f32 = 1.25;

/// Lo que dice el archivo, ya leído.
enum Content {
    Board {
        board: Board,
        scene: Scene,
    },
    /// No se lee como pizarra: lo que hay que decir en su lugar.
    Invalid(String),
}

/// De qué versión del archivo y con qué medidas se armó la escena.
#[derive(PartialEq)]
struct Source {
    path: Option<PathBuf>,
    revision: Revision,
    style: SceneStyle,
}

pub struct BoardView {
    workspace: Entity<Workspace>,
    session: Entity<VaultSession>,
    pane: PaneId,
    source: Option<Source>,
    content: Content,
    /// `None` hasta que se sabe lo que mide el área: la primera vista es
    /// "ver todo".
    camera: Option<Camera>,
    bounds: Option<Bounds<Pixels>>,
    /// El nodo elegido, por su id: sobrevive a que el archivo se relea.
    selected: Option<String>,
    /// Dónde iba el ratón mientras arrastra la pizarra.
    drag: Option<ScreenPoint<Pixels>>,
}

impl BoardView {
    #[must_use]
    pub fn new(workspace: Entity<Workspace>, session: Entity<VaultSession>, pane: PaneId) -> Self {
        Self {
            workspace,
            session,
            pane,
            source: None,
            content: Content::Invalid(String::new()),
            camera: None,
            bounds: None,
            selected: None,
            drag: None,
        }
    }

    /// Lo que muestra la barra de estado en lugar de la posición del cursor.
    #[must_use]
    pub fn zoom_percent(&self) -> Option<u32> {
        self.camera.map(Camera::percent)
    }

    #[cfg(test)]
    pub(crate) fn camera(&self) -> Option<Camera> {
        self.camera
    }

    #[cfg(test)]
    pub(crate) fn window_position(&self, id: &str) -> Option<ScreenPoint<Pixels>> {
        let center = self.board()?.node(id)?.rect().center();
        let (camera, bounds) = (self.camera?, self.bounds?);
        let at = camera.to_screen(center);
        Some(bounds.origin + gpui_kit::point(gpui_kit::px(at.x), gpui_kit::px(at.y)))
    }

    /// Un punto sobre la barra de zoom, en su esquina.
    #[cfg(test)]
    pub(crate) fn toolbar_position(&self, theme: &Theme) -> Option<ScreenPoint<Pixels>> {
        let inset = theme.layout.board_toolbar_margin + theme.layout.board_toolbar_margin;
        Some(self.bounds?.bottom_right() - gpui_kit::point(inset, inset))
    }

    #[cfg(test)]
    pub(crate) fn selected(&self) -> Option<&str> {
        self.selected.as_deref()
    }

    #[cfg(test)]
    pub(crate) fn notice_text(&self) -> Option<String> {
        self.notice()
    }

    /// Vuelve a leer el documento si cambió desde la última vez.
    fn sync(&mut self, theme: &Theme, cx: &App) {
        let Ok(view) = self.workspace.read(cx).view(self.pane) else {
            return;
        };
        let document = view.document;
        let source = Source {
            path: document.path().map(ToOwned::to_owned),
            revision: document.revision(),
            style: SceneStyle::of(theme),
        };
        if self.source.as_ref() == Some(&source) {
            return;
        }
        let other_file = self
            .source
            .as_ref()
            .is_none_or(|old| old.path != source.path);
        self.content = read_content(&document.buffer().to_string(), source.style);
        if other_file {
            self.camera = None;
            self.selected = None;
        }
        self.source = Some(source);
    }

    fn board(&self) -> Option<&Board> {
        match &self.content {
            Content::Board { board, .. } => Some(board),
            Content::Invalid(_) => None,
        }
    }

    fn scene(&self) -> Option<&Scene> {
        match &self.content {
            Content::Board { scene, .. } => Some(scene),
            Content::Invalid(_) => None,
        }
    }

    fn selected_node(&self) -> Option<&Node> {
        self.board()?.node(self.selected.as_deref()?)
    }

    fn area(&self) -> Option<AreaSize> {
        self.bounds.map(|bounds| AreaSize {
            width: f32::from(bounds.size.width),
            height: f32::from(bounds.size.height),
        })
    }

    /// El área ya tiene su medida: la primera vez, deja la pizarra entera.
    fn settle(&mut self, bounds: Bounds<Pixels>, theme: &Theme) {
        self.bounds = Some(bounds);
        if self.camera.is_none() {
            self.camera = self.fitted(theme);
        }
    }

    fn fitted(&self, theme: &Theme) -> Option<Camera> {
        let area = self.area()?;
        let margin = f32::from(theme.layout.board_fit_margin);
        let camera = match self.scene().and_then(|scene| scene.content) {
            Some(content) => Camera::fit(content, area, margin),
            None => Camera {
                x: 0.0,
                y: 0.0,
                zoom: Camera::ACTUAL_SIZE,
            },
        };
        Some(camera)
    }

    /// El punto de la pizarra bajo una posición de la ventana.
    fn board_point(&self, at: ScreenPoint<Pixels>) -> Option<Point> {
        let (camera, bounds) = (self.camera?, self.bounds?);
        Some(camera.to_board(Point::new(
            f32::from(at.x - bounds.origin.x),
            f32::from(at.y - bounds.origin.y),
        )))
    }

    fn paint(&self, bounds: Bounds<Pixels>, theme: &Theme, window: &mut Window, cx: &mut App) {
        let (Some(camera), Some(scene)) = (self.camera, self.scene()) else {
            return;
        };
        let area = AreaSize {
            width: f32::from(bounds.size.width),
            height: f32::from(bounds.size.height),
        };
        let frame = Frame {
            camera,
            origin: bounds.origin,
            theme,
        };
        paint::paint_plan(&plan(scene, camera, area), &frame, window, cx);
        if let Some(node) = self.selected_node() {
            paint::paint_selection(node.rect(), &frame, window);
        }
    }

    /// Sin tecla no hay ratón que la enseñe, y al revés: cada botón muestra
    /// la suya (design.md, 3).
    fn toolbar(theme: &Theme, cx: &App) -> impl IntoElement {
        let margin = theme.layout.board_toolbar_margin;
        let bar = div()
            .flex()
            .flex_wrap()
            .justify_end()
            .gap(theme.layout.panel_gap)
            .bg(theme.ui.panel_background)
            .border(theme.stroke.border)
            .border_color(theme.ui.panel_border)
            .rounded(theme.radius.md)
            .map(|bar| theme.ui_small_text(bar))
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .child(button(
                "board-zoom-out",
                "alejar",
                BoardAction::ZoomOut,
                theme,
                cx,
            ))
            .child(button(
                "board-zoom-in",
                "acercar",
                BoardAction::ZoomIn,
                theme,
                cx,
            ))
            .child(button(
                "board-100",
                "100 %",
                BoardAction::ZoomToActualSize,
                theme,
                cx,
            ))
            .child(button(
                "board-fit",
                "ver todo",
                BoardAction::ZoomToFit,
                theme,
                cx,
            ));
        // En un split angosto se parte en filas en vez de recortarse.
        div()
            .absolute()
            .left(margin)
            .right(margin)
            .bottom(margin)
            .flex()
            .justify_end()
            .child(bar)
    }

    /// Lo que se escribe en el centro cuando no hay nada que pintar.
    fn notice(&self) -> Option<String> {
        match &self.content {
            Content::Invalid(reason) => Some(reason.clone()),
            Content::Board { scene, .. } if scene.content.is_none() => {
                Some("Pizarra vacía".to_owned())
            }
            Content::Board { .. } => None,
        }
    }
}

fn button(
    id: &'static str,
    label: &'static str,
    action: BoardAction,
    theme: &Theme,
    cx: &App,
) -> impl IntoElement {
    key_button(id, label, action, View::Board, theme, cx)
}

/// Un archivo que no se lee no se toca ni se pinta a medias: se dice por qué.
fn read_content(text: &str, style: SceneStyle) -> Content {
    let board = match Board::parse(text) {
        Ok(board) => board,
        Err(error) => return Content::Invalid(format!("No se lee como pizarra: {error}")),
    };
    match Scene::build(&board, style) {
        Ok(scene) => Content::Board { board, scene },
        Err(error) => Content::Invalid(format!("No se puede pintar la pizarra: {error:?}")),
    }
}

impl Render for BoardView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = Theme::get(cx).clone();
        self.sync(&theme, cx);
        let view = cx.entity();
        let (for_prepaint, for_paint) = (theme.clone(), theme.clone());
        let surface = canvas(
            {
                let view = view.clone();
                move |bounds, _, cx| {
                    view.update(cx, |view, _| view.settle(bounds, &for_prepaint));
                }
            },
            move |bounds, (), window, cx| {
                window.with_content_mask(Some(gpui_kit::ContentMask { bounds }), |window| {
                    view.update(cx, |view, cx| {
                        view.paint(bounds, &for_paint, window, cx);
                    });
                });
                FrameClock::painted(cx);
            },
        )
        .size_full();
        let notice = self.notice().map(|text| {
            div()
                .absolute()
                .inset_0()
                .flex()
                .items_center()
                .justify_center()
                .px(theme.layout.board_fit_margin)
                .text_color(theme.ui.board_text_muted)
                .child(text)
        });
        div()
            .relative()
            .size_full()
            .overflow_hidden()
            .bg(theme.ui.background)
            .cursor(CursorStyle::Arrow)
            .on_mouse_down(MouseButton::Left, cx.listener(Self::on_left_down))
            .on_mouse_move(cx.listener(Self::on_mouse_move))
            .on_mouse_up(MouseButton::Left, cx.listener(Self::on_left_up))
            .on_mouse_up_out(MouseButton::Left, cx.listener(Self::on_left_up))
            .on_scroll_wheel(cx.listener(Self::on_scroll))
            .child(surface)
            .children(notice)
            .child(Self::toolbar(&theme, cx))
    }
}
