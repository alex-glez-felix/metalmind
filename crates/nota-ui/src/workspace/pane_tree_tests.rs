use std::collections::HashSet;

use pretty_assertions::assert_eq;
use proptest::prelude::*;

use super::*;

fn pane(id: u32) -> PaneId {
    PaneId(id)
}

fn leaf(id: u32) -> Node {
    Node::Pane(pane(id))
}

fn split(axis: Axis, children: Vec<Node>) -> Node {
    Node::Split { axis, children }
}

/// Lo que debe cumplirse siempre, pase lo que pase.
fn check(tree: &PaneTree) -> Result<(), String> {
    check_node(tree.root(), None)?;
    let panes = tree.panes();
    let unique: HashSet<_> = panes.iter().collect();
    if unique.len() != panes.len() {
        return Err(format!("paneles repetidos: {panes:?}"));
    }
    if !panes.contains(&tree.focus()) {
        return Err(format!("el foco {} no está en {panes:?}", tree.focus()));
    }
    let area: f64 = tree.rects().iter().map(|(_, r)| r.width * r.height).sum();
    if (area - 1.0).abs() > EPSILON {
        return Err(format!("los paneles cubren {area} de la ventana"));
    }
    Ok(())
}

fn check_node(node: &Node, parent: Option<Axis>) -> Result<(), String> {
    let Node::Split { axis, children } = node else {
        return Ok(());
    };
    if children.len() < 2 {
        return Err(format!("split con {} hijos", children.len()));
    }
    if parent == Some(*axis) {
        return Err(format!("split {axis:?} dentro de otro igual"));
    }
    children
        .iter()
        .try_for_each(|child| check_node(child, Some(*axis)))
}

#[test]
fn empieza_con_un_panel_enfocado() {
    let tree = PaneTree::new();
    assert_eq!(tree.root(), &leaf(0));
    assert_eq!(tree.focus(), pane(0));
    assert_eq!(tree.next(), pane(0));
    assert_eq!(tree.neighbor(Direction::Left), None);
}

#[test]
fn dividir_pone_el_nuevo_a_la_derecha_con_el_foco() {
    let mut tree = PaneTree::new();
    assert_eq!(tree.split(Axis::Row), pane(1));
    assert_eq!(tree.root(), &split(Axis::Row, vec![leaf(0), leaf(1)]));
    assert_eq!(tree.focus(), pane(1));
}

#[test]
fn dividir_en_el_mismo_eje_agrega_un_hermano() {
    let mut tree = PaneTree::new();
    tree.split(Axis::Row);
    tree.set_focus(pane(0));
    tree.split(Axis::Row);
    assert_eq!(
        tree.root(),
        &split(Axis::Row, vec![leaf(0), leaf(2), leaf(1)])
    );
}

#[test]
fn dividir_en_otro_eje_anida() {
    let mut tree = PaneTree::new();
    tree.split(Axis::Row);
    tree.split(Axis::Column);
    assert_eq!(
        tree.root(),
        &split(
            Axis::Row,
            vec![leaf(0), split(Axis::Column, vec![leaf(1), leaf(2)])]
        )
    );
    assert_eq!(tree.panes(), [pane(0), pane(1), pane(2)]);
}

#[test]
fn cerrar_da_el_foco_al_vecino_y_colapsa() {
    let mut tree = PaneTree::new();
    tree.split(Axis::Row);
    tree.split(Axis::Column);
    assert!(tree.close(pane(2)));
    assert_eq!(tree.focus(), pane(1));
    assert_eq!(tree.root(), &split(Axis::Row, vec![leaf(0), leaf(1)]));
    tree.set_focus(pane(0));
    assert!(tree.close(pane(0)));
    assert_eq!(tree.focus(), pane(1));
    assert_eq!(tree.root(), &leaf(1));
}

#[test]
fn cerrar_otro_no_mueve_el_foco() {
    let mut tree = PaneTree::new();
    tree.split(Axis::Row);
    tree.split(Axis::Row);
    assert!(tree.close(pane(0)));
    assert_eq!(tree.focus(), pane(2));
}

#[test]
fn el_ultimo_panel_no_se_cierra() {
    let mut tree = PaneTree::new();
    assert!(!tree.close(pane(0)));
    assert!(!tree.close(pane(7)));
    assert_eq!(tree.root(), &leaf(0));
}

#[test]
fn cerrar_funde_un_split_que_queda_del_mismo_eje() {
    let mut tree = PaneTree::new();
    tree.split(Axis::Row);
    tree.split(Axis::Column);
    tree.split(Axis::Row);
    // fila [0, columna [1, fila [2, 3]]]
    assert!(tree.close(pane(1)));
    assert_eq!(
        tree.root(),
        &split(Axis::Row, vec![leaf(0), leaf(2), leaf(3)])
    );
}

