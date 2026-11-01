//! La vista previa de `space /` (#77), sin GPUI: las líneas de la nota
//! alrededor del resultado marcado, con lo que coincide resaltado.
//!
//! Una nota puede pesar megabytes, así que se lee y se corta en el fondo
//! ([`PreviewJob::run`]): cada pedido lleva su versión y lo que vuelve de
//! uno viejo se descarta. Lo de antes se ve hasta que llega lo nuevo, así
//! el panel no parpadea vacío al moverse entre resultados. La última nota
//! leída se guarda: moverse entre sus resultados no la vuelve a leer. Una
//! nota abierta con cambios se muestra como se ve en el editor.

use std::collections::HashMap;
use std::fs;
use std::ops::Range;
use std::path::PathBuf;
use std::sync::Arc;

use metalmind_buffer::Snapshot;
use metalmind_search::LineMatch;
use metalmind_vault::VaultPath;
use unicode_segmentation::UnicodeSegmentation as _;
use unicode_width::UnicodeWidthChar as _;

use super::context::{Cut, crop, end_after};
use super::{Hit, OpenNote};
use crate::picker::{Preview, PreviewLine};

/// Líneas que se toman a cada lado del resultado: más de las que caben en
/// el panel más alto, que las recorta centrando el resultado.
pub const AROUND: usize = 30;
/// Grafemas que se guardan de cada línea: más de los que caben en el panel
/// más ancho, y una línea de un megabyte no se pinta entera.
const MAX_LINE: usize = 300;
/// Una coincidencia que empieza más lejos que estas columnas no se vería en
/// un panel angosto: su línea se corta por delante.
const VISIBLE_START: usize = 32;
/// Las columnas que se dejan antes de la coincidencia al cortar por delante.
const BEFORE_MATCH: usize = 12;

pub struct PreviewState {
    root: PathBuf,
    /// Las notas abiertas con cambios sin guardar.
    open: HashMap<VaultPath, Snapshot>,
    /// La del último pedido: lo que vuelve de otra se descarta.
    version: u64,
    /// El resultado del último pedido.
    wanted: Option<Hit>,
    shown: Option<Preview>,
    /// La última nota leída, con su texto.
    read: Option<(VaultPath, Arc<str>)>,
}

/// Leer una nota y cortar sus líneas alrededor de un resultado, en el
/// fondo: es `Send`.
pub struct PreviewJob {
    version: u64,
    hit: Hit,
    source: Source,
}

enum Source {
    Read(Arc<str>),
    Open(Snapshot),
    Disk(PathBuf),
}

/// Lo que vuelve de un [`PreviewJob`].
pub struct Loaded {
    version: u64,
    path: VaultPath,
    /// El texto de la nota, si se pudo leer.
    text: Option<Arc<str>>,
    preview: Preview,
}

impl PreviewState {
    /// Para las notas del vault de raíz `root`; las de `open` se muestran
    /// con su texto en memoria.
    #[must_use]
    pub fn new(root: PathBuf, open: &[OpenNote]) -> Self {
        Self {
            root,
            open: open
                .iter()
                .map(|note| (note.path.clone(), note.text.clone()))
                .collect(),
            version: 0,
            wanted: None,
            shown: None,
            read: None,
        }
    }

    #[must_use]
    pub fn shown(&self) -> Option<&Preview> {
        self.shown.as_ref()
    }

    /// Ahora está marcado `hit`: devuelve lo que hay que correr para
    /// mostrarlo, o `None` si ya se mostró o se pidió. Sin resultado, el
    /// panel queda vacío ya.
    pub fn request(&mut self, hit: Option<&Hit>) -> Option<PreviewJob> {
        if self.wanted.as_ref() == hit {
            return None;
        }
        self.version += 1;
        self.wanted = hit.cloned();
        let Some(hit) = hit else {
            self.shown = None;
            return None;
        };
        let source = match (&self.read, self.open.get(&hit.path)) {
            (Some((path, text)), _) if *path == hit.path => Source::Read(Arc::clone(text)),
            (_, Some(snapshot)) => Source::Open(snapshot.clone()),
            _ => Source::Disk(self.root.join(hit.path.as_str())),
        };
        Some(PreviewJob {
            version: self.version,
            hit: hit.clone(),
            source,
        })
    }

