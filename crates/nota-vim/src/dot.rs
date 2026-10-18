//! Lo que `.` repite: el último cambio, como lo define vim.

use crate::find::Find;
use crate::insert::Typed;
use crate::object::Object;
use crate::{InsertAt, Operator, PasteSide};

/// Sobre qué actuó un operador, tal como se tecleó.
#[derive(Clone, Copy, Debug)]
pub(crate) enum Subject {
    Object(Object),
    /// `d;` o `d,`: al repetirse usa la búsqueda de ese momento.
    RepeatFind {
        reversed: bool,
    },
}

impl Subject {
    /// `None` si es `;` o `,` sin una búsqueda anterior.
    pub(crate) fn object(self, last_find: Option<Find>) -> Option<Object> {
        match self {
            Self::Object(object) => Some(object),
            Self::RepeatFind { reversed } => {
                last_find.map(|find| Object::Find(find.repeat(reversed)))
            }
        }
    }
}

impl From<Object> for Subject {
    fn from(object: Object) -> Self {
        Self::Object(object)
    }
}

/// La orden que empieza un cambio repetible.
#[derive(Clone, Copy, Debug)]
pub(crate) enum Change {
    /// `d` o `c` con su objeto; `x` es `dl`.
    Operate(Operator, Subject),
    Paste(PasteSide),
    Insert(InsertAt),
}

/// Un cambio que `.` sabe repetir: la orden, su count y, si pasó por Insert,
/// lo tecleado. Como en vim, se repiten las teclas y no el texto que quedó.
#[derive(Clone, Debug)]
pub(crate) struct Dot {
    pub change: Change,
    /// El total: `2d3w` se repite como `d6w`.
    pub count: Option<usize>,
    pub typed: Vec<Typed>,
}

impl Dot {
    pub(crate) fn new(change: Change, count: Option<usize>) -> Self {
        Self {
            change,
            count,
            typed: Vec::new(),
        }
    }

    pub(crate) fn record(&mut self, typed: &Typed) {
        match (self.typed.last_mut(), typed) {
            (Some(Typed::Text(last)), Typed::Text(text)) => last.push_str(text),
            _ => self.typed.push(typed.clone()),
        }
    }
}
