use crate::{
    CharSearch, Motion, Operator, Place, ScreenMotion, Scroll, Sideways, TextObject, VisualKind,
};

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
    /// `f`, `F`, `t`, `T`: el carácter que buscan es la tecla siguiente.
    FindChar(CharSearch),
    /// `;`, o `,` si `reversed`: repite el último [`Action::FindChar`].
    RepeatFind {
        reversed: bool,
    },
    TextObject(TextObject),
    /// `/` o `?`: abre la línea donde se escribe el patrón.
    Search {
        backward: bool,
    },
    /// `n`, o `N` si `reversed`: repite la última búsqueda.
    SearchNext {
        reversed: bool,
    },
    /// `*` y `#`, o `g*` y `g#` sin `whole_word`: busca la palabra bajo el
    /// cursor.
    SearchWord {
        backward: bool,
        whole_word: bool,
    },
    /// `:noh`: deja de resaltar la búsqueda hasta la próxima.
    ClearSearchHighlight,
    /// Mueve la ventana y, si hace falta, el cursor.
    Scroll(Scroll),
    /// `zh`, `zl`, `zH`, `zL`, `zs`, `ze`: corre la ventana a lo ancho, sin
    /// `wrap`, y el cursor si queda fuera.
    ScrollSideways(Sideways),
    /// `zt`, `zz`, `zb`: mueve la ventana para que el cursor quede en ese
    /// lugar; con `first_non_blank` (`z<CR>`, `z.`, `z-`) el cursor va
    /// además al primer no blanco.
    Reposition {
        place: Place,
        first_non_blank: bool,
    },
    Operator(Operator),
    /// `"`: lo siguiente usa el registro que nombre la tecla siguiente.
    SelectRegister,
    /// El objeto de `dd`, `cc` y `yy`.
    CurrentLine,
    /// `v` o `V`: entra a Visual, cambia de tipo o, si ya es de ese tipo,
    /// sale.
    Visual(VisualKind),
    /// `o` en Visual: el cursor pasa al otro extremo de la selección.
    OtherEnd,
    /// `O` en Visual: en un bloque, el cursor pasa a la otra esquina de su
    /// misma línea; si no, como [`Action::OtherEnd`].
    OtherCorner,
    /// `I`, o `A` si `append`, en Visual por bloque: escribe antes o
    /// después del bloque, y al salir de Insert lo escrito se repite en cada
    /// línea.
    BlockInsert {
        append: bool,
    },
    /// `gv`: vuelve a la última selección; en Visual, la intercambia con la
    /// actual.
    RestoreVisual,
    /// `r` en Visual: cada carácter seleccionado pasa a ser la tecla
    /// siguiente.
    Replace,
    Insert(InsertAt),
    NormalMode,
    /// Como `ctrl-c` en vim: sale a Normal igual que [`Action::NormalMode`],
    /// pero sin repetir lo escrito con el count del insert.
    Interrupt,
    DeleteRight,
    Paste(PasteSide),
    /// `.`: repite el último cambio.
    RepeatChange,
    /// `q`: graba en el registro que nombre la tecla siguiente, o termina
    /// de grabar.
    RecordMacro,
    /// `@`: reproduce el registro que nombre la tecla siguiente; `@@`, el
    /// último reproducido.
    PlayMacro,
    /// `m`: pone en el cursor la mark que nombre la tecla siguiente.
    SetMark,
    /// `'`, o `` ` `` si `exact`: va a la mark que nombre la tecla
    /// siguiente. Con un operador pendiente es su motion.
    GoToMark {
        exact: bool,
    },
    /// `ctrl-o`: la entrada anterior de la jumplist.
    JumpOlder,
    /// `ctrl-i`: la siguiente.
    JumpNewer,
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
    /// En la línea de comandos, la entrada anterior del historial que
    /// empieza con lo escrito.
    HistoryPrevious,
    HistoryNext,
    /// Marca o desmarca el checkbox de un ítem de lista (`- [ ]` ↔
    /// `- [x]`), o le pone uno; con count, en tantas líneas.
    ToggleCheckbox,
    /// Alinea las columnas de la tabla del cursor (`:tableformat`).
    FormatTable,
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
        Self::Motion(Motion::Window(Place::Top)),
        Self::Motion(Motion::Window(Place::Middle)),
        Self::Motion(Motion::Window(Place::Bottom)),
        Self::Motion(Motion::Matching),
        Self::Motion(Motion::NextHeading),
        Self::Motion(Motion::PreviousHeading),
        Self::Motion(Motion::Screen(ScreenMotion::Down)),
        Self::Motion(Motion::Screen(ScreenMotion::Up)),
        Self::Motion(Motion::Screen(ScreenMotion::RowStart)),
        Self::Motion(Motion::Screen(ScreenMotion::RowFirstNonBlank)),
        Self::Motion(Motion::Screen(ScreenMotion::RowMiddle)),
        Self::Motion(Motion::Screen(ScreenMotion::RowEnd)),
        Self::ScrollSideways(Sideways::Left),
        Self::ScrollSideways(Sideways::Right),
        Self::ScrollSideways(Sideways::HalfLeft),
        Self::ScrollSideways(Sideways::HalfRight),
        Self::ScrollSideways(Sideways::CursorStart),
        Self::ScrollSideways(Sideways::CursorEnd),
        Self::Scroll(Scroll::LineDown),
        Self::Scroll(Scroll::LineUp),
        Self::Scroll(Scroll::HalfPageDown),
        Self::Scroll(Scroll::HalfPageUp),
        Self::Scroll(Scroll::PageDown),
        Self::Scroll(Scroll::PageUp),
        Self::Reposition {
            place: Place::Top,
            first_non_blank: false,
        },
        Self::Reposition {
            place: Place::Middle,
            first_non_blank: false,
        },
        Self::Reposition {
            place: Place::Bottom,
            first_non_blank: false,
        },
        Self::Reposition {
            place: Place::Top,
            first_non_blank: true,
        },
        Self::Reposition {
            place: Place::Middle,
            first_non_blank: true,
        },
        Self::Reposition {
            place: Place::Bottom,
            first_non_blank: true,
        },
        Self::FindChar(CharSearch {
            backward: false,
            till: false,
        }),
        Self::FindChar(CharSearch {
            backward: true,
            till: false,
        }),
        Self::FindChar(CharSearch {
            backward: false,
            till: true,
        }),
        Self::FindChar(CharSearch {
            backward: true,
            till: true,
        }),
        Self::RepeatFind { reversed: false },
        Self::RepeatFind { reversed: true },
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
        Self::TextObject(TextObject::Paragraph { around: false }),
        Self::TextObject(TextObject::Paragraph { around: true }),
        Self::TextObject(TextObject::Section { around: false }),
        Self::TextObject(TextObject::Section { around: true }),
        Self::Search { backward: false },
        Self::Search { backward: true },
        Self::SearchNext { reversed: false },
        Self::SearchNext { reversed: true },
        Self::SearchWord {
            backward: false,
            whole_word: true,
        },
        Self::SearchWord {
            backward: true,
            whole_word: true,
        },
        Self::SearchWord {
            backward: false,
            whole_word: false,
        },
        Self::SearchWord {
            backward: true,
            whole_word: false,
        },
        Self::ClearSearchHighlight,
        Self::Operator(Operator::Delete),
        Self::Operator(Operator::Change),
        Self::Operator(Operator::Yank),
        Self::Operator(Operator::Lowercase),
        Self::Operator(Operator::Uppercase),
        Self::Operator(Operator::ToggleCase),
        Self::Operator(Operator::Indent),
        Self::Operator(Operator::Outdent),
        Self::SelectRegister,
        Self::CurrentLine,
        Self::Visual(VisualKind::Char),
        Self::Visual(VisualKind::Line),
        Self::Visual(VisualKind::Block),
        Self::OtherEnd,
        Self::OtherCorner,
        Self::BlockInsert { append: false },
        Self::BlockInsert { append: true },
        Self::RestoreVisual,
        Self::Replace,
        Self::Insert(InsertAt::Before),
        Self::Insert(InsertAt::After),
        Self::Insert(InsertAt::FirstNonBlank),
        Self::Insert(InsertAt::LineEnd),
        Self::Insert(InsertAt::LineBelow),
        Self::Insert(InsertAt::LineAbove),
        Self::NormalMode,
        Self::Interrupt,
        Self::DeleteRight,
        Self::Paste(PasteSide::After),
        Self::Paste(PasteSide::Before),
        Self::RepeatChange,
        Self::RecordMacro,
        Self::PlayMacro,
        Self::SetMark,
        Self::GoToMark { exact: false },
        Self::GoToMark { exact: true },
        Self::JumpOlder,
        Self::JumpNewer,
        Self::Undo,
        Self::Redo,
        Self::Earlier,
        Self::Later,
        Self::Backspace,
        Self::Newline,
        Self::CommandLine,
        Self::SubmitCommand,
        Self::HistoryPrevious,
        Self::HistoryNext,
        Self::ToggleCheckbox,
        Self::FormatTable,
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
            Self::FindChar(search) => find_name(search),
            Self::RepeatFind { reversed: false } => "vim::repeat_find",
            Self::RepeatFind { reversed: true } => "vim::repeat_find_reversed",
            Self::TextObject(object) => text_object_name(object),
            Self::Search { backward: false } => "vim::search_forward",
            Self::Search { backward: true } => "vim::search_backward",
            Self::SearchNext { reversed: false } => "vim::search_next",
            Self::SearchNext { reversed: true } => "vim::search_previous",
            Self::SearchWord {
                backward,
                whole_word,
            } => search_word_name(backward, whole_word),
            Self::ClearSearchHighlight => "vim::clear_search_highlight",
            Self::Scroll(scroll) => scroll_name(scroll),
            Self::ScrollSideways(sideways) => sideways_name(sideways),
            Self::Reposition {
                place,
                first_non_blank,
            } => reposition_name(place, first_non_blank),
            Self::Operator(operator) => operator_name(operator),
            Self::SelectRegister => "vim::select_register",
            Self::CurrentLine => "vim::current_line",
            Self::Visual(VisualKind::Char) => "vim::toggle_visual",
            Self::Visual(VisualKind::Line) => "vim::toggle_visual_line",
            Self::Visual(VisualKind::Block) => "vim::toggle_visual_block",
            Self::OtherEnd => "vim::other_end",
            Self::OtherCorner => "vim::other_corner",
            Self::BlockInsert { append: false } => "vim::insert_in_block",
            Self::BlockInsert { append: true } => "vim::append_to_block",
            Self::RestoreVisual => "vim::restore_visual_selection",
            Self::Replace => "vim::replace",
            Self::Insert(at) => insert_name(at),
            Self::NormalMode => "vim::normal_mode",
            Self::Interrupt => "vim::interrupt",
            Self::DeleteRight => "vim::delete_right",
            Self::Paste(PasteSide::After) => "vim::paste_after",
            Self::Paste(PasteSide::Before) => "vim::paste_before",
            Self::RepeatChange => "vim::repeat_change",
            Self::RecordMacro => "vim::record_macro",
            Self::PlayMacro => "vim::play_macro",
            Self::SetMark => "vim::set_mark",
            Self::GoToMark { exact: false } => "vim::go_to_mark_line",
            Self::GoToMark { exact: true } => "vim::go_to_mark",
            Self::JumpOlder => "vim::jump_older",
            Self::JumpNewer => "vim::jump_newer",
            Self::Undo => "vim::undo",
            Self::Redo => "vim::redo",
            Self::Earlier => "vim::earlier",
            Self::Later => "vim::later",
            Self::Backspace => "editor::backspace",
            Self::Newline => "editor::newline",
            Self::CommandLine => "vim::command_line",
            Self::SubmitCommand => "vim::submit_command",
            Self::HistoryPrevious => "vim::history_previous",
            Self::HistoryNext => "vim::history_next",
            Self::ToggleCheckbox => "editor::toggle_checkbox",
            Self::FormatTable => "editor::format_table",
        }
    }
}

