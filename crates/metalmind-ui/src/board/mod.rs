//! La pizarra en un panel ([09](../../docs/research/09-pizarra.md)): lee el
//! `.canvas` del documento, lo guarda teselado y lo pinta con su cámara. El
//! documento sigue siendo el archivo: guardar, releer y `:w` son los de una nota.

mod actions;
mod camera;
mod drag;
mod editing;
mod elements;
mod gesture;
mod hints;
mod label;
mod mouse;
mod open_text;
mod overlays;
mod paint;
mod plan;
mod scene;
mod shapes;
mod tint;
mod toolbar;

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

use gpui_kit::{
    App, Bounds, Context, CursorStyle, Entity, InteractiveElement as _, IntoElement, MouseButton,
    ParentElement as _, Pixels, Point as ScreenPoint, Render, Styled as _, Window, canvas, div,
};
use metalmind_board::{Board, Node, Point, Shape};

pub use self::camera::{AreaSize, Camera};
use self::gesture::Gesture;
use self::hints::Hints;
use self::open_text::TextArea;
pub use self::plan::plan;
pub use self::scene::{OpenLabel, Scene, SceneStyle};
use crate::document::Revision;
use crate::finder::VaultSession;
use crate::frames::FrameClock;
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

/// De qué versión del archivo y con qué medidas se armó la escena, y qué
/// elemento está sin su texto porque se está escribiendo.
#[derive(PartialEq)]
struct Source {
    path: Option<PathBuf>,
    revision: Revision,
    style: SceneStyle,
    open: Option<String>,
}

/// Qué hace el siguiente clic o `enter`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tool {
    Select,
    Shape(Shape),
    Text,
    Arrow,
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
    /// Dónde estaba la vista de las pizarras que ya no se ven en este panel.
    cameras: HashMap<PathBuf, Camera>,
    bounds: Option<Bounds<Pixels>>,
    /// Lo elegido, por su id: sobrevive a que el archivo se relea. Nodos y
    /// flechas; el primer nodo es de donde parten `H J K L`.
    selected: Vec<String>,
    tool: Tool,
    gesture: Option<Gesture>,
    /// Las letras de unir con el teclado, si están a la vista.
    hints: Option<Hints>,
    /// Dónde se abrió el menú contextual.
    menu: Option<ScreenPoint<Pixels>>,
    cursor: CursorStyle,
    /// Dónde va el texto que se está escribiendo aquí.
    open: Option<OpenLabel>,
    /// El texto provisional del IME.
    preedit: Option<String>,
    /// Lo que dejó pintado el texto abierto, para el ratón y el IME.
    text_area: Option<TextArea>,
    caret: Option<Bounds<Pixels>>,
}

impl BoardView {
    #[must_use]
    pub fn new(
        workspace: Entity<Workspace>,
        session: Entity<VaultSession>,
        pane: PaneId,
        cameras: HashMap<PathBuf, Camera>,
    ) -> Self {
        Self {
            workspace,
            session,
            pane,
            source: None,
            content: Content::Invalid(String::new()),
            camera: None,
            cameras,
            bounds: None,
            selected: Vec::new(),
            tool: Tool::Select,
            gesture: None,
            hints: None,
            menu: None,
            cursor: CursorStyle::Arrow,
            open: None,
            preedit: None,
            text_area: None,
            caret: None,
        }
    }

    fn keep_camera(&mut self) {
        let path = self.source.as_ref().and_then(|source| source.path.clone());
        if let (Some(path), Some(camera)) = (path, self.camera) {
            self.cameras.insert(path, camera);
        }
    }

    /// Lo que hay que recordar de esta vista al dejar de mostrarla.
    #[must_use]
    pub fn camera_by_file(&self) -> HashMap<PathBuf, Camera> {
        let mut cameras = self.cameras.clone();
        if let (Some(source), Some(camera)) = (&self.source, self.camera)
            && let Some(path) = &source.path
        {
            cameras.insert(path.clone(), camera);
        }
        cameras
    }

    /// Con las letras de unir a la vista, cada tecla es una letra.
    #[must_use]
    pub fn hints_open(&self) -> bool {
        self.hints.is_some()
    }

    /// El texto provisional del IME: se pinta en el cursor del texto abierto.
    pub fn set_preedit(&mut self, preedit: Option<String>) {
        self.preedit = preedit;
    }

    /// Dónde está el cursor del texto abierto, para los candidatos del IME.
    #[must_use]
    pub fn caret(&self) -> Option<Bounds<Pixels>> {
        self.open.and(self.caret)
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
        self.selected.first().map(String::as_str)
    }

    #[cfg(test)]
    pub(crate) fn all_selected(&self) -> &[String] {
        &self.selected
    }

    #[cfg(test)]
    pub(crate) fn tool(&self) -> Tool {
        self.tool
    }

    #[cfg(test)]
    pub(crate) fn hint_labels(&self) -> Vec<(String, String)> {
        self.hints.as_ref().map_or_else(Vec::new, |hints| {
            hints
                .visible()
                .map(|(id, label)| (id.to_owned(), label.to_owned()))
                .collect()
        })
    }

