//! Lo que una pizarra dice con palabras, como un solo texto: lo que indexa el
//! vault, y donde reescribe un link o un tag sin saber de nodos ni de JSON.

use std::collections::HashMap;
use std::ops::Range;

use thiserror::Error;

use crate::board::Board;
use crate::node::NodeKind;

/// El texto de las cajas y la ruta de cada tarjeta, en el orden de los nodos
/// y cada uno desde su propia línea.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct BoardText {
    text: String,
    passages: Vec<Passage>,
}

/// Lo que aporta un nodo al texto de su pizarra.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Passage {
    /// Dónde está en [`BoardText::as_str`], en bytes.
    pub range: Range<usize>,
    pub kind: PassageKind,
    /// Los pasajes de los nodos que una flecha une con este, en orden.
    pub joined: Vec<usize>,
    node: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PassageKind {
    /// El texto de una caja, en markdown.
    Text,
    /// La ruta en el vault de la nota de una tarjeta.
    Card { subpath: Option<String> },
}

/// Una edición que no cabe en el texto de un solo nodo.
#[derive(Clone, Debug, PartialEq, Eq, Error)]
#[error("{0:?} no está dentro del texto de un nodo")]
pub struct OutsidePassage(pub Range<usize>);

impl BoardText {
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.text
    }

    #[must_use]
    pub fn passages(&self) -> &[Passage] {
        &self.passages
    }

    /// El pasaje que contiene `range` entero.
    fn passage_of(&self, range: &Range<usize>) -> Option<&Passage> {
        let after = self
            .passages
            .partition_point(|passage| passage.range.start <= range.start);
        let passage = self.passages.get(after.checked_sub(1)?)?;
        (range.start <= range.end && range.end <= passage.range.end).then_some(passage)
    }
}

impl Board {
    #[must_use]
    pub fn text(&self) -> BoardText {
        let mut text = String::new();
        let mut passages = Vec::new();
        let mut by_node: HashMap<&str, usize> = HashMap::new();
        for (node, written) in self.nodes.iter().enumerate() {
            let (kind, words) = match &written.kind {
                NodeKind::Text { text, .. } => (PassageKind::Text, text),
                NodeKind::File { file, subpath } => {
                    let subpath = subpath.clone();
                    (PassageKind::Card { subpath }, file)
                }
                NodeKind::Link { .. } | NodeKind::Group { .. } | NodeKind::Stroke { .. } => {
                    continue;
                }
            };
            if !passages.is_empty() {
                text.push('\n');
            }
            let start = text.len();
            text.push_str(words);
            by_node.insert(&written.id, passages.len());
            passages.push(Passage {
                range: start..text.len(),
                kind,
                joined: Vec::new(),
                node,
            });
        }
        for edge in &self.edges {
            let ends = (by_node.get(&*edge.from.node), by_node.get(&*edge.to.node));
            if let (Some(&from), Some(&to)) = ends {
                passages[from].joined.push(to);
                passages[to].joined.push(from);
            }
        }
        BoardText { text, passages }
    }

    /// Cambia cada rango de [`Board::text`] por su texto nuevo. Los rangos
    /// van en orden y sin solaparse.
    ///
    /// # Errors
    /// Si un rango no cae entero en el texto de un nodo; entonces no cambia
    /// nada.
    pub fn replace_text<'e>(
        &mut self,
        edits: impl IntoIterator<Item = (Range<usize>, &'e str)>,
    ) -> Result<(), OutsidePassage> {
        let text = self.text();
        let mut placed = Vec::new();
        for (range, new) in edits {
            let inside = text
                .passage_of(&range)
                .map(|passage| {
                    let start = passage.range.start;
                    (passage.node, range.start - start..range.end - start)
                })
                .filter(|(node, inside)| {
                    self.words(*node)
                        .is_some_and(|w| w.get(inside.clone()).is_some())
                });
            let Some((node, inside)) = inside else {
                return Err(OutsidePassage(range));
            };
            placed.push((node, inside, new));
        }
        // Del final al principio: cada cambio deja en su sitio los de antes.
        for (node, inside, new) in placed.into_iter().rev() {
            if let Some(words) = self.words_mut(node) {
                words.replace_range(inside, new);
            }
        }
        Ok(())
    }

