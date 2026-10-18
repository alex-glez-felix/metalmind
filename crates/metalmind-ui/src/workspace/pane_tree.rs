//! Árbol de splits: cada hoja es un panel y cada nodo interno reparte su
//! espacio en partes iguales, en fila o en columna. No sabe qué muestra cada
//! panel; eso lo guarda el [`Workspace`](super::Workspace).

use std::fmt;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PaneId(u32);

impl fmt::Display for PaneId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Cómo reparte un split el espacio entre sus hijos.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Axis {
    /// Lado a lado, como tras `ctrl-w v`.
    Row,
    /// Uno sobre otro, como tras `ctrl-w s`.
    Column,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Direction {
    Left,
    Down,
    Up,
    Right,
}

/// Siempre normalizado: un split tiene dos hijos o más y ninguno es un
/// split del mismo eje.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Node {
    Pane(PaneId),
    Split { axis: Axis, children: Vec<Node> },
}

/// Lo que ocupa un panel dentro de la ventana, en fracciones de `[0, 1]`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rect {
    pub left: f64,
    pub top: f64,
    pub width: f64,
    pub height: f64,
}

/// Holgura para comparar bordes que salen de dividir en partes iguales.
const EPSILON: f64 = 1e-9;

impl Rect {
    const FULL: Self = Self {
        left: 0.0,
        top: 0.0,
        width: 1.0,
        height: 1.0,
    };

    fn right(&self) -> f64 {
        self.left + self.width
    }

    fn bottom(&self) -> f64 {
        self.top + self.height
    }

    /// Cuánto comparten `self` y `other` a lo largo del borde que los separa.
    fn shared_edge(&self, other: &Self, direction: Direction) -> f64 {
        let touches = match direction {
            Direction::Left => (other.right() - self.left).abs() < EPSILON,
            Direction::Right => (other.left - self.right()).abs() < EPSILON,
            Direction::Up => (other.bottom() - self.top).abs() < EPSILON,
            Direction::Down => (other.top - self.bottom()).abs() < EPSILON,
        };
        if !touches {
            return 0.0;
        }
        match direction {
            Direction::Left | Direction::Right => {
                overlap(self.top, self.bottom(), other.top, other.bottom())
            }
            Direction::Up | Direction::Down => {
                overlap(self.left, self.right(), other.left, other.right())
            }
        }
    }
}

fn overlap(start: f64, end: f64, other_start: f64, other_end: f64) -> f64 {
    (end.min(other_end) - start.max(other_start)).max(0.0)
}

#[derive(Clone, Debug)]
pub struct PaneTree {
    root: Node,
    focus: PaneId,
    next_id: u32,
}

impl Default for PaneTree {
    fn default() -> Self {
        Self::new()
    }
}

impl PaneTree {
    /// Un solo panel, enfocado.
    #[must_use]
    pub fn new() -> Self {
        let first = PaneId(0);
        Self {
            root: Node::Pane(first),
            focus: first,
            next_id: 1,
        }
    }

    #[must_use]
    pub fn root(&self) -> &Node {
        &self.root
    }

    #[must_use]
    pub fn focus(&self) -> PaneId {
        self.focus
    }

    /// Los paneles en el orden de vim: de izquierda a derecha y de arriba
    /// abajo, el que recorre `ctrl-w w`.
    #[must_use]
    pub fn panes(&self) -> Vec<PaneId> {
        let mut panes = Vec::new();
        collect(&self.root, &mut panes);
        panes
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.panes().len()
    }

    #[must_use]
    pub fn contains(&self, pane: PaneId) -> bool {
        self.panes().contains(&pane)
    }

    /// Enfoca `pane` si existe.
    pub fn set_focus(&mut self, pane: PaneId) -> bool {
        let exists = self.contains(pane);
        if exists {
            self.focus = pane;
        }
        exists
    }

    /// Parte el panel enfocado en dos; el nuevo queda a la derecha o abajo
    /// y con el foco.
    pub fn split(&mut self, axis: Axis) -> PaneId {
        let new = PaneId(self.next_id);
        self.next_id += 1;
        let focus = self.focus;
        replace_leaf(&mut self.root, focus, &|old| Node::Split {
            axis,
            children: vec![Node::Pane(old), Node::Pane(new)],
        });
        normalize(&mut self.root);
        self.focus = new;
        new
    }

    /// Quita `pane`; el último no se puede quitar. Si tenía el foco, pasa al
    /// vecino que se queda con su espacio: el de antes o, si era el primero,
    /// el de después.
    pub fn close(&mut self, pane: PaneId) -> bool {
        if self.len() <= 1 {
            return false;
        }
        let Some(heir) = remove_leaf(&mut self.root, pane) else {
            return false;
        };
        normalize(&mut self.root);
        if self.focus == pane {
            self.focus = heir;
        }
        true
    }

