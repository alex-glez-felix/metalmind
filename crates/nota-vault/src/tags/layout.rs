//! Cómo está escrito el frontmatter de una página de tag, para cambiar un
//! campo sin tocar el resto (05 §3).

use std::ops::Range;

use nota_syntax::Frontmatter;

use crate::tags::note::Lines;

/// Cómo está escrito el frontmatter de una página.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PageLayout {
    /// Sin frontmatter: se agrega uno en `at`, tras el BOM si lo hay.
    Bare { at: usize, crlf: bool },
    /// Un mapa en bloque: un campo nuevo va en `end`, la línea del `---` de
    /// cierre.
    Block {
        entries: Vec<PageEntry>,
        end: usize,
        end_line: usize,
        crlf: bool,
    },
    /// YAML roto o con una forma que nota no edita (`{a: 1}`): hay que
    /// arreglarlo a mano antes de cambiar un campo.
    Unreadable,
}

/// Una clave del primer nivel y lo que ocupa, con su texto, que una edición
/// comprueba antes de reemplazarlo.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PageEntry {
    pub key: String,
    pub span: Range<usize>,
    pub text: String,
    pub line: usize,
}

impl PageLayout {
    pub(crate) fn new(text: &str, frontmatter: Option<&Frontmatter>) -> Self {
        let crlf = text
            .split('\n')
            .next()
            .is_some_and(|line| line.ends_with('\r'));
        match frontmatter {
            None => Self::Bare {
                at: if text.starts_with('\u{FEFF}') {
                    '\u{FEFF}'.len_utf8()
                } else {
                    0
                },
                crlf,
            },
            Some(Frontmatter {
                entries: Some(entries),
                yaml,
                ..
            }) => {
                let mut lines = Lines::new(text);
                Self::Block {
                    entries: entries
                        .iter()
                        .map(|entry| PageEntry {
                            key: entry.key.clone(),
                            span: entry.span.clone(),
                            text: text[entry.span.clone()].to_owned(),
                            line: lines.line_of(entry.span.start),
                        })
                        .collect(),
                    end: yaml.end,
                    end_line: lines.line_of(yaml.end),
                    crlf,
                }
            }
            Some(_) => Self::Unreadable,
        }
    }
}
