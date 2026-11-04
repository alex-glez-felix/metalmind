//! El ratón sobre la pizarra: qué gesto empieza cada clic y qué escribe al
//! soltar. Arrastrar en el vacío mueve la vista; con `shift`, selecciona.

use gpui_kit::{
    Context, CursorStyle, Modifiers, MouseDownEvent, Pixels, Point as ScreenPoint,
    ScrollWheelEvent, Window, px,
};
use metalmind_board::{Corner, Node, NodeKind, Pick, Point, Rect};

use super::gesture::{Gesture, MoveNodes};
use super::{BoardView, Tool};
use crate::theme::Theme;

/// Cuánto multiplica el zoom cada píxel de la rueda con `ctrl`, como exponente.
const WHEEL_ZOOM_PER_PIXEL: f32 = 0.002;

/// Lo que hay bajo el ratón, según lo que se apretaría ahí.
#[derive(Clone, Debug, PartialEq)]
enum Under {
    Corner(String, Corner),
    Border(String),
    Inside(String),
    Edge(String),
    Nothing,
}

impl BoardView {
    /// Con las flechas, que hay que recorrer todas: solo al apretar.
    fn under(&self, point: Point, theme: &Theme) -> Under {
        let on_node = self.node_under(point, theme);
        if on_node != Under::Nothing {
            return on_node;
        }
        let (Some(board), Some(camera)) = (self.board(), self.camera) else {
            return Under::Nothing;
        };
        board
            .hit_edge(point, f32::from(theme.layout.board_edge_hit) / camera.zoom)
            .map_or(Under::Nothing, |edge| Under::Edge(edge.id.clone()))
    }

    fn node_under(&self, point: Point, theme: &Theme) -> Under {
        let (Some(board), Some(camera)) = (self.board(), self.camera) else {
            return Under::Nothing;
        };
        let zoom = camera.zoom;
        if let [id] = self.selected.as_slice()
            && let Some(node) = board.node(id)
            && let Some(corner) = node
                .rect()
                .corner_at(point, f32::from(theme.layout.board_corner_grab) / zoom)
        {
            return Under::Corner(id.clone(), corner);
        }
        match board.pick(point, f32::from(theme.layout.board_edge_grab) / zoom) {
            Some(Pick::Border(node)) => Under::Border(node.id.clone()),
            Some(Pick::Inside(node)) => Under::Inside(node.id.clone()),
            None => Under::Nothing,
        }
    }

    pub(super) fn on_left_down(
        &mut self,
        event: &MouseDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.menu = None;
        self.hints = None;
        let theme = Theme::get(cx).clone();
        if self.press_in_open_text(event.position, cx) {
            return cx.notify();
        }
        self.close_text(cx);
        // Lo que había en la caja que se cerró ya está en el archivo.
        self.sync(&theme, cx);
        let Some(point) = self.board_point(event.position) else {
            return;
        };
        let under = self.under(point, &theme);
        self.gesture = match self.tool {
            Tool::Shape(_) | Tool::Text => Some(Gesture::Draw {
                from: point,
                to: point,
                dragged: false,
            }),
            Tool::Pencil => Some(Gesture::Stroke {
                points: vec![point],
            }),
            Tool::Arrow | Tool::Select => self.press_on(under, event, point, &theme, window, cx),
        };
        cx.notify();
    }