    fn words(&self, node: usize) -> Option<&str> {
        match &self.nodes.get(node)?.kind {
            NodeKind::Text { text, .. } => Some(text),
            NodeKind::File { file, .. } => Some(file),
            _ => None,
        }
    }

    fn words_mut(&mut self, node: usize) -> Option<&mut String> {
        match &mut self.nodes.get_mut(node)?.kind {
            NodeKind::Text { text, .. } => Some(text),
            NodeKind::File { file, .. } => Some(file),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;

    use super::*;

    const BOARD: &str = r##"{
	"nodes":[
		{"id":"g","type":"group","x":0,"y":0,"width":900,"height":400,"label":"[[no cuenta]]"},
		{"id":"a","type":"text","x":0,"y":0,"width":200,"height":80,"text":"ver [[plan]]\n#idea"},
		{"id":"b","type":"file","x":300,"y":0,"width":200,"height":80,"file":"notas/plan.md","subpath":"#Meta"},
		{"id":"c","type":"text","x":600,"y":0,"width":200,"height":80,"text":""},
		{"id":"d","type":"link","x":0,"y":200,"width":200,"height":80,"url":"https://ejemplo.org"},
		{"id":"e","type":"file","x":300,"y":200,"width":200,"height":80,"file":"ñu.md"}
	],
	"edges":[
		{"id":"ab","fromNode":"a","toNode":"b"},
		{"id":"eb","fromNode":"e","toNode":"b"},
		{"id":"db","fromNode":"d","toNode":"b"}
	]
}
"##;

    fn shown(text: &BoardText) -> Vec<(&str, &PassageKind, &[usize])> {
        text.passages()
            .iter()
            .map(|passage| {
                (
                    &text.as_str()[passage.range.clone()],
                    &passage.kind,
                    passage.joined.as_slice(),
                )
            })
            .collect()
    }

    #[test]
    fn el_texto_son_las_cajas_y_las_tarjetas_cada_una_en_su_linea() {
        let text = Board::parse(BOARD).unwrap().text();
        assert_eq!(text.as_str(), "ver [[plan]]\n#idea\nnotas/plan.md\n\nñu.md");
        let card = |subpath: Option<&str>| PassageKind::Card {
            subpath: subpath.map(str::to_owned),
        };
        assert_eq!(
            shown(&text),
            [
                ("ver [[plan]]\n#idea", &PassageKind::Text, &[1][..]),
                ("notas/plan.md", &card(Some("#Meta")), &[0, 3][..]),
                ("", &PassageKind::Text, &[][..]),
                ("ñu.md", &card(None), &[1][..]),
            ]
        );
        assert_eq!(Board::default().text(), BoardText::default());
    }

    #[test]
    fn reemplazar_cambia_solo_el_nodo_de_cada_rango() {
        let mut board = Board::parse(BOARD).unwrap();
        board
            .replace_text([
                (6..10, "meta"),
                (19..32, "archivo/plan.md"),
                (33..33, "nuevo"),
            ])
            .unwrap();
        assert_eq!(
            board.text().as_str(),
            "ver [[meta]]\n#idea\narchivo/plan.md\nnuevo\nñu.md"
        );
        let untouched = Board::parse(BOARD).unwrap();
        assert_eq!(board.node("g"), untouched.node("g"));
        assert_eq!(board.edges, untouched.edges);
        assert_eq!(
            board.node("b").unwrap().frame(),
            untouched.node("b").unwrap().frame()
        );
    }

    #[rstest::rstest]
    #[case::entre_dos_nodos(17..20)]
    #[case::fuera_del_texto(40..41)]
    #[case::a_media_letra(35..36)]
    #[case::al_reves(Range { start: 8, end: 6 })]
    fn un_rango_que_no_cabe_en_un_nodo_no_cambia_nada(#[case] range: Range<usize>) {
        let mut board = Board::parse(BOARD).unwrap();
        let edits = [(6..10, "meta"), (range.clone(), "x")];
        assert_eq!(board.replace_text(edits), Err(OutsidePassage(range)));
        assert_eq!(board, Board::parse(BOARD).unwrap());
    }
}