    #[cfg(test)]
    pub(crate) fn window_point(&self, at: Point) -> Option<ScreenPoint<Pixels>> {
        let (camera, bounds) = (self.camera?, self.bounds?);
        let at = camera.to_screen(at);
        Some(bounds.origin + gpui_kit::point(gpui_kit::px(at.x), gpui_kit::px(at.y)))
    }

    #[cfg(test)]
    pub(crate) fn board_for_tests(&self) -> Option<&Board> {
        self.board()
    }

    #[cfg(test)]
    pub(crate) fn notice_text(&self) -> Option<String> {
        self.notice()
    }

    /// Vuelve a leer el documento si cambió desde la última vez. Con un gesto
    /// a medias espera a soltar: lo que se ve es lo arrastrado.
    fn sync(&mut self, theme: &Theme, cx: &App) {
        let Ok(view) = self.workspace.read(cx).view(self.pane) else {
            return;
        };
        let document = view.document;
        let open = document
            .open_board_text()
            .filter(|_| view.focused)
            .map(|open| open.element.to_owned());
        let source = Source {
            path: document.path().map(ToOwned::to_owned),
            revision: document.revision(),
            style: SceneStyle::of(theme),
            open,
        };
        if self.source.as_ref() == Some(&source) {
            return;
        }
        let other_file = self
            .source
            .as_ref()
            .is_none_or(|old| old.path != source.path);
        if self.gesture.is_some() && !other_file {
            return;
        }
        if other_file {
            self.keep_camera();
            self.camera = source
                .path
                .as_ref()
                .and_then(|path| self.cameras.get(path))
                .copied();
            self.selected.clear();
            self.gesture = None;
            self.hints = None;
            self.tool = Tool::Select;
        }
        let text = document.buffer().to_string();
        let was_open = self.source.as_ref().and_then(|old| old.open.clone());
        let same_style = self
            .source
            .as_ref()
            .is_some_and(|old| old.style == source.style);
        self.read_changes(
            &text,
            &source,
            was_open.as_deref(),
            same_style && !other_file,
        );
        self.source = Some(source);
    }

    fn read_changes(
        &mut self,
        text: &str,
        source: &Source,
        was_open: Option<&str>,
        can_update: bool,
    ) {
        let style = source.style;
        let new = match Board::parse(text) {
            Ok(new) => new,
            Err(error) => {
                self.open = None;
                self.content = Content::Invalid(format!("No se lee como pizarra: {error}"));
                return;
            }
        };
        let updated = match &mut self.content {
            Content::Board { board, scene } if can_update => scene
                .refresh(board, &new, style)
                .and_then(|()| restore_label(scene, &new, was_open, style)),
            _ => Scene::build(&new, style).map(|scene| {
                self.content = Content::Board {
                    board: Board::default(),
                    scene,
                };
            }),
        };
        if let Err(error) = updated {
            self.open = None;
            self.content = Content::Invalid(format!("No se puede pintar la pizarra: {error:?}"));
            return;
        }
        self.selected
            .retain(|id| new.node(id).is_some() || new.edge(id).is_some());
        if let Content::Board { board, scene } = &mut self.content {
            self.open = source
                .open
                .as_deref()
                .and_then(|element| scene.open_label(&new, element, style));
            *board = new;
        }
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

    /// Los nodos elegidos, sin las flechas.
    fn selected_nodes(&self) -> impl Iterator<Item = &Node> {
        let board = self.board();
        self.selected
            .iter()
            .filter_map(move |id| board.and_then(|board| board.node(id)))
    }

    fn selected_node(&self) -> Option<&Node> {
        self.selected_nodes().next()
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

/// El elemento que estaba sin su texto vuelve a tenerlo, si ya no se escribe en él.
fn restore_label(
    scene: &mut Scene,
    board: &Board,
    was_open: Option<&str>,
    style: SceneStyle,
) -> Result<(), lyon::tessellation::TessellationError> {
    let Some(element) = was_open else {
        return Ok(());
    };
    let node = board
        .edge(element)
        .map_or(element, |edge| edge.from.node.as_str());
    scene.update_nodes(board, &HashSet::from([node]), style)
}

impl Render for BoardView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = Theme::get(cx).clone();
        self.sync(&theme, cx);
        let menu = self
            .menu
            .map(|position| self.context_menu(position, &theme, window, cx));
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
            .cursor(self.cursor)
            .on_mouse_down(MouseButton::Left, cx.listener(Self::on_left_down))
            .on_mouse_down(MouseButton::Right, cx.listener(Self::on_right_down))
            .on_mouse_move(cx.listener(Self::on_mouse_move))
            .on_mouse_up(MouseButton::Left, cx.listener(Self::on_left_up))
            .on_mouse_up_out(MouseButton::Left, cx.listener(Self::on_left_up))
            .on_scroll_wheel(cx.listener(Self::on_scroll))
            .child(surface)
            .children(notice)
            .child(self.tools(&theme, cx))
            .child(Self::zoom_bar(&theme, cx))
            .children(menu)
    }
}
