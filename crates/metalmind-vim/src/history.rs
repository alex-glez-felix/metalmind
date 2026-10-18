//! Lo enviado en una línea de comandos, para recorrerlo con las flechas.

/// Como en vim, las flechas solo pasan por las entradas que empiezan con lo
/// que se había tecleado, y una entrada repetida sube al final sin duplicarse.
#[derive(Debug, Default)]
pub(crate) struct History {
    entries: Vec<String>,
    browsing: Option<Browsing>,
}

#[derive(Debug)]
struct Browsing {
    typed: String,
    /// Entrada que se muestra; `entries.len()` es lo tecleado.
    index: usize,
}

impl History {
    pub fn push(&mut self, entry: &str) {
        self.browsing = None;
        if entry.is_empty() {
            return;
        }
        self.entries.retain(|old| old != entry);
        self.entries.push(entry.to_owned());
    }

    /// Lo escrito cambió: la próxima flecha filtra por el texto nuevo.
    pub fn stop_browsing(&mut self) {
        self.browsing = None;
    }

    /// La entrada anterior que empieza con lo tecleado; `None` si no hay.
    pub fn older(&mut self, line: &str) -> Option<&str> {
        let len = self.entries.len();
        let browsing = self.browsing.get_or_insert_with(|| Browsing {
            typed: line.to_owned(),
            index: len,
        });
        let index = (0..browsing.index)
            .rev()
            .find(|&index| self.entries[index].starts_with(&browsing.typed))?;
        browsing.index = index;
        Some(&self.entries[index])
    }

    /// La entrada siguiente que empieza con lo tecleado, o lo tecleado tras
    /// la última; `None` si no se estaba recorriendo.
    pub fn newer(&mut self) -> Option<&str> {
        let browsing = self.browsing.as_mut()?;
        let found = (browsing.index + 1..self.entries.len())
            .find(|&index| self.entries[index].starts_with(&browsing.typed));
        browsing.index = found.unwrap_or(self.entries.len());
        Some(found.map_or(&browsing.typed, |index| &self.entries[index]))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn history(entries: &[&str]) -> History {
        let mut history = History::default();
        for entry in entries {
            history.push(entry);
        }
        history
    }

    #[test]
    fn walks_back_and_forth() {
        let mut history = history(&["a", "b", "c"]);
        assert_eq!(history.older(""), Some("c"));
        assert_eq!(history.older(""), Some("b"));
        assert_eq!(history.older(""), Some("a"));
        assert_eq!(history.older(""), None);
        assert_eq!(history.newer(), Some("b"));
        assert_eq!(history.newer(), Some("c"));
        assert_eq!(history.newer(), Some(""));
        assert_eq!(history.newer(), Some(""));
    }

    #[test]
    fn filters_by_what_was_typed() {
        let mut history = history(&["bx", "c", "b"]);
        assert_eq!(history.older("b"), Some("b"));
        assert_eq!(history.older("b"), Some("bx"));
        assert_eq!(history.newer(), Some("b"));
        assert_eq!(history.newer(), Some("b"));
        let mut history = self::history(&["a"]);
        assert_eq!(history.older("z"), None);
        assert_eq!(history.newer(), Some("z"));
    }

    #[test]
    fn a_repeated_entry_moves_to_the_end() {
        let mut history = history(&["b", "c", "b", ""]);
        assert_eq!(history.older(""), Some("b"));
        assert_eq!(history.older(""), Some("c"));
        assert_eq!(history.older(""), None);
    }
}
