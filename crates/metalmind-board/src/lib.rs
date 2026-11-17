//! La pizarra sin GUI ([09](../../docs/research/09-pizarra.md)): el formato
//! `.canvas` (JSON Canvas 1.0 con cuatro extensiones), escrito con un elemento
//! por línea para que un cambio toque solo la línea de lo que cambió, y la
//! geometría que se calcula de él: flechas y acierto de clic. También lo que una
//! edición le hace, las líneas que cambian con ella y su texto para quien la
//! indexa.

mod align;
mod board;
mod clipboard;
mod edge;
mod edit;
mod geometry;
mod line_edits;
mod navigation;
mod node;
mod route;
mod stroke_points;
mod text;

pub use align::{Guide, Placement, place};
pub use board::{Board, FormatError};
pub use edge::{Edge, EdgeEnd, Route, Side, Tip};
pub use edit::{Corner, DEFAULT_SHAPE, DEFAULT_TEXT, Frame, GRID, snap};
pub use geometry::{Pick, Point, Rect, arrowhead, cubic_at};
pub use line_edits::{LineEdit, line_edits};
pub use node::{Node, NodeKind, Shape};
pub use route::{EdgeGeometry, EdgePath};
pub use stroke_points::{PointsError, StrokePoint};
pub use text::{BoardText, OutsidePassage, Passage, PassageKind};
