//! Lo que hacen las acciones con nombre de la pizarra: cada una, por su atajo,
//! su botón, su menú o la paleta.

use gpui_kit::{Context, Window};
use metalmind_board::{Frame, GRID, Node, NodeKind, Point, Rect, Shape, Side};
use metalmind_keymap::{BoardAction, Command, Invocation};
use metalmind_vault::VaultPath;
use metalmind_vim::escape_argument;

use super::{BoardView, Camera, Tool, ZOOM_STEP};
use crate::actions::Dispatch;
use crate::theme::Theme;
use crate::workspace::{Message, Workspace, WorkspaceError, shown_path};

const HALF: f32 = 0.5;

impl BoardView {
    pub fn run(&mut self, action: BoardAction, window: &mut Window, cx: &mut Context<Self>) {
        let theme = Theme::get(cx).clone();
        if action != BoardAction::EndTextEdit {
            self.close_text(cx);
        }
        self.menu = None;
        match action {
            BoardAction::MoveLeft => self.step(Side::Left, &theme, cx),
            BoardAction::MoveRight => self.step(Side::Right, &theme, cx),
            BoardAction::MoveUp => self.step(Side::Top, &theme, cx),
            BoardAction::MoveDown => self.step(Side::Bottom, &theme, cx),
            BoardAction::ZoomIn => self.zoom_by(ZOOM_STEP),
            BoardAction::ZoomOut => self.zoom_by(ZOOM_STEP.recip()),
            BoardAction::ZoomToActualSize => self.zoom_to(Camera::ACTUAL_SIZE),
            BoardAction::ZoomToFit => self.camera = self.fitted(&theme),
            BoardAction::SelectNeighborLeft => self.select_toward(Side::Left, &theme),
            BoardAction::SelectNeighborDown => self.select_toward(Side::Bottom, &theme),
            BoardAction::SelectNeighborUp => self.select_toward(Side::Top, &theme),
            BoardAction::SelectNeighborRight => self.select_toward(Side::Right, &theme),
            BoardAction::OpenNote => self.open_note(window, cx),
            BoardAction::ToolSelect => self.set_tool(Tool::Select),
            BoardAction::ToolRectangle => self.set_tool(Tool::Shape(Shape::Rectangle)),
            BoardAction::ToolEllipse => self.set_tool(Tool::Shape(Shape::Ellipse)),
            BoardAction::ToolDiamond => self.set_tool(Tool::Shape(Shape::Diamond)),
            BoardAction::ToolText => self.set_tool(Tool::Text),
            BoardAction::ToolArrow => self.set_tool(Tool::Arrow),
            BoardAction::ToolPencil => self.set_tool(Tool::Pencil),
            BoardAction::LinkToNode => self.show_hints(cx),
            BoardAction::Confirm => self.confirm(window, &theme, cx),
            BoardAction::EditText => self.edit_selected_text(&theme, cx),
            BoardAction::EndTextEdit => self.close_text(cx),
            BoardAction::Delete => self.delete_selected(cx),
            BoardAction::Copy => self.copy_selected(cx),
            BoardAction::Paste => self.paste(&theme, cx),
            BoardAction::Undo => self.history(Workspace::undo_board, cx),
            BoardAction::Redo => self.history(Workspace::redo_board, cx),
            BoardAction::Cancel => self.cancel(),
            BoardAction::ResizeNarrower => self.resize_selected(-GRID, 0, cx),
            BoardAction::ResizeWider => self.resize_selected(GRID, 0, cx),
            BoardAction::ResizeShorter => self.resize_selected(0, -GRID, cx),
            BoardAction::ResizeTaller => self.resize_selected(0, GRID, cx),
        }
        cx.notify();
    }

    fn set_tool(&mut self, tool: Tool) {
        self.tool = tool;
        self.hints = None;
    }

    pub(super) fn pan(&mut self, dx: f32, dy: f32) {
        if let Some(camera) = &mut self.camera {
            camera.pan(dx, dy);
        }
    }

    fn zoom_by(&mut self, factor: f32) {
        if let Some(camera) = self.camera {
            self.zoom_to(camera.zoom * factor);
        }
    }

    pub(super) fn zoom_to(&mut self, zoom: f32) {
        let area = self.area();
        if let (Some(camera), Some(area)) = (&mut self.camera, area) {
            camera.zoom_center(zoom, area);
        }
    }

    /// El centro de la vista, en la pizarra.
    pub(super) fn view_center(&self) -> Option<Point> {
        let (camera, area) = (self.camera?, self.area()?);
        Some(camera.to_board(Point::new(area.width * HALF, area.height * HALF)))
    }

