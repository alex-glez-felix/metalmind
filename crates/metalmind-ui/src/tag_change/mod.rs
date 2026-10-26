//! Renombrar y fusionar tags desde la UI (05 §5), sin GPUI: qué se pidió
//! con `:tagrename` o `:tagmerge`, su plan y la vista previa que decide qué
//! se aplica.

mod preview;
mod view;

use metalmind_vault::tags::{PlanError, TagPlan, Tags, tag_name};

pub use self::preview::{Blocked, TagChangePreview};
pub use self::view::{TagChangeEvent, TagChangeView};

/// Un cambio de tags pedido, con los nombres como se escribieron.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TagRequest {
    Rename { from: String, to: String },
    Merge { from: String, into: String },
}

#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
#[error("uso: {usage}")]
pub struct RequestError {
    usage: &'static str,
}

impl TagRequest {
    /// Lo escrito tras `:tagrename`: dos tags, con `#` o sin él.
    ///
    /// # Errors
    /// Si no son exactamente dos.
    pub fn rename(argument: Option<&str>) -> Result<Self, RequestError> {
        let (from, to) = two_tags(argument, ":tagrename #viejo #nuevo")?;
        Ok(Self::Rename { from, to })
    }

    /// Lo escrito tras `:tagmerge`.
    ///
    /// # Errors
    /// Si no son exactamente dos tags.
    pub fn merge(argument: Option<&str>) -> Result<Self, RequestError> {
        let (from, into) = two_tags(argument, ":tagmerge #origen #destino")?;
        Ok(Self::Merge { from, into })
    }

    /// El plan sobre `tags`, sin tocar el disco.
    ///
    /// # Errors
    /// Si algún nombre no es válido o no existe.
    pub fn plan(&self, tags: &Tags) -> Result<TagPlan, PlanError> {
        match self {
            Self::Rename { from, to } => tags.plan_rename(from, to),
            Self::Merge { from, into } => tags.plan_merge(from, into),
        }
    }

    /// Lo que encabeza la vista previa y el aviso al aplicar.
    #[must_use]
    pub fn title(&self) -> String {
        match self {
            Self::Rename { from, to } => format!("Renombrar #{from} a #{to}"),
            Self::Merge { from, into } => format!("Fusionar #{from} en #{into}"),
        }
    }
}

fn two_tags(argument: Option<&str>, usage: &'static str) -> Result<(String, String), RequestError> {
    // Sin `#` si es un nombre válido; si no, como se escribió, para que el
    // plan diga por qué no vale.
    let bare = |word: &str| tag_name(word).unwrap_or(word).to_owned();
    let words: Vec<&str> = argument.unwrap_or_default().split_whitespace().collect();
    match words.as_slice() {
        [first, second] => Ok((bare(first), bare(second))),
        _ => Err(RequestError { usage }),
    }
}

#[cfg(test)]
mod tests;
