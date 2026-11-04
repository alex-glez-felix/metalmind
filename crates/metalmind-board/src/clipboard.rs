//! Copiar y pegar elementos. El portapapeles lleva sus líneas de `.canvas`
//! (09 §4.1), así que se puede pegar en otra pizarra o en un editor de texto,
//! y lo que no son líneas de pizarra entra como una caja con ese texto.

use std::collections::HashMap;

use crate::board::Board;
use crate::edge::{Edge, RawEdge};
use crate::edit::{DEFAULT_SHAPE, Frame, GRID};
use crate::geometry::Point;
use crate::node::{Node, RawNode, Shape};

/// Lo que se corre lo pegado respecto a lo copiado.
const PASTE_OFFSET: i32 = 2 * GRID;

impl Board {
    /// Las líneas de `ids`, más las flechas que unen dos de ellos.
    #[must_use]
    pub fn copy(&self, ids: &[String]) -> String {
        let chosen = |id: &String| ids.contains(id);
        let nodes = self.nodes.iter().filter(|node| chosen(&node.id));
        let edges = self
            .edges
            .iter()
            .filter(|edge| chosen(&edge.id) || (chosen(&edge.from.node) && chosen(&edge.to.node)));
        let lines: Vec<String> = nodes.map(Node::line).chain(edges.map(Edge::line)).collect();
        lines.join("\n")
    }

    /// Pega `text` con ids nuevos y corrido: si no son líneas de pizarra, como
    /// una caja en `fallback_at`. Una flecha pegada sin uno de sus nodos se
    /// queda con el que ya estaba en la pizarra, o no entra. Da los ids nuevos.
    pub fn paste(&mut self, text: &str, fallback_at: Point) -> Vec<String> {
        let (mut nodes, mut edges) = parse_lines(text);
        if nodes.is_empty() && edges.is_empty() {
            let trimmed = text.trim();
            if trimmed.is_empty() {
                return Vec::new();
            }
            let frame = self.unstacked(Frame::centered_at(fallback_at, DEFAULT_SHAPE));
            return vec![self.add_text(frame, Shape::Rectangle, trimmed)];
        }
        let shift = self.paste_shift(&nodes);
        let mut renamed = HashMap::new();
        let mut added = Vec::new();
        for node in &mut nodes {
            let id = self.reserve_id(&mut renamed, &node.id);
            node.id.clone_from(&id);
            node.x += shift;
            node.y += shift;
            added.push(id);
        }
        self.nodes.append(&mut nodes);
        edges.retain(|edge| {
            [&edge.from.node, &edge.to.node]
                .iter()
                .all(|end| renamed.contains_key(end.as_str()) || self.node(end).is_some())
        });
        for edge in &mut edges {
            for end in [&mut edge.from, &mut edge.to] {
                if let Some(new) = renamed.get(end.node.as_str()) {
                    end.node.clone_from(new);
                }
            }
            edge.id = self.fresh_id_besides(&added);
            added.push(edge.id.clone());
        }
        self.edges.append(&mut edges);
        added
    }

    /// Un múltiplo del desplazamiento, para que pegar varias veces no deje una
    /// copia justo sobre otra.
    fn paste_shift(&self, nodes: &[Node]) -> i32 {
        let Some(first) = nodes.first() else {
            return PASTE_OFFSET;
        };
        let mut shift = PASTE_OFFSET;
        while self
            .nodes
            .iter()
            .any(|node| (node.x, node.y) == (first.x + shift, first.y + shift))
        {
            shift += PASTE_OFFSET;
        }
        shift
    }

    fn reserve_id(&self, renamed: &mut HashMap<String, String>, old: &str) -> String {
        let taken: Vec<String> = renamed.values().cloned().collect();
        let id = self.fresh_id_besides(&taken);
        renamed.insert(old.to_owned(), id.clone());
        id
    }

    fn fresh_id_besides(&self, taken: &[String]) -> String {
        loop {
            let id = self.fresh_id();
            if !taken.contains(&id) {
                return id;
            }
        }
    }
}

