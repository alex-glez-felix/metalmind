use std::fmt;

use crate::anchor::Anchors;
use crate::{BufferError, ByteOffset, Selections};

/// Edición ya aplicada, con lo necesario para deshacerla y rehacerla.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct AppliedEdit {
    pub start: ByteOffset,
    pub deleted: String,
    pub inserted: String,
}

/// Unidad de undo: todas las ediciones de un comando, en orden de aplicación.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Transaction {
    edits: Vec<AppliedEdit>,
    selections_before: Selections,
    selections_after: Selections,
    /// Las anclas del otro lado de la transacción: las de antes mientras
    /// está hecha, las de después mientras está deshecha.
    anchors: Anchors,
}

impl Transaction {
    #[must_use]
    pub fn selections_before(&self) -> &Selections {
        &self.selections_before
    }

    #[must_use]
    pub fn selections_after(&self) -> &Selections {
        &self.selections_after
    }

    pub(crate) fn edits(&self) -> &[AppliedEdit] {
        &self.edits
    }
}

/// Número de secuencia de un estado del historial, como `changenr()` en vim:
/// 0 es el texto inicial y cada transacción guardada crea el siguiente.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct HistorySeq(usize);

impl HistorySeq {
    pub const INITIAL: Self = Self(0);

    #[must_use]
    pub const fn get(self) -> usize {
        self.0
    }
}

impl fmt::Display for HistorySeq {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "#{}", self.0)
    }
}

/// Sentido en que se recorre una transacción.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Direction {
    Undo,
    Redo,
}

/// Un paso de un recorrido por el historial: la transacción que lleva al
/// estado `seq` y si se deshace o se rehace.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Step {
    pub seq: HistorySeq,
    pub direction: Direction,
}

/// Arista que lleva del padre a un estado; la raíz no tiene.
#[derive(Debug)]
struct Edge {
    parent: HistorySeq,
    transaction: Transaction,
}

#[derive(Debug)]
struct Node {
    edge: Option<Edge>,
    /// Del visitado hace más tiempo al más reciente, que es el destino de redo.
    children: Vec<HistorySeq>,
}

#[derive(Debug)]
struct OpenTransaction {
    before: Selections,
    edits: Vec<AppliedEdit>,
    anchors: Anchors,
}

/// Transacciones que se guardan como un solo paso de undo.
#[derive(Debug, Default)]
struct Group {
    /// Los grupos abiertos dentro de este, como un `:g` dentro de una macro:
    /// solo cierra el de afuera.
    nested: usize,
    /// El estado que creó el grupo, al que se suman las siguientes.
    joined: Option<HistorySeq>,
    /// Se pidió cerrar con una transacción abierta: termina al guardarla.
    ends_at_commit: bool,
}

/// Árbol de transacciones. Los nodos viven en un arena en orden de creación
/// y nunca se borran, así que el índice de cada uno es su [`HistorySeq`] y
/// un padre siempre tiene un número menor que sus hijos.
#[derive(Debug)]
pub(crate) struct History {
    nodes: Vec<Node>,
    current: HistorySeq,
    open: Option<OpenTransaction>,
    group: Option<Group>,
}

impl Default for History {
    fn default() -> Self {
        Self {
            nodes: vec![Node {
                edge: None,
                children: Vec::new(),
            }],
            current: HistorySeq::INITIAL,
            open: None,
            group: None,
        }
    }
}

impl History {
    pub fn current(&self) -> HistorySeq {
        self.current
    }

    pub fn is_open(&self) -> bool {
        self.open.is_some()
    }

    pub fn has_uncommitted_edits(&self) -> bool {
        self.open
            .as_ref()
            .is_some_and(|open| !open.edits.is_empty())
    }

    pub fn begin(&mut self, before: Selections, anchors: Anchors) -> Result<(), BufferError> {
        if self.is_open() {
            return Err(BufferError::TransactionAlreadyOpen);
        }
        self.open = Some(OpenTransaction {
            before,
            edits: Vec::new(),
            anchors,
        });
        Ok(())
    }

    pub fn begin_group(&mut self) {
        match &mut self.group {
            Some(group) => group.nested += 1,
            None => self.group = Some(Group::default()),
        }
    }

    pub fn end_group(&mut self) {
        match &mut self.group {
            Some(group) if group.nested > 0 => group.nested -= 1,
            Some(group) if self.open.is_some() => group.ends_at_commit = true,
            _ => self.group = None,
        }
    }

    pub fn ensure_open(&self) -> Result<(), BufferError> {
        if self.is_open() {
            Ok(())
        } else {
            Err(BufferError::NoOpenTransaction)
        }
    }

    pub fn record(
        &mut self,
        edits: impl IntoIterator<Item = AppliedEdit>,
    ) -> Result<(), BufferError> {
        let open = self.open.as_mut().ok_or(BufferError::NoOpenTransaction)?;
        open.edits.extend(edits);
        Ok(())
    }

    /// Cuelga la transacción del estado actual como su hijo más reciente, o
    /// la suma al estado que creó el grupo abierto. Una transacción sin
    /// ediciones no deja rastro en el historial.
    pub fn commit(&mut self, after: Selections) -> Result<(), BufferError> {
        let open = self.open.take().ok_or(BufferError::NoOpenTransaction)?;
        if !open.edits.is_empty() {
            match self.joined_transaction() {
                Some(joined) => {
                    joined.edits.extend(open.edits);
                    joined.selections_after = after;
                }
                None => self.push(open, after),
            }
        }
        if self
            .group
            .as_ref()
            .is_some_and(|group| group.ends_at_commit)
        {
            self.group = None;
        }
        Ok(())
    }

