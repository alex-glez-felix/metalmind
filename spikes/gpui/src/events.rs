use std::collections::VecDeque;

const CAPACITY: usize = 8;

/// Últimos eventos de entrada para el overlay; cada uno también va al log de
/// la terminal, para poder pegarlo en los resultados.
#[derive(Debug, Default)]
pub struct EventLog(VecDeque<String>);

impl EventLog {
    pub fn push(&mut self, entry: String) {
        log::info!("{entry}");
        if self.0.len() == CAPACITY {
            self.0.pop_front();
        }
        self.0.push_back(entry);
    }

    /// Del más reciente al más viejo.
    pub fn recent(&self) -> impl Iterator<Item = &str> {
        self.0.iter().rev().map(String::as_str)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn guarda_los_ultimos_del_mas_reciente_al_mas_viejo() {
        let mut log = EventLog::default();
        for n in 0..CAPACITY + 2 {
            log.push(n.to_string());
        }
        let recent: Vec<&str> = log.recent().collect();
        assert_eq!(recent.len(), CAPACITY);
        assert_eq!(recent.first(), Some(&"9"));
        assert_eq!(recent.last(), Some(&"2"));
    }
}
