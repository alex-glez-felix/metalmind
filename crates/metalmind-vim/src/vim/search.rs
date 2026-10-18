//! `/`, `?`, `n`, `N`, `*`, `#`, `g*` y `g#`, solos, como objeto de un
//! operador o, los cuatro primeros, extendiendo una selección Visual.

use metalmind_buffer::{Buffer, Goal, Selection, Selections};

use super::Vim;
use super::operate::Operation;
use crate::dot::Subject;
use crate::history::History;
use crate::lines::clamp_normal;
use crate::object::Object;
use crate::operator::Operand;
use crate::register::{Clipboard, RegisterName};
use crate::search::{Case, Hit, Query, WordSearch, split_at_delimiter};
use crate::span::Missed;
use crate::{
    Action, KeyOutcome, Mode, Operator, Pattern, Prompt, SearchError, VimError, VisualKind,
};

/// Lo que recuerda el motor entre búsquedas.
#[derive(Debug, Default)]
pub(super) struct SearchMemory {
    /// El último patrón usado, de una búsqueda o de `:s`.
    pub pattern: Option<Pattern>,
    /// Hacia dónde fue la última búsqueda; `n` sigue igual.
    pub backward: bool,
    /// Si la UI debe resaltar `pattern`; `:noh` lo apaga hasta la próxima.
    pub highlight: bool,
    pub history: History,
}

impl SearchMemory {
    /// Lo que busca `n`.
    pub fn query(&self) -> Option<Query> {
        Some(Query {
            pattern: self.pattern.clone()?,
            backward: self.backward,
        })
    }

    pub fn remember(&mut self, query: &Query) {
        self.pattern = Some(query.pattern.clone());
        self.backward = query.backward;
        self.highlight = true;
    }
}

/// Un operador con su count y su registro, esperando a que se envíe `/` o
/// `?`; o, en Visual, la selección que la búsqueda extiende.
#[derive(Clone, Copy, Debug, Default)]
pub(super) struct AwaitingSearch {
    operator: Option<Operator>,
    count: Option<usize>,
    register: Option<RegisterName>,
    pub visual: Option<VisualKind>,
}

/// Un salto a una coincidencia: qué se buscó, cómo lo repite `.` y qué
/// esperaba la búsqueda.
struct Jump<'a> {
    pattern: &'a str,
    subject: Subject,
    awaiting: AwaitingSearch,
}

impl Vim {
    /// El patrón que la UI debe resaltar, si hay.
    #[must_use]
    pub fn search_highlight(&self) -> Option<&Pattern> {
        self.search
            .pattern
            .as_ref()
            .filter(|_| self.search.highlight)
    }

    pub(super) fn open_search(&mut self, backward: bool) {
        let awaiting = self.take_motion_context();
        self.awaiting_search = Some(awaiting);
        self.open_command_line(Prompt::Search { backward });
    }

    /// `<CR>` tras `/` o `?`. Vacío repite el último patrón en esa dirección.
    pub(super) fn submit_search(
        &mut self,
        backward: bool,
        line: &str,
        buffer: &mut Buffer,
        selections: &mut Selections,
        clipboard: &mut dyn Clipboard,
    ) -> Result<KeyOutcome, VimError> {
        let awaiting = self.awaiting_search.take().unwrap_or_default();
        if let Some(kind) = awaiting.visual {
            self.mode = Mode::Visual(kind);
        }
        self.search.history.push(line);
        let query = match self.typed_query(line, backward) {
            Ok(query) => query,
            Err(error) => return Ok(KeyOutcome::SearchFailed(error)),
        };
        self.search.remember(&query);
        let subject = Subject::Object(Object::Search(query.clone()));
        let find = |buffer: &Buffer, selection: &Selection, times| {
            query.find(buffer, selection.head, times)
        };
        let jump = Jump {
            pattern: query.pattern.source(),
            subject,
            awaiting,
        };
        self.jump_to_match(jump, find, buffer, selections, clipboard)
            .map(|outcome| outcome.unwrap_or(KeyOutcome::Executed(Action::SubmitCommand)))
    }

    pub(super) fn search_next(
        &mut self,
        reversed: bool,
        buffer: &mut Buffer,
        selections: &mut Selections,
        clipboard: &mut dyn Clipboard,
    ) -> Result<KeyOutcome, VimError> {
        let awaiting = self.take_motion_context();
        let Some(last) = self.search.query() else {
            return Ok(KeyOutcome::SearchFailed(SearchError::NoPrevious));
        };
        self.search.highlight = true;
        let query = Query {
            backward: last.backward != reversed,
            ..last
        };
        let subject = Subject::SearchNext { reversed };
        let find = |buffer: &Buffer, selection: &Selection, times| {
            query.find(buffer, selection.head, times)
        };
        let jump = Jump {
            pattern: query.pattern.source(),
            subject,
            awaiting,
        };
        self.jump_to_match(jump, find, buffer, selections, clipboard)
            .map(|outcome| outcome.unwrap_or(KeyOutcome::Executed(Action::SearchNext { reversed })))
    }