    /// El panel que toca al enfocado por el lado `direction`; si hay varios,
    /// el que comparte más borde, y ante un empate el primero.
    #[must_use]
    pub fn neighbor(&self, direction: Direction) -> Option<PaneId> {
        let rects = self.rects();
        let focused = rects.iter().find(|(pane, _)| *pane == self.focus)?.1;
        let mut best: Option<(PaneId, f64)> = None;
        for &(pane, rect) in &rects {
            let shared = focused.shared_edge(&rect, direction);
            if shared > EPSILON && best.is_none_or(|(_, most)| shared > most + EPSILON) {
                best = Some((pane, shared));
            }
        }
        best.map(|(pane, _)| pane)
    }

    /// El siguiente en el orden de [`PaneTree::panes`], volviendo al primero.
    #[must_use]
    pub fn next(&self) -> PaneId {
        let panes = self.panes();
        let at = panes
            .iter()
            .position(|&pane| pane == self.focus)
            .unwrap_or(0);
        panes
            .get(at + 1)
            .or(panes.first())
            .copied()
            .unwrap_or(self.focus)
    }

    /// Dónde queda cada panel, en el orden de [`PaneTree::panes`].
    #[must_use]
    pub fn rects(&self) -> Vec<(PaneId, Rect)> {
        let mut rects = Vec::new();
        layout(&self.root, Rect::FULL, &mut rects);
        rects
    }
}

fn collect(node: &Node, panes: &mut Vec<PaneId>) {
    match node {
        Node::Pane(pane) => panes.push(*pane),
        Node::Split { children, .. } => {
            for child in children {
                collect(child, panes);
            }
        }
    }
}

fn first_leaf(node: &Node) -> Option<PaneId> {
    match node {
        Node::Pane(pane) => Some(*pane),
        Node::Split { children, .. } => children.first().and_then(first_leaf),
    }
}

fn last_leaf(node: &Node) -> Option<PaneId> {
    match node {
        Node::Pane(pane) => Some(*pane),
        Node::Split { children, .. } => children.last().and_then(last_leaf),
    }
}

fn replace_leaf(node: &mut Node, pane: PaneId, with: &impl Fn(PaneId) -> Node) -> bool {
    match node {
        Node::Pane(id) if *id == pane => {
            *node = with(pane);
            true
        }
        Node::Pane(_) => false,
        Node::Split { children, .. } => children
            .iter_mut()
            .any(|child| replace_leaf(child, pane, with)),
    }
}

/// Quita la hoja `pane` y devuelve el panel más cercano entre sus hermanos.
fn remove_leaf(node: &mut Node, pane: PaneId) -> Option<PaneId> {
    let Node::Split { children, .. } = node else {
        return None;
    };
    if let Some(at) = children.iter().position(|child| *child == Node::Pane(pane)) {
        children.remove(at);
        return match at.checked_sub(1) {
            Some(before) => children.get(before).and_then(last_leaf),
            None => children.first().and_then(first_leaf),
        };
    }
    children
        .iter_mut()
        .find_map(|child| remove_leaf(child, pane))
}

/// Un split de un solo hijo se vuelve ese hijo y un hijo del mismo eje se
/// funde en el padre.
fn normalize(node: &mut Node) {
    let Node::Split { axis, children } = node else {
        return;
    };
    let axis = *axis;
    for child in children.iter_mut() {
        normalize(child);
    }
    let mut flat = Vec::with_capacity(children.len());
    for child in children.drain(..) {
        match child {
            Node::Split {
                axis: inner,
                children: grandchildren,
            } if inner == axis => flat.extend(grandchildren),
            other => flat.push(other),
        }
    }
    if flat.len() == 1
        && let Some(only) = flat.pop()
    {
        *node = only;
    } else {
        *children = flat;
    }
}

fn layout(node: &Node, rect: Rect, rects: &mut Vec<(PaneId, Rect)>) {
    let (axis, children) = match node {
        Node::Pane(pane) => {
            rects.push((*pane, rect));
            return;
        }
        Node::Split { axis, children } => (*axis, children),
    };
    let parts = f64::from(u32::try_from(children.len()).unwrap_or(u32::MAX));
    for (index, child) in (0u32..).zip(children) {
        let index = f64::from(index);
        let part = match axis {
            Axis::Row => Rect {
                left: rect.left + rect.width * index / parts,
                width: rect.width / parts,
                ..rect
            },
            Axis::Column => Rect {
                top: rect.top + rect.height * index / parts,
                height: rect.height / parts,
                ..rect
            },
        };
        layout(child, part, rects);
    }
}

#[cfg(test)]
#[path = "pane_tree_tests.rs"]
mod tests;