    /// Un clic con la herramienta de elegir o la de flecha.
    fn press_on(
        &mut self,
        under: Under,
        event: &MouseDownEvent,
        point: Point,
        theme: &Theme,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<Gesture> {
        let shift = event.modifiers.shift;
        let arrow = self.tool == Tool::Arrow;
        match under {
            Under::Corner(id, corner) if !arrow => {
                let start = self.board()?.node(&id)?.frame();
                Some(Gesture::Resize {
                    id,
                    corner,
                    start,
                    now: start,
                })
            }
            Under::Corner(id, _) | Under::Border(id) | Under::Inside(id)
                if arrow && event.click_count == 1 =>
            {
                Some(link_from(id, event.position, point))
            }
            Under::Border(id) if !shift && event.click_count == 1 => {
                Some(link_from(id, event.position, point))
            }
            Under::Corner(id, _) | Under::Border(id) | Under::Inside(id) => {
                self.press_node(id, event, point, theme, window, cx)
            }
            Under::Edge(id) => {
                self.choose(id, shift);
                if event.click_count > 1
                    && let [element] = self.selected.as_slice()
                {
                    let element = element.clone();
                    self.open_text(&element, theme, cx);
                }
                None
            }
            Under::Nothing if shift => Some(Gesture::Marquee {
                from: point,
                to: point,
            }),
            Under::Nothing => {
                self.selected.clear();
                Some(Gesture::Pan {
                    last: event.position,
                })
            }
        }
    }

    fn press_node(
        &mut self,
        id: String,
        event: &MouseDownEvent,
        point: Point,
        theme: &Theme,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<Gesture> {
        if event.modifiers.shift {
            self.choose(id, true);
            return None;
        }
        if event.click_count > 1 {
            self.selected = vec![id.clone()];
            self.activate(&id, theme, window, cx);
            return None;
        }
        self.choose(id.clone(), false);
        Some(Gesture::Move(self.start_move(id, event.position, point)))
    }

    /// Doble clic: escribir en una caja o abrir una tarjeta.
    fn activate(&mut self, id: &str, theme: &Theme, window: &mut Window, cx: &mut Context<Self>) {
        let has_text = self
            .board()
            .and_then(|board| board.node(id))
            .is_some_and(|node| node.text().is_some());
        if has_text {
            self.open_text(id, theme, cx);
        } else {
            self.open_note(window, cx);
        }
    }

    /// Elegir con un clic: con `extend` suma o quita, sin él lo que ya estaba
    /// elegido sigue así, para arrastrarlo entero.
    pub(super) fn choose(&mut self, id: String, extend: bool) {
        let at = self.selected.iter().position(|chosen| *chosen == id);
        match (at, extend) {
            (Some(at), true) => {
                self.selected.remove(at);
            }
            (None, true) => self.selected.push(id),
            (None, false) => self.selected = vec![id],
            (Some(_), false) => {}
        }
    }

    fn start_move(
        &self,
        clicked: String,
        pressed: ScreenPoint<Pixels>,
        grabbed: Point,
    ) -> MoveNodes {
        let nodes: Vec<(String, i32, i32)> = self
            .selected_nodes()
            .map(|node| (node.id.clone(), node.x, node.y))
            .collect();
        let bounds = self
            .selected_nodes()
            .map(Node::rect)
            .reduce(|all, rect| all.union(&rect))
            .unwrap_or(Rect {
                x: grabbed.x,
                y: grabbed.y,
                w: 0.0,
                h: 0.0,
            });
        let shown = self
            .camera
            .zip(self.area())
            .map(|(camera, area)| camera.visible(area));
        let neighbors = self.board().map_or_else(Vec::new, |board| {
            board
                .nodes
                .iter()
                .filter(|node| !matches!(node.kind, NodeKind::Group { .. }))
                .filter(|node| !self.selected.contains(&node.id))
                .map(Node::rect)
                .filter(|rect| shown.is_none_or(|shown| rect.intersects(&shown)))
                .collect()
        });
        MoveNodes {
            pressed,
            grabbed,
            nodes,
            bounds,
            neighbors,
            dx: 0,
            dy: 0,
            guides: Vec::new(),
            dragged: false,
            clicked,
        }
    }

    pub(super) fn on_right_down(
        &mut self,
        event: &MouseDownEvent,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let theme = Theme::get(cx).clone();
        self.close_text(cx);
        self.sync(&theme, cx);
        self.hints = None;
        self.gesture = None;
        if let Some(point) = self.board_point(event.position) {
            match self.under(point, &theme) {
                Under::Corner(id, _) | Under::Border(id) | Under::Inside(id) | Under::Edge(id) => {
                    self.choose(id, false);
                }
                Under::Nothing => self.selected.clear(),
            }
        }
        self.menu = Some(event.position);
        cx.notify();
    }

    pub(super) fn update_cursor(&mut self, at: ScreenPoint<Pixels>, cx: &mut Context<Self>) {
        let cursor = self.cursor_at(at, cx);
        if cursor != self.cursor {
            self.cursor = cursor;
            cx.notify();
        }
    }

    fn cursor_at(&self, at: ScreenPoint<Pixels>, cx: &Context<Self>) -> CursorStyle {
        if self
            .text_area
            .as_ref()
            .is_some_and(|area| area.bounds.contains(&at))
        {
            return CursorStyle::IBeam;
        }
        if !matches!(self.tool, Tool::Select) {
            return CursorStyle::Crosshair;
        }
        let Some(point) = self.board_point(at) else {
            return CursorStyle::Arrow;
        };
        match self.node_under(point, Theme::get(cx)) {
            Under::Corner(_, Corner::TopLeft | Corner::BottomRight) => {
                CursorStyle::ResizeUpLeftDownRight
            }
            Under::Corner(_, Corner::TopRight | Corner::BottomLeft) => {
                CursorStyle::ResizeUpRightDownLeft
            }
            Under::Border(_) => CursorStyle::Crosshair,
            Under::Inside(_) | Under::Edge(_) | Under::Nothing => CursorStyle::Arrow,
        }
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
        let Modifiers { control, shift, .. } = event.modifiers;
        if control {
            let factor = (f32::from(delta.y) * WHEEL_ZOOM_PER_PIXEL).exp();
            if let (Some(camera), Some(bounds)) = (&mut self.camera, self.bounds) {
                let anchor = Point::new(
                    f32::from(event.position.x - bounds.origin.x),
                    f32::from(event.position.y - bounds.origin.y),
                );
                camera.set_zoom_at(camera.zoom * factor, anchor);
            }
        } else if shift && delta.x == px(0.) {
            self.pan(-f32::from(delta.y), 0.0);
        } else {
            self.pan(-f32::from(delta.x), -f32::from(delta.y));
        }
        cx.notify();
    }
}

fn link_from(from: String, pressed: ScreenPoint<Pixels>, to: Point) -> Gesture {
    Gesture::Link {
        from,
        pressed,
        to,
        dragged: false,
    }
}
