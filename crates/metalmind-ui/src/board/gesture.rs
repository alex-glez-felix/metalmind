//! Lo que hace el ratón mientras el botón sigue abajo. Cada gesto es un estado
//! con lo que necesita para pintarse y para escribirse al soltar: hasta
//! entonces el archivo no cambia, y un arrastre deja un solo paso en el historial.

use metalmind_board::{Corner, Frame, Guide, Point, Rect};

use gpui_kit::{Pixels, Point as ScreenPoint};

#[derive(Clone, Debug)]
pub(super) enum Gesture {
    /// Mueve la vista; lleva dónde iba el ratón.
    Pan {
        last: ScreenPoint<Pixels>,
    },
    Move(MoveNodes),
    Resize {
        id: String,
        corner: Corner,
        start: Frame,
        now: Frame,
    },
    /// Dibuja una forma o un texto suelto con la herramienta activa.
    Draw {
        from: Point,
        to: Point,
        dragged: bool,
    },
    /// Dibuja a mano; lleva los puntos por donde pasó.
    Stroke {
        points: Vec<Point>,
    },
    /// Saca una flecha de `from`; suelta en el vacío crea una caja.
    Link {
        from: String,
        pressed: ScreenPoint<Pixels>,
        to: Point,
        dragged: bool,
    },
    /// El rectángulo de selección.
    Marquee {
        from: Point,
        to: Point,
    },
}

/// Arrastrar lo elegido.
#[derive(Clone, Debug)]
pub(super) struct MoveNodes {
    pub pressed: ScreenPoint<Pixels>,
    pub grabbed: Point,
    /// Cada nodo que se mueve, con dónde estaba.
    pub nodes: Vec<(String, i32, i32)>,
    pub bounds: Rect,
    /// Con qué puede alinearse.
    pub neighbors: Vec<Rect>,
    /// Lo que lleva corrido respecto a donde estaba.
    pub dx: i32,
    pub dy: i32,
    pub guides: Vec<Guide>,
    pub dragged: bool,
    /// Se apretó sobre este sin arrastrar: si se suelta así, queda solo él.
    pub clicked: String,
}
