//! Unir con el teclado: una letra sobre cada nodo visible, como el `f` de
//! Vimium (09 §4.1). Sin GPUI, para probarlo.

/// De la fila del medio hacia afuera: lo primero en salir es lo más a mano.
const LETTERS: &str = "asdfghjklqwertyuiopzxcvbnm";

#[derive(Clone, Debug)]
pub struct Hints {
    from: String,
    labels: Vec<(String, String)>,
    typed: String,
    waiting: bool,
}

impl Hints {
    /// `None` si no hay a qué unir.
    pub fn new(from: String, targets: impl Iterator<Item = String>) -> Option<Self> {
        let targets: Vec<String> = targets.collect();
        if targets.is_empty() {
            return None;
        }
        let labels = labels(targets.len());
        Some(Self {
            from,
            labels: targets.into_iter().zip(labels).collect(),
            typed: String::new(),
            waiting: true,
        })
    }

    pub fn from(&self) -> &str {
        &self.from
    }

    /// El nodo, su letra y lo que ya se tecleó de ella, de lo que sigue en juego.
    pub fn visible(&self) -> impl Iterator<Item = (&str, &str)> {
        self.labels
            .iter()
            .filter(|(_, label)| label.starts_with(&self.typed))
            .map(|(id, label)| (id.as_str(), label.as_str()))
    }

    /// Una tecla más: el nodo, si se completó su letra. Sin ninguna letra que
    /// empiece así, deja de esperar.
    pub fn type_key(&mut self, key: char) -> Option<String> {
        self.typed.extend(key.to_lowercase());
        let reached = self
            .labels
            .iter()
            .find(|(_, label)| *label == self.typed)
            .map(|(id, _)| id.clone());
        if reached.is_some() {
            self.waiting = false;
            return reached;
        }
        let still = self.visible().next().is_some();
        self.waiting = still;
        None
    }

    pub fn is_waiting(&self) -> bool {
        self.waiting
    }
}

/// `count` letras donde ninguna es el inicio de otra: de una si caben, de dos
/// si no.
fn labels(count: usize) -> Vec<String> {
    let letters: Vec<char> = LETTERS.chars().collect();
    if count <= letters.len() {
        return letters.iter().take(count).map(char::to_string).collect();
    }
    let length = count.ilog(letters.len()) + 1;
    (0..count)
        .map(|index| {
            let mut rest = index;
            let mut label = String::new();
            for _ in 0..length {
                label.push(letters[rest % letters.len()]);
                rest /= letters.len();
            }
            label
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use pretty_assertions::assert_eq;
    use rstest::rstest;

    use super::*;

    fn hints(count: usize) -> Hints {
        let targets = (0..count).map(|index| format!("n{index}"));
        Hints::new("from".to_owned(), targets).unwrap()
    }

    #[test]
    fn sin_otro_nodo_no_hay_letras() {
        assert!(Hints::new("a".to_owned(), std::iter::empty()).is_none());
    }

    #[test]
    fn una_letra_llega_a_su_nodo() {
        let mut hints = hints(3);
        assert_eq!(hints.type_key('s'), Some("n1".to_owned()));
        assert!(!hints.is_waiting());
    }

    #[test]
    fn una_letra_que_no_es_de_nadie_cancela() {
        let mut hints = hints(3);
        assert_eq!(hints.type_key('z'), None);
        assert!(!hints.is_waiting());
    }

    #[test]
    fn la_mayuscula_vale_como_la_minuscula() {
        let mut hints = hints(3);
        assert_eq!(hints.type_key('D'), Some("n2".to_owned()));
    }

    #[test]
    fn con_mas_de_veintiseis_las_letras_son_de_dos_y_la_primera_filtra() {
        let mut hints = hints(30);
        assert_eq!(hints.visible().count(), 30);
        assert_eq!(hints.type_key('a'), None);
        assert!(hints.is_waiting());
        assert!(hints.visible().count() < 30);
        let (_, label) = hints.visible().next().unwrap();
        let second = label.chars().nth(1).unwrap();
        assert!(hints.type_key(second).is_some());
    }

    #[rstest]
    #[case(1)]
    #[case(26)]
    #[case(27)]
    #[case(676)]
    #[case(677)]
    fn ninguna_letra_es_el_inicio_de_otra(#[case] count: usize) {
        let labels = labels(count);
        assert_eq!(labels.iter().collect::<HashSet<_>>().len(), count);
        let length = labels[0].len();
        assert!(labels.iter().all(|label| label.len() == length));
    }
}
