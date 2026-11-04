//! Pintar la pizarra de un frame: la escena y, encima, lo que no está en el
//! archivo: lo elegido, el gesto a medias, las letras de unir y el texto abierto.

use gpui_kit::{App, Bounds, Pixels, Window};
use metalmind_board::{Frame as NodeFrame, Rect};

use super::gesture::Gesture;
use super::open_text::{OpenTextSnapshot, TextArea};
use super::paint::{self, Frame};
use super::{AreaSize, BoardView, plan};
use crate::theme::Theme;

impl BoardView {
    pub(super) fn paint(
        &mut self,
        bounds: Bounds<Pixels>,
        theme: &Theme,
        window: &mut Window,
        cx: &mut App,
    ) {
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
        self.paint_selection(&frame, window);
        self.paint_gesture(&frame, window);
        self.paint_hints(&frame, window, cx);
        self.paint_open_text(&frame, window, cx);
    }

    fn paint_selection(&self, frame: &Frame<'_>, window: &mut Window) {
        let (Some(board), Some(scene)) = (self.board(), self.scene()) else {
            return;
        };
        for id in &self.selected {
            if let Some(node) = board.node(id) {
                paint::paint_selection(node.rect(), frame, window);
            } else if let Some(element) = board
                .edges
                .iter()
                .position(|edge| edge.id == *id)
                .and_then(|at| scene.edges.get(at))
            {
                paint::paint_edge_selection(element, frame, window);
            }
        }
        if let [id] = self.selected.as_slice()
            && let Some(node) = board.node(id)
            && self.open.is_none()
        {
            paint::paint_handles(node.rect(), frame, window);
        }
    }

    fn paint_gesture(&self, frame: &Frame<'_>, window: &mut Window) {
        match &self.gesture {
            Some(Gesture::Move(moving)) => paint::paint_guides(&moving.guides, frame, window),
            Some(Gesture::Draw { from, to, dragged }) if *dragged => {
                paint::paint_outline(NodeFrame::between(*from, *to).rect(), frame, window);
            }
            Some(Gesture::Stroke { points }) => {
                paint::paint_stroke_preview(points, frame, window);
            }
            Some(Gesture::Marquee { from, to }) => {
                paint::paint_outline(Rect::between(*from, *to), frame, window);
            }
            Some(Gesture::Link {
                from,
                to,
                dragged: true,
                ..
            }) => {
                let Some(board) = self.board() else {
                    return;
                };
                if let Some(node) = board.node(from) {
                    paint::paint_link_preview(node.rect().center(), *to, frame, window);
                }
                if let Some(target) = board.hit(*to) {
                    paint::paint_selection(target.rect(), frame, window);
                }
            }
            _ => {}
        }
    }

    fn paint_hints(&self, frame: &Frame<'_>, window: &mut Window, cx: &mut App) {
        let (Some(hints), Some(board)) = (&self.hints, self.board()) else {
            return;
        };
        for (id, letters) in hints.visible() {
            if let Some(node) = board.node(id) {
                paint::paint_hint(letters, node.rect(), frame, window, cx);
            }
        }
    }

    fn paint_open_text(&mut self, frame: &Frame<'_>, window: &mut Window, cx: &mut App) {
        let Some(snapshot) = self.open_snapshot(cx) else {
            self.text_area = None;
            self.caret = None;
            return;
        };
        let (rows, caret) = snapshot.paint(frame, window, cx).unzip();
        self.caret = caret;
        self.text_area = rows.map(|rows| TextArea {
            bounds: frame.bounds_of(snapshot.label.area),
            rows,
        });
    }

    fn open_snapshot(&self, cx: &App) -> Option<OpenTextSnapshot> {
        let label = self.open?;
        let view = self.workspace.read(cx).view(self.pane).ok()?;
        let open = view.document.open_board_text()?;
        Some(OpenTextSnapshot {
            label,
            text: open.buffer.to_string(),
            selection: view.selection,
            mode: view.mode,
            preedit: self.preedit.clone(),
        })
    }
}