fn operator_name(operator: Operator) -> &'static str {
    match operator {
        Operator::Delete => "vim::delete",
        Operator::Change => "vim::change",
        Operator::Yank => "vim::yank",
        Operator::Lowercase => "vim::lowercase",
        Operator::Uppercase => "vim::uppercase",
        Operator::ToggleCase => "vim::toggle_case",
        Operator::Indent => "vim::indent",
        Operator::Outdent => "vim::outdent",
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
        Motion::Window(Place::Top) => "vim::window_top",
        Motion::Window(Place::Middle) => "vim::window_middle",
        Motion::Window(Place::Bottom) => "vim::window_bottom",
        Motion::Matching => "vim::matching",
        Motion::Screen(ScreenMotion::Down) => "vim::screen_down",
        Motion::Screen(ScreenMotion::Up) => "vim::screen_up",
        Motion::Screen(ScreenMotion::RowStart) => "vim::start_of_screen_line",
        Motion::Screen(ScreenMotion::RowFirstNonBlank) => "vim::first_non_blank_of_screen_line",
        Motion::Screen(ScreenMotion::RowMiddle) => "vim::middle_of_screen_line",
        Motion::Screen(ScreenMotion::RowEnd) => "vim::end_of_screen_line",
        Motion::NextHeading => "vim::next_heading",
        Motion::PreviousHeading => "vim::previous_heading",
    }
}