#[test]
fn moverse_va_al_panel_que_toca_ese_lado() {
    let mut tree = PaneTree::new();
    tree.split(Axis::Row);
    tree.split(Axis::Column);
    // 0 a la izquierda, 1 arriba a la derecha, 2 abajo a la derecha.
    assert_eq!(tree.neighbor(Direction::Up), Some(pane(1)));
    assert_eq!(tree.neighbor(Direction::Left), Some(pane(0)));
    assert_eq!(tree.neighbor(Direction::Right), None);
    assert_eq!(tree.neighbor(Direction::Down), None);
    tree.set_focus(pane(0));
    assert_eq!(tree.neighbor(Direction::Right), Some(pane(1)));
    assert_eq!(tree.neighbor(Direction::Up), None);
}

#[test]
fn moverse_elige_el_que_comparte_mas_borde() {
    let mut tree = PaneTree::new();
    tree.split(Axis::Column);
    tree.set_focus(pane(0));
    tree.split(Axis::Row);
    tree.split(Axis::Row);
    // arriba [0, 2, 3], abajo 1 entero.
    tree.set_focus(pane(1));
    assert_eq!(tree.neighbor(Direction::Up), Some(pane(0)));
    tree.set_focus(pane(2));
    assert_eq!(tree.neighbor(Direction::Down), Some(pane(1)));
}

#[test]
fn siguiente_recorre_en_orden_y_vuelve() {
    let mut tree = PaneTree::new();
    tree.split(Axis::Row);
    tree.split(Axis::Column);
    let mut seen = Vec::new();
    for _ in 0..4 {
        let next = tree.next();
        tree.set_focus(next);
        seen.push(next);
    }
    assert_eq!(seen, [pane(0), pane(1), pane(2), pane(0)]);
}

#[derive(Clone, Debug)]
enum Op {
    Split(Axis),
    Close(usize),
    Focus(usize),
    Move(Direction),
    Next,
}

fn axis() -> impl Strategy<Value = Axis> {
    prop_oneof![Just(Axis::Row), Just(Axis::Column)]
}

fn direction() -> impl Strategy<Value = Direction> {
    prop_oneof![
        Just(Direction::Left),
        Just(Direction::Down),
        Just(Direction::Up),
        Just(Direction::Right),
    ]
}

fn op() -> impl Strategy<Value = Op> {
    prop_oneof![
        3 => axis().prop_map(Op::Split),
        2 => (0usize..16).prop_map(Op::Close),
        1 => (0usize..16).prop_map(Op::Focus),
        2 => direction().prop_map(Op::Move),
        1 => Just(Op::Next),
    ]
}

proptest! {
    #[test]
    fn el_arbol_sigue_valido_y_el_foco_existe(ops in prop::collection::vec(op(), 1..60)) {
        let mut tree = PaneTree::new();
        for op in ops {
            let before = tree.panes();
            match op {
                Op::Split(axis) => {
                    let new = tree.split(axis);
                    prop_assert!(!before.contains(&new));
                    prop_assert_eq!(tree.focus(), new);
                    prop_assert_eq!(tree.len(), before.len() + 1);
                }
                Op::Close(index) => {
                    let target = before[index % before.len()];
                    let focus = tree.focus();
                    let closed = tree.close(target);
                    prop_assert_eq!(closed, before.len() > 1);
                    if closed {
                        prop_assert!(!tree.contains(target));
                        prop_assert_eq!(tree.len(), before.len() - 1);
                        if focus != target {
                            prop_assert_eq!(tree.focus(), focus);
                        }
                    }
                }
                Op::Focus(index) => {
                    prop_assert!(tree.set_focus(before[index % before.len()]));
                }
                Op::Move(direction) => {
                    if let Some(next) = tree.neighbor(direction) {
                        prop_assert_ne!(next, tree.focus());
                        prop_assert!(tree.set_focus(next));
                    }
                }
                Op::Next => {
                    let next = tree.next();
                    prop_assert!(tree.set_focus(next));
                }
            }
            if let Err(problem) = check(&tree) {
                return Err(TestCaseError::fail(problem));
            }
        }
    }

    #[test]
    fn ir_y_volver_regresa_al_mismo_panel(ops in prop::collection::vec(op(), 1..40)) {
        let mut tree = PaneTree::new();
        for op in ops {
            if let Op::Split(axis) = op {
                tree.split(axis);
            }
        }
        let start = tree.focus();
        let opposite = [
            (Direction::Left, Direction::Right),
            (Direction::Right, Direction::Left),
            (Direction::Up, Direction::Down),
            (Direction::Down, Direction::Up),
        ];
        for (there, back) in opposite {
            let Some(next) = tree.neighbor(there) else { continue };
            tree.set_focus(next);
            let returned = tree.neighbor(back);
            prop_assert!(returned.is_some());
            tree.set_focus(start);
        }
    }
}
