use std::borrow::Cow;

use crate::note_name;

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
/// cualquier otro se resuelve con [`resolve`]. Si quedan varias empatadas, gana
/// la única que está en la misma carpeta que `source`.
pub fn resolve_from<'p, I>(source: &str, target: &str, paths: I) -> Resolution<'p>
where
    I: IntoIterator<Item = &'p str>,
{
    let resolution = match absolute(source, target) {
        Some(target) => resolve(&target, paths),
        None => Resolution::Unresolved,
    };
    let Resolution::Ambiguous(tied) = resolution else {
        return resolution;
    };
    let folder = folder(source);
    let mut beside = tied.iter().filter(|path| self::folder(path) == folder);
    match (beside.next(), beside.next()) {
        (Some(path), None) => Resolution::Resolved(path),
        _ => Resolution::Ambiguous(tied),
    }
}

/// Con qué buscar en un índice las rutas que un destino puede resolver.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TargetKey {
    /// El último componente, en minúsculas: está entre los [`path_keys`] de
    /// toda ruta que el destino pueda elegir.
    pub name: String,
    /// Si el destino lleva carpeta, la de antes del nombre en minúsculas, o
    /// vacía si es la raíz (`/nota`): es la [`path_parent_key`] de toda ruta
    /// que pueda elegir. `None` en un nombre solo, que puede estar en
    /// cualquier carpeta.
    pub parent: Option<String>,
}

/// La clave de `target`, escrito en `source`, para buscar sus candidatas
/// sin recorrer todo el vault: basta con pasarle a [`resolve_from`] las
/// rutas con esas claves. `None` si no puede resolver nada.
#[must_use]
pub fn target_key(source: &str, target: &str) -> Option<TargetKey> {
    let target = absolute(source, target)?;
    let target = target.trim();
    let (target, from_root) = match target.strip_prefix('/') {
        Some(rest) => (rest, true),
        None => (target, false),
    };
    let mut parts = target.rsplit('/');
    let name = parts.next().filter(|name| !name.is_empty())?.to_lowercase();
    let parent = match parts.next() {
        Some(parent) => Some(parent.to_lowercase()),
        None => from_root.then(String::new),
    };
    Some(TargetKey { name, parent })
}

/// Los nombres con que se encuentra `path` (ver [`TargetKey::name`]): su
/// nombre en minúsculas y, si termina en `.md`, también sin la extensión.
pub fn path_keys(path: &str) -> impl Iterator<Item = String> {
    let name = path.rsplit('/').next().unwrap_or_default().to_lowercase();
    let stem = note_name::strip_extension(&name).map(str::to_owned);
    std::iter::once(name).chain(stem)
}

/// La carpeta que contiene a `path`, solo su nombre y en minúsculas; vacía
/// en la raíz (ver [`TargetKey::parent`]).
#[must_use]
pub fn path_parent_key(path: &str) -> String {
    let mut parts = path.rsplit('/').skip(1);
    parts.next().unwrap_or_default().to_lowercase()
}

/// `target` como lo compara [`resolve`]: los relativos se vuelven una ruta
/// desde la raíz. `None` si un relativo sube más allá de la raíz.
fn absolute<'t>(source: &str, target: &'t str) -> Option<Cow<'t, str>> {
    let target = target.trim();
    if !target.starts_with("./") && !target.starts_with("../") {
        return Some(Cow::Borrowed(target));
    }
    join(folder(source), target).map(|path| Cow::Owned(format!("/{path}")))
}

fn folder(path: &str) -> &str {
    path.rsplit_once('/').map_or("", |(folder, _)| folder)
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
    ends_with(path) || note_name::strip_extension(path).is_some_and(ends_with)
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
    fn the_same_folder_breaks_a_tie() {
        let paths = ["a/x.md", "b/x.md", "c/x.md"];
        assert_eq!(
            resolve_from("b/y.md", "x", paths),
            Resolution::Resolved("b/x.md")
        );
        assert_eq!(
            resolve_from("y.md", "x", paths),
            Resolution::Ambiguous(paths.to_vec())
        );
    }

    #[test]
    fn keys_are_the_lowercase_last_components() {
        let key = |source, target| target_key(source, target).map(|key| (key.name, key.parent));
        let owned =
            |name: &str, parent: Option<&str>| Some((name.to_owned(), parent.map(str::to_owned)));
        assert_eq!(key("a/b.md", "Nota.MD"), owned("nota.md", None));
        assert_eq!(
            key("a/b.md", "Carpeta/Nota.MD"),
            owned("nota.md", Some("carpeta"))
        );
        assert_eq!(key("a/b.md", "/Nota"), owned("nota", Some("")));
        assert_eq!(key("a/b.md", "./Nota"), owned("nota", Some("a")));
        assert_eq!(key("a/b.md", "../x/./"), owned("x", Some("")));
        assert_eq!(key("a/b.md", "../../x"), None);
        assert_eq!(key("a/b.md", "/"), None);
        let keys: Vec<String> = path_keys("a/Nota.MD").collect();
        assert_eq!(keys, ["nota.md", "nota"]);
        assert_eq!(path_keys("img.png").collect::<Vec<_>>(), ["img.png"]);
        assert_eq!(path_parent_key("A/B/nota.md"), "b");
        assert_eq!(path_parent_key("nota.md"), "");
    }

    #[test]
    fn a_bare_extension_is_a_file_and_not_a_note_without_a_name() {
        assert_eq!(resolve("a/", ["a/.md"]), Resolution::Unresolved);
        assert_eq!(resolve("a/.md", ["a/.md"]), Resolution::Resolved("a/.md"));
        assert_eq!(path_keys("a/.MD").collect::<Vec<_>>(), [".md"]);
    }

    #[test]
    fn a_multibyte_name_shorter_than_the_extension() {
        assert_eq!(resolve("ñ", ["ñ"]), Resolution::Resolved("ñ"));
        assert_eq!(resolve("x", ["é"]), Resolution::Unresolved);
    }
}
