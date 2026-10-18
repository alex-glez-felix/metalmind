//! `/`, `?`, `n`, `N`, `*`, `#`, `g*` y `g#`, solos o como objeto de un
//! operador.

use nota_buffer::{Buffer, ByteOffset, Selection, Selections};

use super::{Vim, combined};
use crate::dot::Subject;
use crate::history::History;
use crate::lines::clamp_normal;
use crate::object::Object;
use crate::operator::Operand;
use crate::search::{Case, Query, Word, split_at_delimiter};
use crate::span::Missed;
use crate::{Action, KeyOutcome, Mode, Operator, Pattern, Prompt, SearchError, VimError};

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

/// Un operador y su count, esperando a que se envíe `/` o `?`.
#[derive(Clone, Copy, Debug, Default)]
pub(super) struct AwaitingSearch {
    operator: Option<Operator>,
    count: Option<usize>,
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
    ) -> Result<KeyOutcome, VimError> {
        let awaiting = self.awaiting_search.take().unwrap_or_default();
        self.search.history.push(line);
        let query = match self.typed_query(line, backward) {
            Ok(query) => query,
            Err(error) => return Ok(KeyOutcome::SearchFailed(error)),
        };
        self.search.remember(&query);
        let subject = Subject::Object(Object::Search(query.clone()));
        self.jump_to_match(&query, subject, awaiting, at_head, buffer, selections)
            .map(|outcome| outcome.unwrap_or(KeyOutcome::Executed(Action::SubmitCommand)))
    }

    pub(super) fn search_next(
        &mut self,
        reversed: bool,
        buffer: &mut Buffer,
        selections: &mut Selections,
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
        self.jump_to_match(&query, subject, awaiting, at_head, buffer, selections)
            .map(|outcome| outcome.unwrap_or(KeyOutcome::Executed(Action::SearchNext { reversed })))
    }

    /// Como vim, ignora `smartcase`, y cada cursor busca desde el inicio de
    /// su palabra para saltarla.
    pub(super) fn search_word(
        &mut self,
        backward: bool,
        whole_word: bool,
        buffer: &mut Buffer,
        selections: &mut Selections,
    ) -> Result<KeyOutcome, VimError> {
        let awaiting = self.take_motion_context();
        let Some(word) = Word::under_cursor(buffer, selections.primary().head) else {
            return Ok(KeyOutcome::SearchFailed(SearchError::NoStringUnderCursor));
        };
        let source = word.pattern(whole_word);
        let pattern = match Pattern::compile(&source, Case::Ignore) {
            Ok(pattern) => pattern,
            Err(error) => return Ok(KeyOutcome::SearchFailed(error)),
        };
        self.search.history.push(&source);
        let query = Query { pattern, backward };
        self.search.remember(&query);
        let subject = Subject::Object(Object::Search(query.clone()));
        let from = |buffer: &Buffer, selection: &Selection| {
            Word::under_cursor(buffer, selection.head).map_or(selection.head, |word| word.start)
        };
        let action = Action::SearchWord {
            backward,
            whole_word,
        };
        self.jump_to_match(&query, subject, awaiting, from, buffer, selections)
            .map(|outcome| outcome.unwrap_or(KeyOutcome::Executed(action)))
    }

    /// El operador pendiente, si hay, y el count total; deja el resto limpio.
    fn take_motion_context(&mut self) -> AwaitingSearch {
        let operator = match self.mode {
            Mode::OperatorPending(operator) => Some(operator),
            _ => None,
        };
        let count = combined(self.operator_count.take(), self.count.take());
        self.reset();
        AwaitingSearch { operator, count }
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
        query: &Query,
        subject: Subject,
        awaiting: AwaitingSearch,
        from: impl Fn(&Buffer, &Selection) -> ByteOffset,
        buffer: &mut Buffer,
        selections: &mut Selections,
    ) -> Result<Option<KeyOutcome>, VimError> {
        let times = awaiting.count.unwrap_or(1);
        let hits = selections.map_to(|selection| {
            let hit = query.find(buffer, from(buffer, selection), times);
            (*selection, hit)
        });
        if hits.iter().all(|(_, hit)| hit.is_none()) {
            let pattern = query.pattern.source().to_owned();
            return Ok(Some(KeyOutcome::SearchFailed(SearchError::NotFound(
                pattern,
            ))));
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
                self.finish_operator(
                    operator,
                    subject,
                    awaiting.count,
                    &operands,
                    buffer,
                    selections,
                )?;
            }
            None => {
                *selections = hits.to_selections(|(selection, hit)| {
                    hit.map_or(*selection, |hit| {
                        Selection::cursor(clamp_normal(buffer, hit.offset))
                    })
                });
            }
        }
        Ok(wrapped.map(KeyOutcome::SearchWrapped))
    }
}

fn at_head(_: &Buffer, selection: &Selection) -> ByteOffset {
    selection.head
}
