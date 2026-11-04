//! Lo que escriben las acciones que cambian la pizarra: crear, escribir, borrar,
//! copiar y pegar, deshacer, cambiar de tamaño y unir con las letras.

use gpui_kit::{ClipboardItem, Context, Window};
use metalmind_board::{DEFAULT_SHAPE, DEFAULT_TEXT, Frame, GRID, Node, NodeKind, Shape};

use super::hints::Hints;
use super::{BoardView, Camera, Tool};
use crate::theme::Theme;
use crate::workspace::{Message, PaneId, Workspace, WorkspaceError};

const HALF: f32 = 0.5;
/// Por debajo de este zoom un texto no se lee: abrirlo para escribir acerca la vista.
const MIN_TEXT_ZOOM: f32 = 0.5;

/// `height` más `lacking`, en la rejilla y nunca menos.
#[expect(
    clippy::cast_possible_truncation,
    reason = "las medidas de una pizarra caben en i32"
)]
fn grown(height: i32, lacking: f32) -> i32 {
    let needed = height.saturating_add(lacking.ceil() as i32);
    match needed.rem_euclid(GRID) {
        0 => needed,
        past => needed.saturating_add(GRID - past),
    }
}

impl BoardView {
    /// `enter`: crea con la herramienta activa, escribe en la caja elegida o
    /// abre la nota de la tarjeta.
    pub(super) fn confirm(&mut self, window: &mut Window, theme: &Theme, cx: &mut Context<Self>) {
        let Some(center) = self.view_center() else {
            return;
        };
        match self.tool {
            Tool::Shape(shape) => {
                self.create_shape(shape, Frame::centered_at(center, DEFAULT_SHAPE), cx);
            }
            Tool::Text => self.create_text(Frame::centered_at(center, DEFAULT_TEXT), theme, cx),
            Tool::Pencil => {}
            Tool::Select | Tool::Arrow => {
                if self
                    .selected_node()
                    .is_some_and(|node| node.text().is_some())
                {
                    self.edit_selected_text(theme, cx);
                } else {
                    self.open_note(window, cx);
                }
            }
        }
    }

    /// Una forma nueva, sin caer justo sobre otra, ya elegida.
    pub(super) fn create_shape(&mut self, shape: Shape, frame: Frame, cx: &mut Context<Self>) {
        self.tool = Tool::Select;
        let added = self.edit(
            |board| board.add_text(board.unstacked(frame), shape, ""),
            cx,
        );
        if let Some(id) = added {
            self.selected = vec![id];
        }
    }

    /// Una tarjeta de `file` en el centro de la vista, ya elegida.
    pub fn insert_card(&mut self, file: &str, theme: &Theme, cx: &mut Context<Self>) {
        let Some(center) = self.view_center() else {
            return;
        };
        let frame = Frame::centered_at(center, DEFAULT_SHAPE);
        let added = self.edit(|board| board.add_card(board.unstacked(frame), file), cx);
        if let Some(id) = added {
            self.selected = vec![id];
            self.reveal(frame.rect(), theme);
        }
        cx.notify();
    }

    /// Un texto suelto nuevo, abierto para escribir.
    pub(super) fn create_text(&mut self, frame: Frame, theme: &Theme, cx: &mut Context<Self>) {
        self.tool = Tool::Select;
        let pane = self.pane;
        let opened = self.workspace.update(cx, |workspace, cx| {
            cx.notify();
            workspace.open_new_board_text(pane, frame)
        });
        if let Some(id) = self.keep(opened, cx) {
            self.selected = vec![id];
            self.reveal(frame.rect(), theme);
            self.ensure_readable();
        }
    }

    pub(super) fn edit_selected_text(&mut self, theme: &Theme, cx: &mut Context<Self>) {
        let [element] = self.selected.as_slice() else {
            return self.tell(
                Message::Info("Elige una caja para escribir en ella".to_owned()),
                cx,
            );
        };
        let element = element.clone();
        self.open_text(&element, theme, cx);
    }

    /// Abre el texto de `element`, o dice por qué no se puede.
    pub(super) fn open_text(&mut self, element: &str, theme: &Theme, cx: &mut Context<Self>) {
        let pane = self.pane;
        let opened = self.workspace.update(cx, |workspace, cx| {
            cx.notify();
            workspace.open_board_text(pane, element)
        });
        match self.keep(opened, cx) {
            Some(true) => {
                if let Some(rect) = self.board().and_then(|b| b.node(element)).map(Node::rect) {
                    self.reveal(rect, theme);
                }
                self.ensure_readable();
            }
            Some(false) => self.tell(Message::Info("Esto no tiene texto".to_owned()), cx),
            None => {}
        }
    }

    fn ensure_readable(&mut self) {
        if self
            .camera
            .is_some_and(|camera| camera.zoom < MIN_TEXT_ZOOM)
        {
            self.zoom_to(Camera::ACTUAL_SIZE);
            if let Some(rect) = self.selected_node().map(Node::rect)
                && let (Some(camera), Some(area)) = (&mut self.camera, self.bounds)
            {
                let center = rect.center();
                camera.x = center.x - f32::from(area.size.width) * HALF / camera.zoom;
                camera.y = center.y - f32::from(area.size.height) * HALF / camera.zoom;
            }
        }
    }

