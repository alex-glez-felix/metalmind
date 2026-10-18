/// A qué archivo del vault apunta el destino de un link.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Resolution<'p> {
    Resolved(&'p str),
    /// Varias rutas empatan como la más corta; van en el orden recibido.
    Ambiguous(Vec<&'p str>),
    Unresolved,
}

/// Resuelve `target` contra las rutas del vault (relativas a su raíz,
/// separadas por `/`). Coincide el nombre o el final de la ruta
/// (`carpeta/nota`), sin distinguir mayúsculas y con o sin `.md`; gana la de
/// menos carpetas. Si empatan, desempata la única escrita con las mismas
/// mayúsculas; si no, es `Ambiguous` en vez de elegir una al azar. Un `/` al
/// principio exige la ruta completa. El destino vacío de `[[#Heading]]` es la
/// nota misma y aquí no resuelve; los relativos (`./`, `../`) necesitan la
/// nota de origen y van por [`resolve_from`].
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
    let exact = target;
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
        _ => match same_case(&best, exact, from_root).as_slice() {
            [path] => Resolution::Resolved(path),
            _ => Resolution::Ambiguous(best),
        },
    }
}

/// Resuelve `target` escrito en la nota `source` (su ruta en el vault). Un
/// destino que empieza con `./` o `../` es relativo a la carpeta de `source`,
/// como en un link markdown, y un `../` que sale del vault no resuelve;
/// cualquier otro se resuelve con [`resolve`].
pub fn resolve_from<'p, I>(source: &str, target: &str, paths: I) -> Resolution<'p>
where
    I: IntoIterator<Item = &'p str>,
{
    let target = target.trim();
    if !target.starts_with("./") && !target.starts_with("../") {
        return resolve(target, paths);
    }
    let folder = source.rsplit_once('/').map_or("", |(folder, _)| folder);
    match join(folder, target) {
        Some(path) => resolve(&format!("/{path}"), paths),
        None => Resolution::Unresolved,
    }
}

/// `relative` sobre `folder`, con `.` y `..` resueltos. `None` si sube más
/// allá de la raíz.
fn join(folder: &str, relative: &str) -> Option<String> {
    let mut parts: Vec<&str> = folder.split('/').filter(|part| !part.is_empty()).collect();
    for part in relative.split('/') {
        match part {
            "" | "." => {}
            ".." => {
                parts.pop()?;
            }
            _ => parts.push(part),
        }
    }
    Some(parts.join("/"))
}

/// En un sistema de archivos que distingue mayúsculas, `Casa.md` y `casa.md`
/// conviven; `[[Casa]]` apunta a la primera.
fn same_case<'p>(paths: &[&'p str], target: &str, from_root: bool) -> Vec<&'p str> {
    paths
        .iter()
        .copied()
        .filter(|path| matches(path, target, from_root))
        .collect()
}

/// Con mayúsculas exactas; quien llama pasa los dos en minúsculas para
/// ignorarlas.
fn matches(path: &str, target: &str, from_root: bool) -> bool {
    let ends_with = |name: &str| {
        if from_root {
            return name == target;
        }
        name.strip_suffix(target)
            .is_some_and(|rest| rest.is_empty() || rest.ends_with('/'))
    };
    ends_with(path) || strip_md(path).is_some_and(ends_with)
}

/// `path` sin su `.md`, que puede estar en mayúsculas (`Casa.MD`).
fn strip_md(path: &str) -> Option<&str> {
    let stem = path.len().checked_sub(3)?;
    let extension = path.get(stem..)?;
    extension.eq_ignore_ascii_case(".md").then(|| &path[..stem])
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

    #[test]
    fn the_case_tie_break_ignores_the_case_of_the_extension() {
        let paths = ["Casa.MD", "casa.md"];
        assert_eq!(resolve("Casa", paths), Resolution::Resolved("Casa.MD"));
        assert_eq!(resolve("/casa", paths), Resolution::Resolved("casa.md"));
    }

    #[test]
    fn a_multibyte_name_shorter_than_the_extension() {
        assert_eq!(resolve("ñ", ["ñ"]), Resolution::Resolved("ñ"));
        assert_eq!(resolve("x", ["é"]), Resolution::Unresolved);
    }
}
