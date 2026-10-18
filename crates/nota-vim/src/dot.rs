//! Lo que `.` repite: el último cambio, como lo define vim.

use crate::insert::Typed;
use crate::mark::MarkName;
use crate::object::Object;
use crate::register::RegisterName;
use crate::visual::VisualSize;
use crate::{InsertAt, Operator, PasteSide};

/// Sobre qué actuó un operador, tal como se tecleó.
#[derive(Clone, Debug)]
pub(crate) enum Subject {
    Object(Object),
    /// `d;` o `d,`: al repetirse usa la búsqueda de ese momento.
    RepeatFind {
        reversed: bool,
    },
    /// `dn` o `dN`: al repetirse usa el patrón de ese momento.
    SearchNext {
        reversed: bool,
    },
    /// `d'a`: al repetirse va a donde esté la mark en ese momento.
    Mark(MarkJump),
}

impl Subject {
    /// Como el `use_reg_one` de vim: lo borrado con `` ` `` o una búsqueda
    /// va a `"1` aunque no pase de una línea.
    pub(crate) fn always_numbered(&self) -> bool {
        matches!(
            self,
            Self::Mark(MarkJump { exact: true, .. })
                | Self::SearchNext { .. }
                | Self::Object(Object::Search(_) | Object::SearchWord(_))
        )
    }
}

/// `'x` o, si `exact`, `` `x ``.
#[derive(Clone, Copy, Debug)]
pub(crate) struct MarkJump {
    pub name: MarkName,
    pub exact: bool,
}

impl From<MarkJump> for Subject {
    fn from(jump: MarkJump) -> Self {
        Self::Mark(jump)
    }
}

impl From<Object> for Subject {
    fn from(object: Object) -> Self {
        Self::Object(object)
    }
}

/// La orden que empieza un cambio repetible.
#[derive(Clone, Debug)]
pub(crate) enum Change {
    /// `d` o `c` con su objeto; `x` es `dl`.
    Operate(Operator, Subject),
    Paste(PasteSide),
    Insert(InsertAt),
    /// `r` en Visual, sobre la misma cantidad de texto.
    Replace(char, VisualSize),
}

/// Un cambio que `.` sabe repetir: la orden, su count, su registro y, si
/// pasó por Insert, lo tecleado. Como en vim, se repiten las teclas y no el
/// texto que quedó.
#[derive(Clone, Debug)]
pub(crate) struct Dot {
    pub change: Change,
    /// El total: `2d3w` se repite como `d6w`.
    pub count: Option<usize>,
    pub register: Option<RegisterName>,
    pub typed: Vec<Typed>,
}

impl Dot {
    pub(crate) fn new(
        change: Change,
        count: Option<usize>,
        register: Option<RegisterName>,
    ) -> Self {
        Self {
            change,
            count,
            register,
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
