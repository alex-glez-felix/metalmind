//! Lo que le llega a la pizarra: sus acciones con nombre, el ratón y la rueda.

use gpui_kit::{
    Context, MouseDownEvent, MouseMoveEvent, MouseUpEvent, ScrollWheelEvent, Window, px,
};
use metalmind_board::{Node, NodeKind, Point, Side};
use metalmind_keymap::{BoardAction, Command, Invocation};
use metalmind_vault::VaultPath;
use metalmind_vim::escape_argument;

use super::{BoardView, Camera, ZOOM_STEP};
use crate::actions::Dispatch;
use crate::theme::Theme;
use crate::workspace::{Message, shown_path};

/// Cuánto multiplica el zoom cada píxel de la rueda con `ctrl`, como exponente.
const WHEEL_ZOOM_PER_PIXEL: f32 = 0.002;
const HALF: f32 = 0.5;

impl BoardView {
    pub fn run(&mut self, action: BoardAction, window: &mut Window, cx: &mut Context<Self>) {
        let theme = Theme::get(cx).clone();
        let step = f32::from(theme.layout.board_pan_step);
        match action {
            BoardAction::PanLeft => self.pan(-step, 0.0),
            BoardAction::PanRight => self.pan(step, 0.0),
            BoardAction::PanUp => self.pan(0.0, -step),
            BoardAction::PanDown => self.pan(0.0, step),
            BoardAction::ZoomIn => self.zoom_by(ZOOM_STEP),
            BoardAction::ZoomOut => self.zoom_by(ZOOM_STEP.recip()),
            BoardAction::ZoomToActualSize => self.zoom_to(Camera::ACTUAL_SIZE),
            BoardAction::ZoomToFit => self.camera = self.fitted(&theme),
            BoardAction::SelectNeighborLeft => self.select_toward(Side::Left, &theme),
            BoardAction::SelectNeighborDown => self.select_toward(Side::Bottom, &theme),
            BoardAction::SelectNeighborUp => self.select_toward(Side::Top, &theme),
            BoardAction::SelectNeighborRight => self.select_toward(Side::Right, &theme),
            BoardAction::OpenNote => self.open_note(window, cx),
        }
        cx.notify();
    }

    fn pan(&mut self, dx: f32, dy: f32) {
        if let Some(camera) = &mut self.camera {
            camera.pan(dx, dy);
        }
    }

    fn zoom_by(&mut self, factor: f32) {
        if let Some(camera) = self.camera {
            self.zoom_to(camera.zoom * factor);
        }
    }

    fn zoom_to(&mut self, zoom: f32) {
        let area = self.area();
        if let (Some(camera), Some(area)) = (&mut self.camera, area) {
            camera.zoom_center(zoom, area);
        }
    }

    /// El centro de la vista, en la pizarra.
    fn view_center(&self) -> Option<Point> {
        let (camera, area) = (self.camera?, self.area()?);
        Some(camera.to_board(Point::new(area.width * HALF, area.height * HALF)))
    }

    fn select_toward(&mut self, side: Side, theme: &Theme) {
        let Some(board) = self.board() else {
            return;
        };
        let next = match (self.selected_node(), self.view_center()) {
            (Some(from), _) => board.neighbor(from, side),
            (None, Some(center)) => board.nearest(center, |_| true),
            (None, None) => None,
        };
        let Some((id, rect)) = next.map(|node| (node.id.clone(), node.rect())) else {
            return;
        };
        self.selected = Some(id);
        let area = self.area();
        if let (Some(camera), Some(area)) = (&mut self.camera, area) {
            camera.reveal(rect, area, f32::from(theme.layout.board_fit_margin));
        }
    }

    /// La tarjeta que abriría `board::open_note` ahora: la elegida, o la más
    /// cercana al centro de lo que se ve.
    fn target_card(&self) -> Option<&Node> {
        if self.selected.is_some() {
            return self.selected_node();
        }
        let (camera, area) = (self.camera?, self.area()?);
        let shown = camera.visible(area);
        self.board()?.nearest(self.view_center()?, |node| {
            is_card(node) && node.rect().intersects(&shown)
        })
    }

    /// Por el mismo camino que `:e`: deja el salto en la jumplist.
    fn open_note(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(node) = self.target_card() else {
            return self.tell(
                Message::Info("No hay una tarjeta de nota aquí".to_owned()),
                cx,
            );
        };
        let NodeKind::File { file, .. } = &node.kind else {
            return self.tell(
                Message::Info("Lo elegido no es una tarjeta de nota".to_owned()),
                cx,
            );
        };
        let path = match VaultPath::new(file) {
            Ok(path) => path,
            Err(error) => return self.tell(Message::Error(format!("{file}: {error}")), cx),
        };
        let file = shown_path(&self.session.read(cx).root().join(path.as_str()));
        let edit = Invocation {
            action: Command::Edit.into(),
            argument: Some(escape_argument(&file.to_string_lossy())),
        };
        window.dispatch_action(Box::new(Dispatch(edit)), cx);
    }

    fn tell(&self, message: Message, cx: &mut Context<Self>) {
        self.workspace.update(cx, |workspace, cx| {
            workspace.show_message(message);
            cx.notify();
        });
    }

    pub(super) fn on_left_down(
        &mut self,
        event: &MouseDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let hit = self
            .board_point(event.position)
            .zip(self.board())
            .and_then(|(point, board)| board.hit(point))
            .map(|node| node.id.clone());
        let double_click = event.click_count > 1 && hit.is_some() && hit == self.selected;
        self.selected = hit;
        self.drag = Some(event.position);
        if double_click {
            self.open_note(window, cx);
        }
        cx.notify();
    }

    pub(super) fn on_mouse_move(
        &mut self,
        event: &MouseMoveEvent,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(last) = self.drag.filter(|_| event.dragging()) else {
            return;
        };
        self.drag = Some(event.position);
        self.pan(
            f32::from(last.x - event.position.x),
            f32::from(last.y - event.position.y),
        );
        cx.notify();
    }

    pub(super) fn on_left_up(&mut self, _: &MouseUpEvent, _: &mut Window, _: &mut Context<Self>) {
        self.drag = None;
    }

    /// La rueda mueve la pizarra y, con `ctrl`, cambia el zoom bajo el ratón.
    pub(super) fn on_scroll(
        &mut self,
        event: &ScrollWheelEvent,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let theme = Theme::get(cx);
        let delta = event.delta.pixel_delta(theme.ui_line());
        if event.modifiers.control {
            let factor = (f32::from(delta.y) * WHEEL_ZOOM_PER_PIXEL).exp();
            if let (Some(camera), Some(bounds)) = (&mut self.camera, self.bounds) {
                let anchor = Point::new(
                    f32::from(event.position.x - bounds.origin.x),
                    f32::from(event.position.y - bounds.origin.y),
                );
                camera.set_zoom_at(camera.zoom * factor, anchor);
            }
        } else if event.modifiers.shift && delta.x == px(0.) {
            self.pan(-f32::from(delta.y), 0.0);
        } else {
            self.pan(-f32::from(delta.x), -f32::from(delta.y));
        }
        cx.notify();
    }
}

fn is_card(node: &Node) -> bool {
    matches!(node.kind, NodeKind::File { .. })
}