fn scroll_name(scroll: Scroll) -> &'static str {
    match scroll {
        Scroll::LineDown => "vim::scroll_line_down",
        Scroll::LineUp => "vim::scroll_line_up",
        Scroll::HalfPageDown => "vim::scroll_half_page_down",
        Scroll::HalfPageUp => "vim::scroll_half_page_up",
        Scroll::PageDown => "vim::scroll_page_down",
        Scroll::PageUp => "vim::scroll_page_up",
    }
}

fn sideways_name(sideways: Sideways) -> &'static str {
    match sideways {
        Sideways::Left => "vim::scroll_left",
        Sideways::Right => "vim::scroll_right",
        Sideways::HalfLeft => "vim::scroll_half_screen_left",
        Sideways::HalfRight => "vim::scroll_half_screen_right",
        Sideways::CursorStart => "vim::scroll_cursor_start",
        Sideways::CursorEnd => "vim::scroll_cursor_end",
    }
}

fn reposition_name(place: Place, first_non_blank: bool) -> &'static str {
    match (place, first_non_blank) {
        (Place::Top, false) => "vim::scroll_cursor_top",
        (Place::Middle, false) => "vim::scroll_cursor_center",
        (Place::Bottom, false) => "vim::scroll_cursor_bottom",
        (Place::Top, true) => "vim::scroll_cursor_top_first_non_blank",
        (Place::Middle, true) => "vim::scroll_cursor_center_first_non_blank",
        (Place::Bottom, true) => "vim::scroll_cursor_bottom_first_non_blank",
    }
}

