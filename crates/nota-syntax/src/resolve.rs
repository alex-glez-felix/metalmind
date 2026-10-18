/// A qué archivo del vault apunta el destino de un link.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Resolution<'p> {
    Resolved(&'p str),
    /// Varias rutas empatan como la más corta; van en el orden recibido.
    Ambiguous(Vec<&'p str>),
    Unresolved,
}

/// Resuelve `target` como Obsidian contra las rutas del vault (relativas a
/// su raíz, separadas por `/`). Coincide el nombre o el final de la ruta
/// (`carpeta/nota`), sin distinguir mayúsculas y con o sin `.md`; gana la de
/// menos carpetas. Un `/` al principio exige la ruta completa. El destino
/// vacío de `[[#Heading]]` es la nota misma y aquí no resuelve.
pub fn resolve<'p, I>(target: &str, paths: I) -> Resolution<'p>
where
    I: IntoIterator<Item = &'p str>,
{
    let target = target.trim();
    let (target, from_root) = match target.strip_prefix('/') {
        Some(rest) => (rest, true),
        None => (target, false),
    };
    if target.is_empty() {
        return Resolution::Unresolved;
    }
    let target = target.to_lowercase();
    let mut best: Vec<&'p str> = Vec::new();
    let mut best_depth = usize::MAX;
    for path in paths {
        if !matches(&path.to_lowercase(), &target, from_root) {
            continue;
        }
        let depth = path.matches('/').count();
        if depth < best_depth {
            best_depth = depth;
            best.clear();
        }
        if depth == best_depth {
            best.push(path);
        }
    }
    match best.as_slice() {
        [] => Resolution::Unresolved,
        [path] => Resolution::Resolved(path),
        _ => Resolution::Ambiguous(best),
    }
}

/// `path` y `target` ya en minúsculas.
fn matches(path: &str, target: &str, from_root: bool) -> bool {
    let ends_with = |name: &str| {
        if from_root {
            return name == target;
        }
        name.strip_suffix(target)
            .is_some_and(|rest| rest.is_empty() || rest.ends_with('/'))
    };
    ends_with(path) || path.strip_suffix(".md").is_some_and(ends_with)
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn a_partial_name_is_not_a_match() {
        assert_eq!(resolve("ota", ["nota.md"]), Resolution::Unresolved);
        assert_eq!(resolve("a/nota", ["la/nota.md"]), Resolution::Unresolved);
    }
}
