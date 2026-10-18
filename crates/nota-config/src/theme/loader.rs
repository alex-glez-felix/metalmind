//! Buscar un tema por nombre, seguir su cadena de `extends` y resolverla.

use std::borrow::Cow;
use std::io;
use std::path::{Path, PathBuf};

use super::groups::{Palette, PartialCursor, PartialSyntax, PartialUi, UnknownColor};
use super::{BUILTIN_THEMES, Theme, ThemeFile};
use crate::diagnostic::{Diagnostic, Loaded, Location};
use crate::parse::parse;

/// Dónde se buscan los temas: primero los del usuario, luego los embebidos.
/// Un tema del usuario con el nombre de uno embebido lo reemplaza.
#[derive(Clone, Debug)]
pub struct Themes {
    user_dir: Option<PathBuf>,
}

/// Un archivo de la cadena de `extends`.
struct Layer {
    name: String,
    file: PathBuf,
    text: Cow<'static, str>,
    theme: ThemeFile,
}

impl Layer {
    fn locate(&self, offset: usize) -> Location {
        Location::of(&self.text, offset)
    }

    fn unknown_color(&self, unknown: UnknownColor) -> Diagnostic {
        let UnknownColor { key, name, offset } = unknown;
        Diagnostic::error(format!("`{name}` no está en la paleta (en {key})"))
            .at(Some(self.locate(offset)))
            .in_file(&self.file)
    }
}

impl Themes {
    #[must_use]
    pub fn new(user_dir: PathBuf) -> Self {
        Self {
            user_dir: Some(user_dir),
        }
    }

    /// Solo los temas embebidos.
    #[must_use]
    pub fn builtin() -> Self {
        Self { user_dir: None }
    }

    /// El tema `name` con toda su cadena de `extends` resuelta. Los avisos
    /// (claves desconocidas) no impiden usarlo.
    ///
    /// # Errors
    /// Si el tema no existe, algún archivo de la cadena está mal, la cadena
    /// es circular o le faltan colores.
    pub fn load(&self, name: &str) -> Result<Loaded<Theme>, Diagnostic> {
        let mut diagnostics = Vec::new();
        let chain = self.chain(name, &mut diagnostics)?;
        let mut palette = Palette::new();
        for layer in chain.iter().rev() {
            palette.extend(
                layer
                    .theme
                    .palette
                    .iter()
                    .map(|(key, hex)| (key.clone(), hex.0)),
            );
        }
        let (mut ui, mut cursor, mut syntax) = Default::default();
        for layer in chain.iter().rev() {
            let resolve = |unknown| layer.unknown_color(unknown);
            ui = layer.theme.ui.resolve(&palette).map_err(resolve)?.over(ui);
            cursor = layer
                .theme
                .cursor
                .resolve(&palette)
                .map_err(resolve)?
                .over(cursor);
            syntax = layer
                .theme
                .syntax
                .resolve(&palette)
                .map_err(resolve)?
                .over(syntax);
        }
        let theme = complete(name, ui, cursor, syntax).map_err(|missing| {
            let error = Diagnostic::error(format!(
                "al tema `{name}` le faltan colores: {}",
                missing.join(", ")
            ));
            match chain.first() {
                Some(leaf) => error.in_file(&leaf.file),
                None => error,
            }
        })?;
        Ok(Loaded {
            value: theme,
            diagnostics,
        })
    }

    /// La cadena desde `name` hasta el tema sin `extends`, en ese orden.
    fn chain(&self, name: &str, warnings: &mut Vec<Diagnostic>) -> Result<Vec<Layer>, Diagnostic> {
        let mut chain: Vec<Layer> = Vec::new();
        let mut next = name.to_owned();
        let mut extended_at: Option<usize> = None;
        loop {
            let referrer = |error: Diagnostic| match chain.last() {
                Some(layer) => error
                    .at(extended_at.map(|offset| layer.locate(offset)))
                    .in_file(&layer.file),
                None => error,
            };
            if chain.iter().any(|layer| layer.name == next) {
                let names: Vec<&str> = chain.iter().map(|layer| layer.name.as_str()).collect();
                return Err(referrer(Diagnostic::error(format!(
                    "`extends` circular: {} → {next}",
                    names.join(" → ")
                ))));
            }
            let (file, text) = self.read(&next).map_err(referrer)?;
            let (theme, layer_warnings) =
                parse::<ThemeFile>(&text).map_err(|error| error.in_file(&file))?;
            warnings.extend(
                layer_warnings
                    .into_iter()
                    .map(|warning| warning.in_file(&file)),
            );
            let extends = theme.extends.clone();
            chain.push(Layer {
                name: next,
                file,
                text,
                theme,
            });
            let Some(extends) = extends else {
                return Ok(chain);
            };
            extended_at = Some(extends.span().start);
            next = extends.into_inner();
        }
    }

    fn read(&self, name: &str) -> Result<(PathBuf, Cow<'static, str>), Diagnostic> {
        if name.is_empty()
            || !name
                .chars()
                .all(|c| c.is_alphanumeric() || c == '-' || c == '_')
        {
            return Err(Diagnostic::error(format!(
                "`{name}` no es un nombre de tema válido: solo letras, números, `-` y `_`"
            )));
        }
        if let Some(dir) = &self.user_dir {
            let file = dir.join(format!("{name}.toml"));
            match std::fs::read_to_string(&file) {
                Ok(text) => return Ok((file, Cow::Owned(text))),
                Err(error) if error.kind() == io::ErrorKind::NotFound => {}
                Err(error) => {
                    return Err(
                        Diagnostic::error(format!("no se pudo leer: {error}")).in_file(&file)
                    );
                }
            }
        }
        BUILTIN_THEMES
            .iter()
            .find(|(builtin, _)| *builtin == name)
            .map(|&(_, text)| {
                (
                    Path::new("<embebido>").join(format!("{name}.toml")),
                    Cow::Borrowed(text),
                )
            })
            .ok_or_else(|| Diagnostic::error(format!("no existe el tema `{name}`")))
    }
}

fn complete(
    name: &str,
    ui: PartialUi,
    cursor: PartialCursor,
    syntax: PartialSyntax,
) -> Result<Theme, Vec<&'static str>> {
    let mut missing = Vec::new();
    let ui = ui.complete(&mut missing);
    let cursor = cursor.complete(&mut missing);
    let syntax = syntax.complete(&mut missing);
    match (ui, cursor, syntax) {
        (Some(ui), Some(cursor), Some(syntax)) => Ok(Theme {
            name: name.to_owned(),
            ui,
            cursor,
            syntax,
        }),
        _ => Err(missing),
    }
}
