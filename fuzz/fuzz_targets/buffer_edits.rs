//! Ediciones, undo/redo y `g-`/`g+` sobre `nota-buffer`. Cada estado del
//! historial guarda el texto que tenía al crearse, y volver a él con
//! cualquier recorrido debe dar ese mismo texto.

#![no_main]

use std::collections::HashMap;

use arbitrary::Arbitrary;
use libfuzzer_sys::fuzz_target;
use nota_buffer::{Buffer, ByteOffset, Edit, HistorySeq, Selection, Selections};

#[derive(Debug, Arbitrary)]
struct RawEdit {
    start: u16,
    len: u8,
    text: String,
}

#[derive(Debug, Arbitrary)]
enum Op {
    /// Un lote de ediciones en la transacción abierta; puede ser inválido.
    Apply(Vec<RawEdit>),
    Commit,
    Undo,
    Redo,
    Earlier(u8),
    Later(u8),
    BeginGroup,
    EndGroup,
    CreateAnchor(u16),
    MoveAnchor(u8, u16),
}

#[derive(Debug, Arbitrary)]
struct Case {
    text: String,
    ops: Vec<Op>,
}

const MAX_LEN: usize = 1 << 16;

struct State {
    buffer: Buffer,
    /// El texto de cada estado del historial.
    texts: HashMap<HistorySeq, String>,
    anchors: Vec<nota_buffer::LineAnchor>,
}

fn cursor() -> Selections {
    Selections::single(Selection::cursor(ByteOffset::ZERO))
}

fn edit(raw: &RawEdit) -> Edit {
    let start = ByteOffset::new(usize::from(raw.start));
    Edit::replace(start..start + usize::from(raw.len), raw.text.clone())
}

impl State {
    fn new(text: &str) -> Self {
        let buffer = Buffer::from(text);
        let texts = HashMap::from([(HistorySeq::INITIAL, text.to_owned())]);
        Self {
            buffer,
            texts,
            anchors: Vec::new(),
        }
    }

    fn run(&mut self, op: &Op) {
        match op {
            Op::Apply(raw) => self.apply(raw),
            Op::Commit => self.commit(),
            Op::Undo => self.travel(|buffer| buffer.undo().map(|_| ())),
            Op::Redo => self.travel(|buffer| buffer.redo().map(|_| ())),
            Op::Earlier(steps) => {
                self.travel(|buffer| buffer.earlier(usize::from(*steps)).map(|_| ()));
            }
            Op::Later(steps) => {
                self.travel(|buffer| buffer.later(usize::from(*steps)).map(|_| ()));
            }
            Op::BeginGroup => self.buffer.begin_undo_group(),
            Op::EndGroup => self.buffer.end_undo_group(),
            Op::CreateAnchor(raw) => {
                let offset = ByteOffset::new(usize::from(*raw));
                match self.buffer.create_anchor(offset) {
                    Ok(anchor) => self.anchors.push(anchor),
                    Err(_) => assert!(!self.buffer.is_char_boundary(offset)),
                }
            }
            Op::MoveAnchor(index, raw) => {
                let Some(&anchor) = self.anchors.get(usize::from(*index)) else {
                    return;
                };
                let offset = ByteOffset::new(usize::from(*raw));
                let moved = self.buffer.move_anchor(anchor, offset);
                assert_eq!(moved.is_ok(), self.buffer.is_char_boundary(offset));
            }
        }
        self.check();
    }

    fn apply(&mut self, raw: &[RawEdit]) {
        if !self.buffer.has_open_transaction() {
            assert!(self.buffer.begin_transaction(cursor()).is_ok());
        }
        let before = self.buffer.to_string();
        let edits: Vec<Edit> = raw.iter().map(edit).collect();
        if self.buffer.apply(edits.clone()).is_ok() {
            assert_eq!(self.buffer.to_string(), expected(&before, edits));
        } else {
            assert_eq!(
                self.buffer.to_string(),
                before,
                "un lote inválido cambió el texto"
            );
        }
    }

    fn commit(&mut self) {
        if self.buffer.has_open_transaction() {
            assert!(self.buffer.commit_transaction(cursor()).is_ok());
            self.texts
                .insert(self.buffer.history_seq(), self.buffer.to_string());
        }
    }

    fn travel(&mut self, step: impl FnOnce(&mut Buffer) -> Result<(), nota_buffer::BufferError>) {
        let open = self.buffer.has_open_transaction();
        let before = self.buffer.to_string();
        let moved = step(&mut self.buffer);
        if open {
            assert!(
                moved.is_err(),
                "recorrió el historial con una transacción abierta"
            );
            assert_eq!(self.buffer.to_string(), before);
            return;
        }
        assert!(moved.is_ok());
        self.assert_at_saved_text();
    }

    fn assert_at_saved_text(&self) {
        let seq = self.buffer.history_seq();
        assert_eq!(
            Some(&self.buffer.to_string()),
            self.texts.get(&seq),
            "el estado {seq} no tiene su texto"
        );
    }

    fn check(&self) {
        let text = self.buffer.to_string();
        assert_eq!(text.len(), self.buffer.len().get());
        assert_eq!(text.matches('\n').count() + 1, self.buffer.line_count());
        for &anchor in &self.anchors {
            if let Some(offset) = self.buffer.anchor_offset(anchor) {
                assert!(
                    self.buffer.is_char_boundary(offset),
                    "el ancla {anchor:?} quedó en {offset}, fuera de un límite de carácter"
                );
            }
        }
    }

    /// Deshacer todo da el texto inicial y rehacer todo, el del último estado.
    fn check_whole_history(&mut self) {
        self.commit();
        assert!(self.buffer.earlier(usize::MAX).is_ok());
        assert_eq!(self.buffer.history_seq(), HistorySeq::INITIAL);
        self.assert_at_saved_text();
        assert!(self.buffer.later(usize::MAX).is_ok());
        let last = self.texts.keys().max().copied().unwrap_or_default();
        assert_eq!(self.buffer.history_seq(), last);
        self.assert_at_saved_text();
        while self.buffer.history_seq() != HistorySeq::INITIAL {
            assert!(self.buffer.undo().is_ok());
            self.assert_at_saved_text();
        }
        loop {
            let seq = self.buffer.history_seq();
            assert!(self.buffer.redo().is_ok());
            self.assert_at_saved_text();
            if self.buffer.history_seq() == seq {
                break;
            }
        }
        self.check();
    }
}

/// Lo que debe quedar tras un lote válido, calculado sin el rope.
fn expected(before: &str, mut edits: Vec<Edit>) -> String {
    edits.sort_by_key(|edit| edit.range.start);
    let mut text = before.to_owned();
    for edit in edits.iter().rev() {
        text.replace_range(edit.range.start.get()..edit.range.end.get(), &edit.text);
    }
    text
}

fuzz_target!(|case: Case| {
    let mut state = State::new(&case.text);
    for op in &case.ops {
        state.run(op);
        if state.buffer.len().get() > MAX_LEN {
            break;
        }
    }
    state.check_whole_history();
});
