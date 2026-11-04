//! Lo que pasa con el botón abajo y al soltarlo: mover la vista, lo elegido o una
//! esquina, dibujar, sacar una flecha o seleccionar. Hasta soltar solo cambia lo
//! que se ve; el archivo cambia una vez.

use std::collections::HashSet;

use gpui_kit::{Context, MouseMoveEvent, MouseUpEvent, Pixels, Point as ScreenPoint, Window};
use metalmind_board::{DEFAULT_SHAPE, DEFAULT_TEXT, Frame, Point, Rect, Shape, place};

use super::gesture::{Gesture, MoveNodes};
use super::{BoardView, Content, SceneStyle, Tool};
use crate::frames::FrameClock;
use crate::theme::Theme;

impl BoardView {
    pub(super) fn on_mouse_move(
        &mut self,
        event: &MouseMoveEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !event.dragging() {
            if self.gesture.is_some() {
                self.finish_gesture(event.position, window, cx);
            }
            return self.update_cursor(event.position, cx);
        }
        let Some(gesture) = self.gesture.take() else {
            return;
        };
        FrameClock::input(cx);
        let theme = Theme::get(cx).clone();
        self.gesture = Some(self.drag(gesture, event.position, &theme));
        cx.notify();
    }

    fn drag(&mut self, gesture: Gesture, at: ScreenPoint<Pixels>, theme: &Theme) -> Gesture {
        let point = self.board_point(at);
        match (gesture, point) {
            (Gesture::Pan { last }, _) => {
                self.pan(f32::from(last.x - at.x), f32::from(last.y - at.y));
                Gesture::Pan { last: at }
            }
            (Gesture::Move(moving), Some(point)) => {
                Gesture::Move(self.drag_nodes(moving, at, point, theme))
            }
            (
                Gesture::Resize {
                    id, corner, start, ..
                },
                Some(point),
            ) => {
                let now = start.dragged(corner, point);
                self.preview_frame(&id, now, theme);
                Gesture::Resize {
                    id,
                    corner,
                    start,
                    now,
                }
            }
            (Gesture::Draw { from, dragged, .. }, Some(point)) => Gesture::Draw {
                from,
                to: point,
                dragged: dragged || self.past_threshold(from, point, theme),
            },
            (
                Gesture::Link {
                    from,
                    pressed,
                    dragged,
                    ..
                },
                Some(point),
            ) => Gesture::Link {
                from,
                pressed,
                to: point,
                dragged: dragged || screen_distance(pressed, at) >= threshold(theme),
            },
            (Gesture::Marquee { from, .. }, Some(point)) => Gesture::Marquee { from, to: point },
            (Gesture::Stroke { mut points }, Some(point)) => {
                points.push(point);
                Gesture::Stroke { points }
            }
            (gesture, None) => gesture,
        }
    }

    fn past_threshold(&self, from: Point, to: Point, theme: &Theme) -> bool {
        self.camera
            .is_some_and(|camera| from.distance(to) * camera.zoom >= threshold(theme))
    }