    /// Vuelve a la pizarra. Si el texto ya no cabe, la caja crece hacia abajo en
    /// el mismo paso: lo que se escribió no se queda recortado.
    pub(super) fn close_text(&mut self, cx: &mut Context<Self>) {
        self.grow_to_fit_text(cx);
        let pane = self.pane;
        let closed = self.workspace.update(cx, |workspace, cx| {
            cx.notify();
            workspace.close_board_text(pane)
        });
        self.keep(closed, cx);
    }

    fn grow_to_fit_text(&mut self, cx: &mut Context<Self>) {
        let (Some(label), Some(area), Some(camera)) = (self.open, &self.text_area, self.camera)
        else {
            return;
        };
        let lacking = f32::from(area.rows.height()) / camera.zoom - label.area.h;
        if lacking <= 0.0 {
            return;
        }
        let Some(element) = self.selected_node().map(|node| node.id.clone()) else {
            return;
        };
        self.edit(
            |board| {
                if let Some(node) = board.nodes.iter_mut().find(|node| node.id == element) {
                    node.height = grown(node.height, lacking);
                }
            },
            cx,
        );
    }

    pub(super) fn delete_selected(&mut self, cx: &mut Context<Self>) {
        if self.selected.is_empty() {
            return self.tell(Message::Info("No hay nada elegido".to_owned()), cx);
        }
        let ids = std::mem::take(&mut self.selected);
        self.edit(|board| board.remove(&ids), cx);
    }

    pub(super) fn copy_selected(&self, cx: &mut Context<Self>) {
        let Some(board) = self.board().filter(|_| !self.selected.is_empty()) else {
            return self.tell(Message::Info("No hay nada elegido".to_owned()), cx);
        };
        let lines = board.copy(&self.selected);
        cx.write_to_clipboard(ClipboardItem::new_string(lines));
    }

    pub(super) fn paste(&mut self, theme: &Theme, cx: &mut Context<Self>) {
        let Some(text) = cx.read_from_clipboard().and_then(|item| item.text()) else {
            return;
        };
        let Some(center) = self.view_center() else {
            return;
        };
        let Some(added) = self.edit(|board| board.paste(&text, center), cx) else {
            return;
        };
        if added.is_empty() {
            return;
        }
        self.selected = added;
        self.sync(theme, cx);
        let shown = self
            .selected_nodes()
            .map(Node::rect)
            .reduce(|all, rect| all.union(&rect));
        if let Some(shown) = shown {
            self.reveal(shown, theme);
        }
    }

    pub(super) fn history(
        &mut self,
        travel: impl FnOnce(&mut Workspace, PaneId) -> Result<(), WorkspaceError>,
        cx: &mut Context<Self>,
    ) {
        let pane = self.pane;
        let moved = self.workspace.update(cx, |workspace, cx| {
            cx.notify();
            travel(workspace, pane)
        });
        self.keep(moved, cx);
    }

    /// `esc`: lo que esté a medias, de lo último a lo primero.
    pub(super) fn cancel(&mut self) {
        if self.gesture.take().is_some() {
            self.stale = true;
        } else if self.hints.take().is_some() {
        } else if self.tool != Tool::Select {
            self.tool = Tool::Select;
        } else {
            self.selected.clear();
        }
    }

    pub(super) fn resize_selected(&mut self, width: i32, height: i32, cx: &mut Context<Self>) {
        if self.selected_node().is_none() {
            return self.tell(Message::Info("No hay nada elegido".to_owned()), cx);
        }
        let ids = self.selected.clone();
        self.edit(
            |board| {
                for node in board.nodes.iter_mut().filter(|node| ids.contains(&node.id)) {
                    node.width = node.width.saturating_add(width).max(GRID);
                    node.height = node.height.saturating_add(height).max(GRID);
                }
            },
            cx,
        );
    }

    pub(super) fn show_hints(&mut self, cx: &mut Context<Self>) {
        let Some(from) = self.selected_node().map(|node| node.id.clone()) else {
            return self.tell(Message::Info("Elige el nodo de donde sale".to_owned()), cx);
        };
        let (Some(board), Some(camera), Some(area)) = (self.board(), self.camera, self.area())
        else {
            return;
        };
        let shown = camera.visible(area);
        let targets = board
            .nodes
            .iter()
            .filter(|node| !matches!(node.kind, NodeKind::Group { .. }) && node.id != from)
            .filter(|node| node.rect().intersects(&shown))
            .map(|node| node.id.clone())
            .collect::<Vec<_>>();
        match Hints::new(from, targets.into_iter()) {
            Some(hints) => self.hints = Some(hints),
            None => self.tell(Message::Info("No hay otro nodo a la vista".to_owned()), cx),
        }
        self.tool = Tool::Select;
    }

    /// Una letra con las pistas de unir a la vista.
    pub(crate) fn hint_key(&mut self, key: char, cx: &mut Context<Self>) {
        let Some(hints) = &mut self.hints else {
            return;
        };
        let reached = hints.type_key(key);
        if !hints.is_waiting() {
            let from = hints.from().to_owned();
            self.hints = None;
            if let Some(to) = reached {
                self.edit(|board| board.connect(&from, &to), cx);
            }
        }
        cx.notify();
    }
}