    fn joined_transaction(&mut self) -> Option<&mut Transaction> {
        let joined = self.group.as_ref()?.joined?;
        if joined != self.current {
            return None;
        }
        self.node_mut(joined)
            .edge
            .as_mut()
            .map(|edge| &mut edge.transaction)
    }

    fn push(&mut self, open: OpenTransaction, after: Selections) {
        let seq = HistorySeq(self.nodes.len());
        self.nodes.push(Node {
            edge: Some(Edge {
                parent: self.current,
                transaction: Transaction {
                    edits: open.edits,
                    selections_before: open.before,
                    selections_after: after,
                    anchors: open.anchors,
                },
            }),
            children: Vec::new(),
        });
        self.node_mut(self.current).children.push(seq);
        self.current = seq;
        if let Some(group) = &mut self.group {
            group.joined = Some(seq);
        }
    }

    pub fn transaction(&self, seq: HistorySeq) -> Option<&Transaction> {
        self.node(seq).edge.as_ref().map(|edge| &edge.transaction)
    }

    /// Guarda `anchors` en la transacción que lleva a `seq` y devuelve las
    /// que tenía.
    pub fn swap_anchors(&mut self, seq: HistorySeq, anchors: Anchors) -> Anchors {
        match self.node_mut(seq).edge.as_mut() {
            Some(edge) => std::mem::replace(&mut edge.transaction.anchors, anchors),
            None => Anchors::default(),
        }
    }

    /// Al padre del estado actual.
    pub fn undo(&mut self) -> Result<Vec<Step>, BufferError> {
        self.ensure_closed()?;
        let target = self.parent(self.current).unwrap_or(self.current);
        Ok(self.travel_to(target))
    }

    /// Al hijo visitado más recientemente, como en vim.
    pub fn redo(&mut self) -> Result<Vec<Step>, BufferError> {
        self.ensure_closed()?;
        let target = self.node(self.current).children.last().copied();
        Ok(self.travel_to(target.unwrap_or(self.current)))
    }

    /// `steps` estados atrás en orden cronológico, cruzando ramas (`g-`).
    pub fn earlier(&mut self, steps: usize) -> Result<Vec<Step>, BufferError> {
        self.ensure_closed()?;
        let target = HistorySeq(self.current.0.saturating_sub(steps));
        Ok(self.travel_to(target))
    }

    /// `steps` estados adelante en orden cronológico, cruzando ramas (`g+`).
    pub fn later(&mut self, steps: usize) -> Result<Vec<Step>, BufferError> {
        self.ensure_closed()?;
        let last = self.nodes.len() - 1;
        let target = HistorySeq(self.current.0.saturating_add(steps).min(last));
        Ok(self.travel_to(target))
    }

    /// Sube desde el estado actual hasta el ancestro común con `target` y
    /// baja hasta él. Cada hijo por el que baja pasa a ser el destino de redo.
    fn travel_to(&mut self, target: HistorySeq) -> Vec<Step> {
        if let Some(group) = &mut self.group {
            group.joined = None;
        }
        let (undone, mut redone) = self.path(self.current, target);
        redone.reverse();
        for &child in &redone {
            self.mark_visited(child);
        }
        self.current = target;
        let undone = undone.into_iter().map(|seq| (seq, Direction::Undo));
        let redone = redone.into_iter().map(|seq| (seq, Direction::Redo));
        undone
            .chain(redone)
            .map(|(seq, direction)| Step { seq, direction })
            .collect()
    }

    /// Estados a deshacer subiendo desde `from`, y a rehacer, de abajo
    /// arriba, para llegar a `to`.
    fn path(&self, mut from: HistorySeq, mut to: HistorySeq) -> (Vec<HistorySeq>, Vec<HistorySeq>) {
        let (mut undone, mut redone) = (Vec::new(), Vec::new());
        while from != to {
            // El mayor de los dos no puede ser el ancestro común: los padres
            // son siempre más viejos que sus hijos.
            let (seq, steps) = if from > to {
                (&mut from, &mut undone)
            } else {
                (&mut to, &mut redone)
            };
            let Some(parent) = self.parent(*seq) else {
                break;
            };
            steps.push(*seq);
            *seq = parent;
        }
        (undone, redone)
    }

    fn mark_visited(&mut self, child: HistorySeq) {
        let Some(parent) = self.parent(child) else {
            return;
        };
        let siblings = &mut self.node_mut(parent).children;
        siblings.retain(|&sibling| sibling != child);
        siblings.push(child);
    }

    fn parent(&self, seq: HistorySeq) -> Option<HistorySeq> {
        self.node(seq).edge.as_ref().map(|edge| edge.parent)
    }

    fn node(&self, seq: HistorySeq) -> &Node {
        &self.nodes[seq.0]
    }

    fn node_mut(&mut self, seq: HistorySeq) -> &mut Node {
        &mut self.nodes[seq.0]
    }

    fn ensure_closed(&self) -> Result<(), BufferError> {
        if self.is_open() {
            Err(BufferError::TransactionAlreadyOpen)
        } else {
            Ok(())
        }
    }
}