    /// Corre `change` sobre el archivo como un paso del historial.
    pub(super) fn edit<T>(
        &self,
        change: impl FnOnce(&mut metalmind_board::Board) -> T,
        cx: &mut Context<Self>,
    ) -> Option<T> {
        let pane = self.pane;
        let edited = self.workspace.update(cx, |workspace, cx| {
            cx.notify();
            workspace.edit_board(pane, change)
        });
        self.keep(edited, cx)
    }

    pub(super) fn keep<T>(
        &self,
        result: Result<T, WorkspaceError>,
        cx: &mut Context<Self>,
    ) -> Option<T> {
        result
            .inspect_err(|error| {
                self.workspace
                    .update(cx, |workspace, _| workspace.report(error));
            })
            .ok()
    }

    /// `h j k l`: crear hacia ese lado con la herramienta de flecha, mover lo
    /// elegido o, sin nada elegido, mover la vista.
    fn step(&mut self, side: Side, theme: &Theme, cx: &mut Context<Self>) {
        if self.tool == Tool::Arrow
            && let Some(from) = self.selected_node().map(|node| node.id.clone())
        {
            self.tool = Tool::Select;
            let added = self
                .edit(|board| board.add_linked_box(&from, side), cx)
                .flatten();
            if let Some(id) = added {
                self.selected = vec![id];
            }
            return;
        }
        let Some(first) = self.selected_nodes().next().map(Node::frame) else {
            let step = f32::from(theme.layout.board_pan_step);
            let (dx, dy) = screen_step(side, step);
            return self.pan(dx, dy);
        };
        let (dx, dy) = grid_step(first, side);
        let ids = self.selected.clone();
        self.edit(|board| board.move_nodes(&ids, dx, dy), cx);
    }

    pub(super) fn reveal(&mut self, rect: Rect, theme: &Theme) {
        let area = self.area();
        if let (Some(camera), Some(area)) = (&mut self.camera, area) {
            camera.reveal(rect, area, f32::from(theme.layout.board_fit_margin));
        }
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
        self.selected = vec![id];
        self.reveal(rect, theme);
    }

    /// La tarjeta que abriría `board::open_note` ahora: la elegida, o la más
    /// cercana al centro de lo que se ve.
    fn target_card(&self) -> Option<&Node> {
        if !self.selected.is_empty() {
            return self.selected_node();
        }
        let (camera, area) = (self.camera?, self.area()?);
        let shown = camera.visible(area);
        self.board()?.nearest(self.view_center()?, |node| {
            is_card(node) && node.rect().intersects(&shown)
        })
    }

    /// Por el mismo camino que `:e`: deja el salto en la jumplist.
    pub(super) fn open_note(&mut self, window: &mut Window, cx: &mut Context<Self>) {
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

    pub(super) fn tell(&self, message: Message, cx: &mut Context<Self>) {
        self.workspace.update(cx, |workspace, cx| {
            workspace.show_message(message);
            cx.notify();
        });
    }
}

fn is_card(node: &Node) -> bool {
    matches!(node.kind, NodeKind::File { .. })
}

fn screen_step(side: Side, step: f32) -> (f32, f32) {
    match side {
        Side::Left => (-step, 0.0),
        Side::Right => (step, 0.0),
        Side::Top => (0.0, -step),
        Side::Bottom => (0.0, step),
    }
}

/// Lo que lleva `frame` a la siguiente línea de la rejilla hacia `side`.
fn grid_step(frame: Frame, side: Side) -> (i32, i32) {
    let toward = |at: i32, forward: bool| {
        let past = at.rem_euclid(GRID);
        match (forward, past) {
            (true, _) => GRID - past,
            (false, 0) => -GRID,
            (false, _) => -past,
        }
    };
    match side {
        Side::Left => (toward(frame.x, false), 0),
        Side::Right => (toward(frame.x, true), 0),
        Side::Top => (0, toward(frame.y, false)),
        Side::Bottom => (0, toward(frame.y, true)),
    }
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;
    use rstest::rstest;

    use super::*;

    #[rstest]
    #[case(100, Side::Right, (10, 0))]
    #[case(100, Side::Left, (-10, 0))]
    #[case(13, Side::Right, (7, 0))]
    #[case(13, Side::Left, (-3, 0))]
    #[case(-13, Side::Left, (-7, 0))]
    #[case(-13, Side::Right, (3, 0))]
    fn un_paso_va_a_la_siguiente_linea_de_la_rejilla(
        #[case] x: i32,
        #[case] side: Side,
        #[case] expected: (i32, i32),
    ) {
        let frame = Frame {
            x,
            y: 0,
            width: 10,
            height: 10,
        };
        assert_eq!(grid_step(frame, side), expected);
    }
}
