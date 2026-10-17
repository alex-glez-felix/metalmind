use crate::{Motion, Operator, TextObject};

/// Dónde empieza el modo Insert.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InsertAt {
    Before,
    After,
    FirstNonBlank,
    LineEnd,
    LineBelow,
    LineAbove,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PasteSide {
    After,
    Before,
}

/// Todo lo que el motor sabe hacer. El keymap solo traduce teclas a estas
/// acciones, siempre por nombre.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    Motion(Motion),
    TextObject(TextObject),
    Operator(Operator),
    /// El objeto de `dd`, `cc` y `yy`.
    CurrentLine,
    Insert(InsertAt),
    NormalMode,
    DeleteRight,
    Paste(PasteSide),
    Undo,
    Redo,
    /// Estado anterior en orden cronológico, cruzando ramas del historial.
    Earlier,
    /// Estado siguiente en orden cronológico, cruzando ramas del historial.
    Later,
    Backspace,
    Newline,
    /// Abre la línea de comandos (`:`).
    CommandLine,
    /// Envía lo escrito en la línea de comandos.
    SubmitCommand,
}

impl Action {
    pub const ALL: &[Self] = &[
        Self::Motion(Motion::Left),
        Self::Motion(Motion::Right),
        Self::Motion(Motion::Up),
        Self::Motion(Motion::Down),
        Self::Motion(Motion::NextWordStart { big: false }),
        Self::Motion(Motion::NextWordStart { big: true }),
        Self::Motion(Motion::NextWordEnd { big: false }),
        Self::Motion(Motion::NextWordEnd { big: true }),
        Self::Motion(Motion::PreviousWordStart { big: false }),
        Self::Motion(Motion::PreviousWordStart { big: true }),
        Self::Motion(Motion::LineStart),
        Self::Motion(Motion::FirstNonBlank),
        Self::Motion(Motion::LineEnd),
        Self::Motion(Motion::DocumentStart),
        Self::Motion(Motion::DocumentEnd),
        Self::TextObject(TextObject::Word {
            big: false,
            around: false,
        }),
        Self::TextObject(TextObject::Word {
            big: false,
            around: true,
        }),
        Self::TextObject(TextObject::Word {
            big: true,
            around: false,
        }),
        Self::TextObject(TextObject::Word {
            big: true,
            around: true,
        }),
        Self::TextObject(TextObject::Parentheses { around: false }),
        Self::TextObject(TextObject::Parentheses { around: true }),
        Self::TextObject(TextObject::DoubleQuotes { around: false }),
        Self::TextObject(TextObject::DoubleQuotes { around: true }),
        Self::Operator(Operator::Delete),
        Self::Operator(Operator::Change),
        Self::Operator(Operator::Yank),
        Self::CurrentLine,
        Self::Insert(InsertAt::Before),
        Self::Insert(InsertAt::After),
        Self::Insert(InsertAt::FirstNonBlank),
        Self::Insert(InsertAt::LineEnd),
        Self::Insert(InsertAt::LineBelow),
        Self::Insert(InsertAt::LineAbove),
        Self::NormalMode,
        Self::DeleteRight,
        Self::Paste(PasteSide::After),
        Self::Paste(PasteSide::Before),
        Self::Undo,
        Self::Redo,
        Self::Earlier,
        Self::Later,
        Self::Backspace,
        Self::Newline,
        Self::CommandLine,
        Self::SubmitCommand,
    ];

    #[must_use]
    pub fn from_name(name: &str) -> Option<Self> {
        Self::ALL
            .iter()
            .copied()
            .find(|action| action.name() == name)
    }

    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::Motion(motion) => motion_name(motion),
            Self::TextObject(object) => text_object_name(object),
            Self::Operator(Operator::Delete) => "vim::delete",
            Self::Operator(Operator::Change) => "vim::change",
            Self::Operator(Operator::Yank) => "vim::yank",
            Self::CurrentLine => "vim::current_line",
            Self::Insert(at) => insert_name(at),
            Self::NormalMode => "vim::normal_mode",
            Self::DeleteRight => "vim::delete_right",
            Self::Paste(PasteSide::After) => "vim::paste_after",
            Self::Paste(PasteSide::Before) => "vim::paste_before",
            Self::Undo => "vim::undo",
            Self::Redo => "vim::redo",
            Self::Earlier => "vim::earlier",
            Self::Later => "vim::later",
            Self::Backspace => "editor::backspace",
            Self::Newline => "editor::newline",
            Self::CommandLine => "vim::command_line",
            Self::SubmitCommand => "vim::submit_command",
        }
    }
}

fn motion_name(motion: Motion) -> &'static str {
    match motion {
        Motion::Left => "vim::left",
        Motion::Right => "vim::right",
        Motion::Up => "vim::up",
        Motion::Down => "vim::down",
        Motion::NextWordStart { big: false } => "vim::next_word_start",
        Motion::NextWordStart { big: true } => "vim::next_big_word_start",
        Motion::NextWordEnd { big: false } => "vim::next_word_end",
        Motion::NextWordEnd { big: true } => "vim::next_big_word_end",
        Motion::PreviousWordStart { big: false } => "vim::previous_word_start",
        Motion::PreviousWordStart { big: true } => "vim::previous_big_word_start",
        Motion::LineStart => "vim::start_of_line",
        Motion::FirstNonBlank => "vim::first_non_blank",
        Motion::LineEnd => "vim::end_of_line",
        Motion::DocumentStart => "vim::start_of_document",
        Motion::DocumentEnd => "vim::end_of_document",
    }
}

fn text_object_name(object: TextObject) -> &'static str {
    match object {
        TextObject::Word {
            big: false,
            around: false,
        } => "vim::inner_word",
        TextObject::Word {
            big: false,
            around: true,
        } => "vim::a_word",
        TextObject::Word {
            big: true,
            around: false,
        } => "vim::inner_big_word",
        TextObject::Word {
            big: true,
            around: true,
        } => "vim::a_big_word",
        TextObject::Parentheses { around: false } => "vim::inner_parentheses",
        TextObject::Parentheses { around: true } => "vim::a_parentheses",
        TextObject::DoubleQuotes { around: false } => "vim::inner_double_quotes",
        TextObject::DoubleQuotes { around: true } => "vim::a_double_quotes",
    }
}

fn insert_name(at: InsertAt) -> &'static str {
    match at {
        InsertAt::Before => "vim::insert_before",
        InsertAt::After => "vim::insert_after",
        InsertAt::FirstNonBlank => "vim::insert_first_non_blank",
        InsertAt::LineEnd => "vim::insert_end_of_line",
        InsertAt::LineBelow => "vim::insert_line_below",
        InsertAt::LineAbove => "vim::insert_line_above",
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::*;

    #[test]
    fn every_action_round_trips_through_its_name() {
        for &action in Action::ALL {
            assert_eq!(Action::from_name(action.name()), Some(action));
        }
    }

    #[test]
    fn names_are_unique() {
        let names: HashSet<_> = Action::ALL.iter().map(|a| a.name()).collect();
        assert_eq!(names.len(), Action::ALL.len());
    }
}