    /// Cada cursor busca su palabra desde donde empieza, para saltarla; la
    /// del principal queda como último patrón.
    pub(super) fn search_word(
        &mut self,
        backward: bool,
        whole_word: bool,
        buffer: &mut Buffer,
        selections: &mut Selections,
        clipboard: &mut dyn Clipboard,
    ) -> Result<KeyOutcome, VimError> {
        let awaiting = self.take_motion_context();
        let search = WordSearch {
            backward,
            whole_word,
        };
        let Some(query) = self.remember_word(search, buffer, selections) else {
            return Ok(KeyOutcome::SearchFailed(SearchError::NoStringUnderCursor));
        };
        self.search.history.push(query.pattern.source());
        let subject = Subject::Object(Object::SearchWord(search));
        let find = |buffer: &Buffer, selection: &Selection, times| {
            search.find(buffer, selection.head, times)
        };
        let action = Action::SearchWord {
            backward,
            whole_word,
        };
        let jump = Jump {
            pattern: query.pattern.source(),
            subject,
            awaiting,
        };
        self.jump_to_match(jump, find, buffer, selections, clipboard)
            .map(|outcome| outcome.unwrap_or(KeyOutcome::Executed(action)))
    }

    /// Como vim, `*` (también al repetirlo con `.`) deja la palabra del
    /// cursor principal como último patrón.
    pub(super) fn remember_word(
        &mut self,
        search: WordSearch,
        buffer: &Buffer,
        selections: &Selections,
    ) -> Option<Query> {
        let (_, query) = search.query(buffer, selections.primary().head)?;
        self.search.remember(&query);
        Some(query)
    }

    /// El operador pendiente, si hay, el count total y el registro; deja el
    /// resto limpio.
    fn take_motion_context(&mut self) -> AwaitingSearch {
        let (operator, visual) = match self.mode {
            Mode::OperatorPending(operator) => (Some(operator), None),
            Mode::Visual(kind) => (None, Some(kind)),
            _ => (None, None),
        };
        let count = self.take_count();
        let register = self.selected.take();
        self.reset();
        AwaitingSearch {
            operator,
            count,
            register,
            visual,
        }
    }

    fn typed_query(&self, line: &str, backward: bool) -> Result<Query, SearchError> {
        let delimiter = Prompt::Search { backward }.symbol();
        let (source, offset) = split_at_delimiter(line, delimiter);
        if let Some(offset) = offset.filter(|offset| !offset.is_empty()) {
            return Err(SearchError::Unsupported(format!("{delimiter}{offset}")));
        }
        let pattern = if source.is_empty() {
            self.search.pattern.clone().ok_or(SearchError::NoPrevious)?
        } else {
            Pattern::compile(&source, Case::Smart)?
        };
        Ok(Query { pattern, backward })
    }

    /// Mueve cada cursor a su coincidencia, u opera hasta ahí. `None` si
    /// salió bien sin dar la vuelta.
    fn jump_to_match(
        &mut self,
        jump: Jump<'_>,
        find: impl Fn(&Buffer, &Selection, usize) -> Option<Hit>,
        buffer: &mut Buffer,
        selections: &mut Selections,
        clipboard: &mut dyn Clipboard,
    ) -> Result<Option<KeyOutcome>, VimError> {
        let Jump {
            pattern,
            subject,
            awaiting,
        } = jump;
        let times = awaiting.count.unwrap_or(1);
        let hits = selections.map_to(|selection| (*selection, find(buffer, selection, times)));
        if hits.iter().all(|(_, hit)| hit.is_none()) {
            let error = SearchError::NotFound(pattern.to_owned());
            return Ok(Some(KeyOutcome::SearchFailed(error)));
        }
        let wrapped = hits
            .iter()
            .find_map(|(_, hit)| hit.and_then(|hit| hit.wrapped));
        match awaiting.operator {
            Some(operator) => {
                let operands = hits.map(|(selection, hit)| Operand {
                    selection: *selection,
                    target: hit
                        .map(|hit| {
                            hit.destination()
                                .span_from(selection.head)
                                .into_target(buffer)
                        })
                        .ok_or(Missed::Stay),
                });
                let operation = Operation {
                    operator,
                    subject,
                    count: awaiting.count,
                    register: awaiting.register,
                };
                self.finish_operator(operation, &operands, buffer, selections, clipboard)?;
            }
            None => {
                *selections = hits.to_selections(|(selection, hit)| {
                    hit.map_or(*selection, |hit| match awaiting.visual {
                        Some(_) => Selection {
                            head: hit.offset,
                            goal: Goal::None,
                            ..*selection
                        },
                        None => Selection::cursor(clamp_normal(buffer, hit.offset)),
                    })
                });
            }
        }
        Ok(wrapped.map(KeyOutcome::SearchWrapped))
    }
}
