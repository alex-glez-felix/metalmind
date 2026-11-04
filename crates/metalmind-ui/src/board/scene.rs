//! Lo que se pinta de cada elemento: mallas y textos en coordenadas de la
//! pizarra (09 §3.1). Se construye al abrir y cuando cambia el archivo, no en
//! cada frame: el pan y el zoom solo transforman.

use gpui_kit::SharedString;
use lyon::tessellation::TessellationError;
use metalmind_board::{Board, NodeKind, Rect};

use super::elements::{edge_scene, node_scene};
use super::shapes::{self, Mesh};
use super::tint::Role;
use crate::theme::Theme;

/// Las medidas del tema con que se construye la escena; si cambian, se
/// reconstruye.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SceneStyle {
    pub line_width: f32,
    pub radius: f32,
    pub padding: f32,
    pub arrow_size: f32,
    pub card_accent: f32,
    pub text: f32,
    pub text_small: f32,
    pub line_height: f32,
    pub edge_label_width: f32,
}

impl SceneStyle {
    #[must_use]
    pub fn of(theme: &Theme) -> Self {
        Self {
            line_width: f32::from(theme.stroke.board_line),
            radius: f32::from(theme.radius.lg),
            padding: f32::from(theme.layout.board_padding),
            arrow_size: f32::from(theme.layout.board_arrow_size),
            card_accent: f32::from(theme.layout.board_card_accent),
            text: f32::from(theme.layout.board_text),
            text_small: f32::from(theme.layout.board_text_small),
            line_height: theme.layout.board_line_height.0,
            edge_label_width: f32::from(theme.layout.board_edge_label_width),
        }
    }

    pub(super) fn text_line(self) -> f32 {
        self.text * self.line_height
    }

    pub(super) fn small_line(self) -> f32 {
        self.text_small * self.line_height
    }
}

#[derive(Clone, Debug)]
pub struct Piece {
    pub role: Role,
    pub mesh: Mesh,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LabelKind {
    /// Dentro de una forma: centrado y partido a su ancho.
    Body,
    /// Texto suelto (`shape: none`): alineado a la izquierda.
    Free,
    /// Un `# título` suelto o en una forma.
    Heading,
    CardTitle,
    CardPath,
    Group,
    Edge,
}

#[derive(Clone, Debug)]
pub struct Label {
    pub text: SharedString,
    pub kind: LabelKind,
    /// El área donde va el texto, en coordenadas de la pizarra.
    pub area: Rect,
}

#[derive(Clone, Debug)]
pub struct ElementScene {
    pub pieces: Vec<Piece>,
    pub labels: Vec<Label>,
    /// La caja del nodo o de la flecha.
    pub outline: Rect,
    /// Todo lo que pinta, mallas y textos: decide si se ve.
    pub bounds: Rect,
}

impl ElementScene {
    pub(super) fn new(pieces: Vec<Piece>, labels: Vec<Label>, outline: Rect) -> Self {
        let bounds = pieces
            .iter()
            .filter_map(|piece| shapes::mesh_bounds(&piece.mesh))
            .chain(labels.iter().map(|label| label.area))
            .fold(outline, |bounds, part| bounds.union(&part));
        Self {
            pieces,
            labels,
            outline,
            bounds,
        }
    }

    #[must_use]
    pub fn triangles(&self) -> usize {
        self.pieces
            .iter()
            .map(|piece| piece.mesh.indices.as_chunks::<3>().0.len())
            .sum()
    }

    /// El color de su relleno y el de su borde, si los tiene.
    #[must_use]
    pub fn fill_role(&self) -> Option<Role> {
        self.pieces.iter().map(|p| p.role).find(|r| !r.is_line())
    }

    #[must_use]
    pub fn line_role(&self) -> Option<Role> {
        self.pieces.iter().map(|p| p.role).find(|r| r.is_line())
    }
}

/// La pizarra lista para pintar, en el orden en que se pinta: los grupos son
/// el fondo, y un nodo no se deja tapar por una flecha.
#[derive(Clone, Debug, Default)]
pub struct Scene {
    pub groups: Vec<ElementScene>,
    pub edges: Vec<ElementScene>,
    pub nodes: Vec<ElementScene>,
    /// Todo lo que pinta la pizarra, si pinta algo.
    pub content: Option<Rect>,
}

impl Scene {
    /// # Errors
    /// Si lyon no puede teselar un elemento.
    pub fn build(board: &Board, style: SceneStyle) -> Result<Self, TessellationError> {
        let mut scene = Self::default();
        for node in &board.nodes {
            let element = node_scene(node, style)?;
            if matches!(node.kind, NodeKind::Group { .. }) {
                scene.groups.push(element);
            } else {
                scene.nodes.push(element);
            }
        }
        for (edge, geometry) in board.edge_geometries() {
            scene.edges.push(edge_scene(edge, &geometry, style)?);
        }
        scene.content = scene
            .groups
            .iter()
            .chain(&scene.edges)
            .chain(&scene.nodes)
            .map(|element| element.bounds)
            .reduce(|content, bounds| content.union(&bounds));
        Ok(scene)
    }
}

#[cfg(test)]
#[path = "scene_tests.rs"]
mod tests;
