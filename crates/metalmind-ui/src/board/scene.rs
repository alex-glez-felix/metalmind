//! Lo que se pinta de cada elemento: mallas y textos en coordenadas de la
//! pizarra (09 §3.1). Se construye al abrir y, al cambiar el archivo o al
//! arrastrar, solo lo que cambió: el pan y el zoom solo transforman.

use std::collections::HashSet;

use gpui_kit::SharedString;
use lyon::tessellation::TessellationError;
use metalmind_board::{Board, Edge, NodeKind, Rect};

use super::elements::{edge_label_area, edge_scene, node_scene};
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
        scene.measure();
        Ok(scene)
    }

    fn measure(&mut self) {
        self.content = self
            .groups
            .iter()
            .chain(&self.edges)
            .chain(&self.nodes)
            .map(|element| element.bounds)
            .reduce(|content, bounds| content.union(&bounds));
    }

    /// Reconstruye lo de `nodes` y sus flechas, y lo que dice cuánto ocupa. Es
    /// lo que cuesta cada frame de un arrastre, con la pizarra que sea.
    ///
    /// # Errors
    /// Si lyon no puede teselar un elemento.
    pub fn update_nodes(
        &mut self,
        board: &Board,
        nodes: &HashSet<&str>,
        style: SceneStyle,
    ) -> Result<(), TessellationError> {
        let mut plain = 0;
        let mut grouped = 0;
        for node in &board.nodes {
            let slot = if matches!(node.kind, NodeKind::Group { .. }) {
                grouped += 1;
                self.groups.get_mut(grouped - 1)
            } else {
                plain += 1;
                self.nodes.get_mut(plain - 1)
            };
            if nodes.contains(node.id.as_str())
                && let Some(slot) = slot
            {
                *slot = node_scene(node, style)?;
            }
        }
        for (at, edge, geometry) in board.geometries_touching(nodes) {
            if let Some(slot) = self.edges.get_mut(at) {
                *slot = edge_scene(edge, &geometry, style)?;
            }
        }
        self.measure();
        Ok(())
    }

    /// El archivo pasó de `old` a `new`: reconstruye solo lo que cambió, si
    /// siguen los mismos elementos en el mismo orden; si no, todo.
    ///
    /// # Errors
    /// Si lyon no puede teselar un elemento.
    pub fn refresh(
        &mut self,
        old: &Board,
        new: &Board,
        style: SceneStyle,
    ) -> Result<(), TessellationError> {
        if !same_elements(old, new) {
            *self = Self::build(new, style)?;
            return Ok(());
        }
        let mut touched: HashSet<&str> = old
            .nodes
            .iter()
            .zip(&new.nodes)
            .filter(|(before, after)| before != after)
            .map(|(_, after)| after.id.as_str())
            .collect();
        // Una flecha cambiada se reconstruye con cualquiera de sus nodos.
        for (before, after) in old.edges.iter().zip(&new.edges) {
            if before != after {
                touched.insert(&after.from.node);
            }
        }
        if touched.is_empty() {
            return Ok(());
        }
        self.update_nodes(new, &touched, style)
    }

    /// Lo que hay que saber para escribir sobre `element`, ya sin su texto: se
    /// pinta el del buffer abierto.
    pub(super) fn open_label(
        &mut self,
        board: &Board,
        element: &str,
        style: SceneStyle,
    ) -> Option<OpenLabel> {
        if let Some(at) = board.edges.iter().position(|edge| edge.id == element) {
            if let Some(label) = self.edges.get_mut(at)?.labels.first_mut() {
                label.text = SharedString::default();
                return Some(OpenLabel {
                    kind: label.kind,
                    area: label.area,
                });
            }
            let edge: &Edge = board.edges.get(at)?;
            let touching = HashSet::from([edge.from.node.as_str()]);
            let (_, _, geometry) = board
                .geometries_touching(&touching)
                .find(|(found, _, _)| *found == at)?;
            return Some(OpenLabel {
                kind: LabelKind::Edge,
                area: edge_label_area(&geometry, style),
            });
        }
        let at = board
            .nodes
            .iter()
            .filter(|node| !matches!(node.kind, NodeKind::Group { .. }))
            .position(|node| node.id == element)?;
        let label = self.nodes.get_mut(at)?.labels.first_mut()?;
        label.text = SharedString::default();
        Some(OpenLabel {
            kind: label.kind,
            area: label.area,
        })
    }
}

/// Dónde va el texto que se está escribiendo.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct OpenLabel {
    pub kind: LabelKind,
    pub area: Rect,
}

fn same_elements(old: &Board, new: &Board) -> bool {
    let same_node = |(a, b): (&metalmind_board::Node, &metalmind_board::Node)| {
        a.id == b.id
            && matches!(a.kind, NodeKind::Group { .. }) == matches!(b.kind, NodeKind::Group { .. })
    };
    old.nodes.len() == new.nodes.len()
        && old.edges.len() == new.edges.len()
        && old.nodes.iter().zip(&new.nodes).all(same_node)
        && old.edges.iter().zip(&new.edges).all(|(a, b)| a.id == b.id)
}

#[cfg(test)]
#[path = "scene_tests.rs"]
mod tests;
