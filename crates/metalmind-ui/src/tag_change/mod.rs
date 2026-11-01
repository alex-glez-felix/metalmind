//! Renombrar y fusionar tags y cambiar los campos de su página desde la UI
//! (05 §3 y §5), sin GPUI: qué se pidió con `:tagrename`, `:tagmerge`,
//! `:tagaliases` y los demás, y su plan como vista previa
//! ([`ChangePreview`]), que decide qué se aplica.

mod field;
mod views;

use metalmind_vault::VaultPath;
use metalmind_vault::tags::{PageChange, PlanError, PlanNotice, TagPlan, Tags, tag_name};

pub use self::field::{FieldError, TagField};
pub use self::views::{VaultConfigText, config_path};
use crate::change_preview::{ChangePreview, Notice};

/// Un cambio de tags pedido, con los nombres como se escribieron.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TagRequest {
    Rename {
        from: String,
        to: String,
    },
    Merge {
        from: String,
        into: String,
    },
    /// Un campo de la página del tag; `change` sale de [`TagField::change`].
    Page {
        tag: String,
        field: TagField,
        change: PageChange,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum RequestError {
    #[error("uso: {0}")]
    Usage(&'static str),
    #[error(transparent)]
    Field(#[from] FieldError),
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

    /// Lo escrito tras `:tagaliases` y los demás de `field`: el tag y su
    /// valor nuevo, que vacío quita el campo.
    ///
    /// # Errors
    /// Si falta el tag o el valor no vale para `field`.
    pub fn page(field: TagField, argument: Option<&str>) -> Result<Self, RequestError> {
        let argument = argument.unwrap_or_default().trim();
        let (tag, value) = argument
            .split_once(char::is_whitespace)
            .unwrap_or((argument, ""));
        if tag.is_empty() {
            return Err(RequestError::Usage(field.usage()));
        }
        Ok(Self::Page {
            tag: bare(tag),
            field,
            change: field.change(value)?,
        })
    }

    /// El plan sobre `tags`, sin tocar el disco.
    ///
    /// # Errors
    /// Si algún nombre no es válido o no existe.
    pub fn plan(&self, tags: &Tags) -> Result<TagPlan, PlanError> {
        match self {
            Self::Rename { from, to } => tags.plan_rename(from, to),
            Self::Merge { from, into } => tags.plan_merge(from, into),
            Self::Page { tag, change, .. } => tags.plan_page_change(tag, change),
        }
    }

    /// La vista previa del plan sobre `tags`, con las vistas guardadas de
    /// `config` que lo siguen; `None` si no hay nada que cambiar ni que
    /// avisar. `unsaved` dice si un archivo del vault tiene cambios sin
    /// guardar: `tags` y `config` tienen que ser los de sus buffers
    /// ([`Tags::with_unsaved`]), porque sus ediciones van a ellos.
    ///
    /// # Errors
    /// Si algún nombre no es válido o no existe.
    pub fn preview(
        &self,
        tags: &Tags,
        config: &VaultConfigText,
        unsaved: impl Fn(&VaultPath) -> bool,
    ) -> Result<Option<ChangePreview>, PlanError> {
        let TagPlan {
            mut changes,
            notices,
            rewrite,
        } = self.plan(tags)?;
        let mut notices: Vec<Notice> = notices
            .iter()
            .map(|notice| Notice {
                text: notice_text(notice),
                blocks: notice.blocks(),
            })
            .collect();
        if let Some(rewrite) = rewrite {
            match views::view_edits(&rewrite, config) {
                Ok(Some(edits)) => {
                    changes.edits.push(edits);
                    changes.edits.sort_by(|a, b| a.path.cmp(&b.path));
                }
                Ok(None) => {}
                Err(error) => notices.push(Notice {
                    text: format!("las vistas guardadas no siguen a #{}: {error}", rewrite.to),
                    blocks: false,
                }),
            }
        }
        Ok(ChangePreview::new(self.title(), changes, notices, unsaved)
            .map(ChangePreview::with_edits_in_buffers))
    }

    /// Lo que encabeza la vista previa y el aviso al aplicar.
    #[must_use]
    pub fn title(&self) -> String {
        match self {
            Self::Rename { from, to } => format!("Renombrar #{from} a #{to}"),
            Self::Merge { from, into } => format!("Fusionar #{from} en #{into}"),
            Self::Page { tag, field, .. } => format!("{} de #{tag}", field.label()),
        }
    }
}

fn notice_text(notice: &PlanNotice) -> String {
    match notice {
        PlanNotice::Merges { into } => format!("#{into} ya existe: se fusionan"),
        PlanNotice::RenamesAlias { of } => format!("es un alias de #{of}: se renombra el alias"),
        PlanNotice::AliasAbsorbs { tag, of } => {
            format!("#{tag} ya se escribe en notas: pasa a contar como #{of}")
        }
        PlanNotice::BothHavePages { from, into } => {
            format!("los dos tags tienen página ({from} y {into}): deja una antes de fusionar")
        }
        PlanNotice::AliasTaken { alias, by } => format!("#{alias} ya es de #{by}"),
        PlanNotice::NotPortable { path } => {
            format!("{path} no se podrá sincronizar con Windows: ese nombre está reservado")
        }
    }
}

fn two_tags(argument: Option<&str>, usage: &'static str) -> Result<(String, String), RequestError> {
    let words: Vec<&str> = argument.unwrap_or_default().split_whitespace().collect();
    match words.as_slice() {
        [first, second] => Ok((bare(first), bare(second))),
        _ => Err(RequestError::Usage(usage)),
    }
}

/// Sin `#` si es un nombre válido; si no, como se escribió, para que el plan
/// diga por qué no vale.
fn bare(word: &str) -> String {
    tag_name(word).unwrap_or(word).to_owned()
}

#[cfg(test)]
mod tests;