fn find_name(search: CharSearch) -> &'static str {
    match search {
        CharSearch {
            backward: false,
            till: false,
        } => "vim::find_forward",
        CharSearch {
            backward: true,
            till: false,
        } => "vim::find_backward",
        CharSearch {
            backward: false,
            till: true,
        } => "vim::till_forward",
        CharSearch {
            backward: true,
            till: true,
        } => "vim::till_backward",
    }
}

fn search_word_name(backward: bool, whole_word: bool) -> &'static str {
    match (backward, whole_word) {
        (false, true) => "vim::search_word_forward",
        (true, true) => "vim::search_word_backward",
        (false, false) => "vim::search_partial_word_forward",
        (true, false) => "vim::search_partial_word_backward",
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
        TextObject::Paragraph { around: false } => "vim::inner_paragraph",
        TextObject::Paragraph { around: true } => "vim::a_paragraph",
        TextObject::Section { around: false } => "vim::inner_section",
        TextObject::Section { around: true } => "vim::a_section",
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

    /// Lo que no está en [`Action::ALL`] no sale en la paleta.
    #[test]
    fn every_bound_action_is_listed() {
        for binding in crate::Keymap::vim().bindings() {
            if let crate::Bound::Engine(action) = binding.bound {
                assert!(Action::ALL.contains(&action), "{action:?}");
            }
        }
    }

    #[test]
    fn names_are_unique() {
        let names: HashSet<_> = Action::ALL.iter().map(|a| a.name()).collect();
        assert_eq!(names.len(), Action::ALL.len());
    }
}