    /// Se lanzó otra búsqueda, que lee las notas del disco de nuevo: lo leído
    /// antes, o lo que se está leyendo, puede ser viejo. El resultado marcado
    /// se vuelve a pedir aunque sea el mismo; lo que se ve se queda hasta que
    /// llegue.
    pub fn searched_again(&mut self) {
        self.read = None;
        self.version += 1;
        self.wanted = None;
    }

    /// Volvió un [`PreviewJob`]: si es del último pedido, se muestra.
    pub fn finish(&mut self, loaded: Loaded) {
        if loaded.version != self.version {
            return;
        }
        if let Some(text) = loaded.text {
            self.read = Some((loaded.path, text));
        }
        self.shown = Some(loaded.preview);
    }
}

impl PreviewJob {
    #[must_use]
    pub fn run(self) -> Loaded {
        let text = match self.source {
            Source::Read(text) => Ok(text),
            Source::Open(snapshot) => Ok(Arc::from(snapshot.to_string())),
            Source::Disk(file) => fs::read_to_string(file).map(Arc::from),
        };
        let (text, preview) = match text {
            Ok(text) => {
                let preview = excerpt(&text, &self.hit);
                (Some(text), preview)
            }
            Err(error) => {
                let message = format!("No se pudo leer la nota: {error}");
                (None, Preview::Message(message.into()))
            }
        };
        Loaded {
            version: self.version,
            path: self.hit.path,
            text,
            preview,
        }
    }
}

/// Las líneas de `text` alrededor de la de `hit`. Lo que coincide se
/// resalta solo si la línea sigue siendo la que se encontró.
#[must_use]
pub fn excerpt(text: &str, hit: &Hit) -> Preview {
    let target = hit.line.line;
    let mut focus = None;
    let lines: Vec<PreviewLine> = text
        .lines()
        .enumerate()
        .skip(target.saturating_sub(AROUND))
        .take(2 * AROUND + 1)
        .map(|(index, line)| {
            let shown = if index == target {
                focus = Some(index);
                found_line(line, &hit.line)
            } else {
                capped(line, 0, &[])
            };
            PreviewLine {
                number: index + 1,
                text: shown.text,
                highlights: shown.highlights,
            }
        })
        .collect();
    if lines.is_empty() {
        return Preview::Message("La nota ya no llega a esa línea".into());
    }
    let first = target.saturating_sub(AROUND);
    Preview::Lines {
        focus: focus.map(|index| index - first),
        lines,
    }
}

/// La línea del resultado, cortada por delante si lo que coincide quedaría
/// fuera de la vista.
fn found_line(line: &str, found: &LineMatch) -> super::RowText {
    if line != found.text {
        return capped(line, 0, &[]);
    }
    let before = &line[..found.matches.first().map_or(0, |range| range.start)];
    let start = if wider_than(before, VISIBLE_START) {
        last_columns(before, BEFORE_MATCH)
    } else {
        0
    };
    capped(line, start, &found.matches)
}

/// Las columnas de `c` en la vista previa: un carácter CJK ocupa dos y un
/// tab, que se pinta como espacio, una.
fn columns(c: char) -> usize {
    c.width().unwrap_or(1)
}

/// Si `text` ocupa más de `limit` columnas.
fn wider_than(text: &str, limit: usize) -> bool {
    let mut width = 0;
    text.chars().any(|c| {
        width += columns(c);
        width > limit
    })
}

/// El byte desde donde lo que queda de `text` ocupa a lo sumo `limit`
/// columnas, sin partir un grafema.
fn last_columns(text: &str, limit: usize) -> usize {
    let mut width = 0;
    let mut start = text.len();
    for (at, grapheme) in text.grapheme_indices(true).rev() {
        width += grapheme.chars().map(columns).sum::<usize>();
        if width > limit {
            break;
        }
        start = at;
    }
    start
}

/// `line` desde `start`, con a lo sumo [`MAX_LINE`] grafemas.
fn capped(line: &str, start: usize, matches: &[Range<usize>]) -> super::RowText {
    let end = end_after(line, start, MAX_LINE);
    let cut = Cut {
        before: start > 0,
        after: end < line.len(),
    };
    crop(line, start..end, cut, matches)
}

#[cfg(test)]
#[path = "preview_tests.rs"]
mod tests;
