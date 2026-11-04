//! La pizarra sin GUI ([09](../../docs/research/09-pizarra.md)): el formato
//! `.canvas` (JSON Canvas 1.0 con cuatro extensiones), escrito con un elemento
//! por línea para que un cambio toque solo la línea de lo que cambió, y la
//! geometría que se calcula de él: flechas y acierto de clic.

mod board;
mod edge;
mod geometry;
mod navigation;
mod node;
mod route;
mod stroke_points;

pub use board::{Board, FormatError};
pub use edge::{Edge, EdgeEnd, Route, Side, Tip};
pub use geometry::{Point, Rect, arrowhead, cubic_at};
pub use node::{Node, NodeKind, Shape};
pub use route::{EdgeGeometry, EdgePath};
pub use stroke_points::{PointsError, StrokePoint};
