//! Ciclos de `parents`: las componentes fuertemente conexas (Tarjan, sin
//! recursión para que un grafo profundo no desborde la pila) y, de cada una,
//! un camino que da la vuelta para mostrarlo.

use std::collections::{BTreeMap, BTreeSet, HashMap, VecDeque};

use crate::tags::key::TagKey;

type Edges = BTreeMap<TagKey, Vec<TagKey>>;

/// Un camino por cada ciclo, empezando y terminando en su tag menor, en orden.
pub(crate) fn cycles(parents: &Edges) -> Vec<Vec<TagKey>> {
    let mut found: Vec<Vec<TagKey>> = components(parents)
        .into_iter()
        .filter(|component| component.len() > 1)
        .filter_map(|component| loop_through(parents, &component))
        .collect();
    found.sort();
    found
}

fn targets<'e>(edges: &'e Edges, key: &TagKey) -> &'e [TagKey] {
    edges.get(key).map_or(&[], Vec::as_slice)
}

#[derive(Clone, Copy)]
struct Visit {
    index: usize,
    low: usize,
    on_stack: bool,
}

/// Las componentes fuertemente conexas de `edges`.
fn components(edges: &Edges) -> Vec<BTreeSet<TagKey>> {
    let mut visits: HashMap<&TagKey, Visit> = HashMap::new();
    let mut stack: Vec<&TagKey> = Vec::new();
    let mut found = Vec::new();
    for root in edges.keys() {
        if visits.contains_key(root) {
            continue;
        }
        // Cada marco es un nodo y el próximo de sus vecinos por mirar.
        let mut frames: Vec<(&TagKey, usize)> = vec![(root, 0)];
        open(&mut visits, &mut stack, root);
        while let Some(frame) = frames.last_mut() {
            let node = frame.0;
            if let Some(target) = targets(edges, node).get(frame.1) {
                frame.1 += 1;
                match visits.get(target).copied() {
                    None => {
                        open(&mut visits, &mut stack, target);
                        frames.push((target, 0));
                    }
                    Some(visit) if visit.on_stack => lower(&mut visits, node, visit.index),
                    Some(_) => {}
                }
                continue;
            }
            frames.pop();
            let visit = visits[node];
            if let Some(&(parent, _)) = frames.last() {
                lower(&mut visits, parent, visit.low);
            }
            if visit.low == visit.index {
                found.push(close(&mut visits, &mut stack, node));
            }
        }
    }
    found
}

fn open<'k>(
    visits: &mut HashMap<&'k TagKey, Visit>,
    stack: &mut Vec<&'k TagKey>,
    node: &'k TagKey,
) {
    let index = visits.len();
    visits.insert(
        node,
        Visit {
            index,
            low: index,
            on_stack: true,
        },
    );
    stack.push(node);
}

fn lower(visits: &mut HashMap<&TagKey, Visit>, node: &TagKey, low: usize) {
    if let Some(visit) = visits.get_mut(node) {
        visit.low = visit.low.min(low);
    }
}

fn close(
    visits: &mut HashMap<&TagKey, Visit>,
    stack: &mut Vec<&TagKey>,
    root: &TagKey,
) -> BTreeSet<TagKey> {
    let mut component = BTreeSet::new();
    while let Some(node) = stack.pop() {
        if let Some(visit) = visits.get_mut(node) {
            visit.on_stack = false;
        }
        component.insert(node.clone());
        if node == root {
            break;
        }
    }
    component
}

/// El camino más corto que sale del tag menor de `component` y vuelve a él
/// sin salir de la componente.
fn loop_through(edges: &Edges, component: &BTreeSet<TagKey>) -> Option<Vec<TagKey>> {
    let start = component.first()?;
    let mut came_from: HashMap<&TagKey, &TagKey> = HashMap::new();
    let mut pending = VecDeque::from([start]);
    while let Some(node) = pending.pop_front() {
        for target in targets(edges, node) {
            if !component.contains(target) {
                continue;
            }
            if target == start {
                let mut path = vec![start.clone()];
                let mut at = node;
                while at != start {
                    path.push(at.clone());
                    at = came_from.get(at)?;
                }
                path[1..].reverse();
                path.push(start.clone());
                return Some(path);
            }
            if !came_from.contains_key(target) {
                came_from.insert(target, node);
                pending.push_back(target);
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;

    use super::*;

    fn edges(pairs: &[(&str, &[&str])]) -> Edges {
        pairs
            .iter()
            .map(|(from, to)| {
                (
                    TagKey::new(from),
                    to.iter().map(|to| TagKey::new(to)).collect(),
                )
            })
            .collect()
    }

    fn names(found: &[Vec<TagKey>]) -> Vec<String> {
        found
            .iter()
            .map(|path| {
                path.iter()
                    .map(TagKey::as_str)
                    .collect::<Vec<_>>()
                    .join(" → ")
            })
            .collect()
    }

    #[test]
    fn no_cycles_in_a_dag() {
        let graph = edges(&[
            ("gpui", &["rust", "ui"]),
            ("rust", &["lenguajes"]),
            ("ui", &["lenguajes"]),
        ]);
        assert_eq!(cycles(&graph), Vec::<Vec<TagKey>>::new());
    }

    #[test]
    fn two_cycles_are_reported_apart() {
        let graph = edges(&[
            ("a", &["b"]),
            ("b", &["c"]),
            ("c", &["a", "x"]),
            ("x", &["y"]),
            ("y", &["x"]),
        ]);
        assert_eq!(names(&cycles(&graph)), ["a → b → c → a", "x → y → x"]);
    }

    #[test]
    fn the_shortest_loop_through_the_first_tag() {
        let graph = edges(&[("a", &["b", "c"]), ("b", &["c"]), ("c", &["a"])]);
        assert_eq!(names(&cycles(&graph)), ["a → c → a"]);
    }

    #[test]
    fn a_deep_chain_does_not_overflow() {
        let keys: Vec<String> = (0..100_000).map(|i| format!("t{i:06}")).collect();
        let mut graph: Edges = keys
            .windows(2)
            .map(|pair| (TagKey::new(&pair[0]), vec![TagKey::new(&pair[1])]))
            .collect();
        graph.insert(
            TagKey::new(&keys[keys.len() - 1]),
            vec![TagKey::new(&keys[0])],
        );
        let found = cycles(&graph);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].len(), keys.len() + 1);
    }
}
