//! La pizarra leída sigue al documento: al cambiar de archivo, de versión o de
//! caja abierta se relee y se reconstruye solo lo que cambió.

use std::collections::HashSet;

use gpui_kit::App;
use metalmind_board::Board;

use super::{BoardView, Content, Scene, SceneStyle, Source, Tool};
use crate::theme::Theme;

impl BoardView {
    /// Vuelve a leer el documento si cambió desde la última vez. Con un gesto
    /// a medias espera a soltar: lo que se ve es lo arrastrado.
    pub(super) fn sync(&mut self, theme: &Theme, cx: &App) {
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
        if self.source.as_ref() == Some(&source) && !self.stale {
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
        self.stale = false;
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