    fn drag_nodes(
        &mut self,
        mut moving: MoveNodes,
        at: ScreenPoint<Pixels>,
        point: Point,
        theme: &Theme,
    ) -> MoveNodes {
        moving.dragged = moving.dragged || screen_distance(moving.pressed, at) >= threshold(theme);
        let Some(camera) = self.camera.filter(|_| moving.dragged) else {
            return moving;
        };
        let raw = Rect {
            x: moving.bounds.x + point.x - moving.grabbed.x,
            y: moving.bounds.y + point.y - moving.grabbed.y,
            ..moving.bounds
        };
        let reach = f32::from(theme.layout.board_snap_distance) / camera.zoom;
        let placed = place(raw, &moving.neighbors, reach);
        #[expect(
            clippy::cast_possible_truncation,
            reason = "los nodos ya vienen de coordenadas enteras"
        )]
        let (left, top) = (moving.bounds.x as i32, moving.bounds.y as i32);
        moving.dx = placed.x - left;
        moving.dy = placed.y - top;
        moving.guides = placed.guides;
        self.preview_move(&moving, theme);
        moving
    }

    /// Lo arrastrado se mueve en la pizarra leída y en su escena, solo eso y
    /// sus flechas: el archivo cambia al soltar.
    fn preview_move(&mut self, moving: &MoveNodes, theme: &Theme) {
        let Content::Board { board, scene } = &mut self.content else {
            return;
        };
        for (id, x, y) in &moving.nodes {
            if let Some(node) = board.nodes.iter_mut().find(|node| node.id == *id) {
                node.x = x + moving.dx;
                node.y = y + moving.dy;
            }
        }
        let changed: HashSet<&str> = moving.nodes.iter().map(|(id, _, _)| id.as_str()).collect();
        if let Err(error) = scene.update_nodes(board, &changed, SceneStyle::of(theme)) {
            log::error!("no se pudo teselar lo que se mueve: {error:?}");
        }
    }

    fn preview_frame(&mut self, id: &str, frame: Frame, theme: &Theme) {
        let Content::Board { board, scene } = &mut self.content else {
            return;
        };
        if let Some(node) = board.nodes.iter_mut().find(|node| node.id == id) {
            node.set_frame(frame);
        }
        let changed = HashSet::from([id]);
        if let Err(error) = scene.update_nodes(board, &changed, SceneStyle::of(theme)) {
            log::error!("no se pudo teselar lo que cambia de tamaño: {error:?}");
        }
    }

    pub(super) fn on_left_up(
        &mut self,
        event: &MouseUpEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.finish_gesture(event.position, window, cx);
    }

    fn finish_gesture(
        &mut self,
        at: ScreenPoint<Pixels>,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(gesture) = self.gesture.take() else {
            return;
        };
        let theme = Theme::get(cx).clone();
        let point = self.board_point(at);
        self.stale = true;
        match gesture {
            Gesture::Pan { .. } => {}
            Gesture::Move(moving) => self.finish_move(moving, cx),
            Gesture::Resize { id, start, now, .. } => {
                if now != start {
                    self.edit(
                        |board| {
                            if let Some(node) = board.nodes.iter_mut().find(|node| node.id == id) {
                                node.set_frame(now);
                            }
                        },
                        cx,
                    );
                }
            }
            Gesture::Draw { from, to, dragged } => self.finish_draw(from, to, dragged, &theme, cx),
            Gesture::Link {
                from,
                dragged: false,
                ..
            } => {
                self.choose(from, false);
            }
            Gesture::Link { from, .. } => {
                if let Some(point) = point {
                    self.finish_link(&from, point, cx);
                }
            }
            Gesture::Stroke { points } => {
                let width = SceneStyle::of(&theme).line_width;
                let added = self
                    .edit(|board| board.add_stroke(&points, width), cx)
                    .flatten();
                self.selected = added.into_iter().collect();
            }
            Gesture::Marquee { from, .. } => {
                if let (Some(point), Some(board)) = (point, self.board()) {
                    self.selected = board.nodes_within(&Rect::between(from, point));
                }
            }
        }
        cx.notify();
    }

    fn finish_move(&mut self, moving: MoveNodes, cx: &mut Context<Self>) {
        if !moving.dragged {
            self.selected = vec![moving.clicked];
            return;
        }
        if (moving.dx, moving.dy) == (0, 0) {
            return;
        }
        let ids: Vec<String> = moving.nodes.iter().map(|(id, _, _)| id.clone()).collect();
        self.edit(|board| board.move_nodes(&ids, moving.dx, moving.dy), cx);
    }

    fn finish_draw(
        &mut self,
        from: Point,
        to: Point,
        dragged: bool,
        theme: &Theme,
        cx: &mut Context<Self>,
    ) {
        match self.tool {
            Tool::Shape(shape) => {
                let frame = if dragged {
                    Frame::between(from, to)
                } else {
                    Frame::centered_at(from, DEFAULT_SHAPE)
                };
                self.create_shape(shape, frame, cx);
            }
            Tool::Text => {
                let frame = if dragged {
                    Frame::between(from, to)
                } else {
                    Frame::centered_at(from, DEFAULT_TEXT)
                };
                self.create_text(frame, theme, cx);
            }
            Tool::Select | Tool::Arrow | Tool::Pencil => {}
        }
    }

    /// Soltar una flecha: sobre otro nodo los une, en el vacío crea una caja
    /// ahí, y sobre el mismo nodo no hace nada.
    fn finish_link(&mut self, from: &str, point: Point, cx: &mut Context<Self>) {
        self.tool = Tool::Select;
        let target = self
            .board()
            .and_then(|board| board.hit(point))
            .map(|node| node.id.clone());
        match target {
            Some(to) if to == from => {}
            Some(to) => {
                self.edit(|board| board.connect(from, &to), cx);
            }
            None => {
                let added = self
                    .edit(
                        |board| {
                            let frame = board.unstacked(Frame::centered_at(point, DEFAULT_SHAPE));
                            let id = board.add_text(frame, Shape::Rectangle, "");
                            board.connect(from, &id).map(|_| id)
                        },
                        cx,
                    )
                    .flatten();
                if let Some(id) = added {
                    self.selected = vec![id];
                }
            }
        }
    }
}

fn threshold(theme: &Theme) -> f32 {
    f32::from(theme.layout.board_drag_threshold)
}

fn screen_distance(a: ScreenPoint<Pixels>, b: ScreenPoint<Pixels>) -> f32 {
    f32::from(a.x - b.x).hypot(f32::from(a.y - b.y))
}