/// Lo que se lee de cada línea; lo demás no se tiene en cuenta. La coma final
/// llega si las líneas se copiaron del archivo.
fn parse_lines(text: &str) -> (Vec<Node>, Vec<Edge>) {
    let mut nodes = Vec::new();
    let mut edges = Vec::new();
    for line in text.lines() {
        let line = line.trim().trim_end_matches(',');
        if let Ok(raw) = serde_json::from_str::<RawNode>(line) {
            if let Ok(node) = Node::try_from(raw) {
                nodes.push(node);
            }
        } else if let Ok(raw) = serde_json::from_str::<RawEdge>(line) {
            edges.push(Edge::from(raw));
        }
    }
    (nodes, edges)
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;

    use super::*;

    const BOARD: &str = r#"{"nodes":[
        {"id":"a","type":"text","x":0,"y":0,"width":100,"height":50,"text":"uno","plugin":{"k":1}},
        {"id":"b","type":"text","x":200,"y":0,"width":100,"height":50,"text":"dos"},
        {"id":"c","type":"text","x":400,"y":0,"width":100,"height":50,"text":"tres"}
    ],"edges":[
        {"id":"ab","fromNode":"a","toNode":"b"},
        {"id":"bc","fromNode":"b","toNode":"c"}
    ]}"#;

    fn ids(names: &[&str]) -> Vec<String> {
        names.iter().map(|name| (*name).to_owned()).collect()
    }

    fn origin() -> Point {
        Point::new(0.0, 0.0)
    }

    #[test]
    fn copiar_lleva_las_lineas_y_las_flechas_entre_lo_copiado() {
        let board = Board::parse(BOARD).unwrap();
        let copied = board.copy(&ids(&["a", "b"]));
        let lines: Vec<&str> = copied.lines().collect();
        assert_eq!(lines.len(), 3);
        assert_eq!(
            lines[0],
            r#"{"id":"a","type":"text","x":0,"y":0,"width":100,"height":50,"text":"uno","plugin":{"k":1}}"#
        );
        assert_eq!(lines[2], r#"{"id":"ab","fromNode":"a","toNode":"b"}"#);
    }

    #[test]
    fn pegar_da_ids_nuevos_corridos_y_flechas_entre_las_copias() {
        let mut board = Board::parse(BOARD).unwrap();
        let copied = board.copy(&ids(&["a", "b"]));
        let added = board.paste(&copied, origin());
        assert_eq!(added.len(), 3);
        assert!(
            !added
                .iter()
                .any(|id| ["a", "b", "ab"].contains(&id.as_str()))
        );
        let new_a = board.node(&added[0]).unwrap();
        assert_eq!((new_a.x, new_a.y), (PASTE_OFFSET, PASTE_OFFSET));
        assert_eq!(new_a.extra["plugin"], serde_json::json!({"k": 1}));
        let edge = board.edges.last().unwrap();
        assert_eq!((&edge.from.node, &edge.to.node), (&added[0], &added[1]));
        Board::parse(&board.write()).unwrap();
    }

    #[test]
    fn pegar_dos_veces_no_deja_una_copia_sobre_otra() {
        let mut board = Board::parse(BOARD).unwrap();
        let copied = board.copy(&ids(&["a"]));
        let first = board.paste(&copied, origin());
        let second = board.paste(&copied, origin());
        assert_ne!(
            board.node(&first[0]).unwrap().x,
            board.node(&second[0]).unwrap().x
        );
    }

    #[test]
    fn una_flecha_sola_se_pega_si_sus_nodos_siguen_y_no_si_no() {
        let mut board = Board::parse(BOARD).unwrap();
        let copied = board.copy(&ids(&["ab"]));
        assert_eq!(board.paste(&copied, origin()).len(), 1);
        board.remove(&ids(&["a"]));
        assert_eq!(board.paste(&copied, origin()), Vec::<String>::new());
    }

    #[test]
    fn lo_que_no_es_de_la_pizarra_entra_como_una_caja() {
        let mut board = Board::parse(BOARD).unwrap();
        let added = board.paste("  una idea\ncon dos líneas\n", Point::new(1000.0, 1000.0));
        let node = board.node(&added[0]).unwrap();
        assert_eq!(node.text(), Some("una idea\ncon dos líneas"));
        assert_eq!(board.paste("  \n", origin()), Vec::<String>::new());
    }

    #[test]
    fn las_lineas_copiadas_del_archivo_traen_su_coma() {
        let mut board = Board::default();
        let added = board.paste(
            "\t\t{\"id\":\"a\",\"type\":\"text\",\"x\":0,\"y\":0,\"width\":10,\"height\":10,\"text\":\"\"},\n",
            origin(),
        );
        assert_eq!(added.len(), 1);
    }
}
