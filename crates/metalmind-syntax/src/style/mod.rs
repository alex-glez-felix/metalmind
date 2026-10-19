//! Cómo se ve cada línea de una nota en el editor: qué tramos son sintaxis
//! (que se atenúa y, en el live preview, se podrá ocultar), qué es un
//! heading, un link, un tag o código. Sale del mismo scanner que el índice,
//! recorrido por tramos que empiezan en un estado conocido, así que el
//! editor nunca ve un link donde el índice no lo ve.
//!
//! [`Styles`] guarda el estilo de cada línea y lo recalcula por tramos al
//! editar, solo hasta donde se pide y hasta que el estado vuelve a coincidir.

mod block;
mod cache;
mod emphasis;
mod fence;
mod flatten;
mod frontmatter;
mod inline;
mod pieces;
mod region;

#[cfg(test)]
mod tests;

use std::borrow::Cow;
use std::ops::Range;

pub use cache::Styles;

/// De dónde sale el texto de cada línea: el buffer del editor o, en tests,
/// un `&str`.
pub trait Lines {
    fn line_count(&self) -> usize;
    /// La línea `index`, sin el salto.
    fn line(&self, index: usize) -> Cow<'_, str>;
}

/// Las líneas de un texto, partidas por `\n` como las de un buffer.
#[derive(Debug, Clone)]
pub struct TextLines<'t> {
    lines: Vec<&'t str>,
}

impl<'t> TextLines<'t> {
    #[must_use]
    pub fn new(text: &'t str) -> Self {
        Self {
            lines: text.split('\n').collect(),
        }
    }
}

impl Lines for TextLines<'_> {
    fn line_count(&self) -> usize {
        self.lines.len()
    }

    fn line(&self, index: usize) -> Cow<'_, str> {
        Cow::Borrowed(self.lines.get(index).copied().unwrap_or_default())
    }
}

/// El estilo de una línea.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LineStyle {
    pub kind: LineKind,
    /// Cuántos `>` de cita la abren.
    pub quote: u8,
    /// Tramos con estilo, en bytes desde el inicio de la línea, en orden y
    /// sin solaparse. Lo que no cubren es texto normal.
    pub spans: Vec<Span>,
    /// Los links de la línea, para pintar distinto los que no llevan a
    /// ninguna nota. Un [`Span`] apunta aquí con [`Span::link`].
    pub links: Vec<LinkSpot>,
    /// Las fórmulas que se componen en esta línea, en orden.
    pub math: Vec<MathSpot>,
}

/// Lo que cambia la línea entera: su alto, su fondo.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum LineKind {
    #[default]
    Text,
    /// Un heading ATX, del 1 al 6.
    Heading(u8),
    /// `---`, `***` o `___`.
    Rule,
    /// Una línea de un bloque de código cercado.
    Fence(FencePart),
    /// Una línea del frontmatter, delimitadores incluidos.
    Frontmatter,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FencePart {
    Open,
    Body,
    Close,
}

/// Un tramo de una línea con su estilo.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Span {
    pub range: Range<usize>,
    pub style: Style,
    /// Qué hará el live preview con este tramo de sintaxis cuando el
    /// cursor no esté en la línea. Hoy solo se atenúa.
    pub conceal: Option<Conceal>,
    /// El link de [`LineStyle::links`] que pinta este tramo.
    pub link: Option<usize>,
}

/// Cómo se pinta un tramo: un color (`role`) y lo que se le suma.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct Style {
    pub role: Role,
    pub decorations: Decorations,
}

/// Lo que se suma al color de un tramo; pueden ir varias juntas.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct Decorations(u8);

impl Decorations {
    pub const STRONG: Self = Self(1);
    pub const EMPHASIS: Self = Self(1 << 1);
    pub const STRIKE: Self = Self(1 << 2);
    /// `==resaltado==`: lleva fondo.
    pub const MARK: Self = Self(1 << 3);
    /// Código inline: lleva fondo.
    pub const CODE: Self = Self(1 << 4);

    #[must_use]
    pub fn contains(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }

    #[must_use]
    pub fn with(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }
}

/// De qué es el texto, que decide su color.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum Role {
    #[default]
    Text,
    /// Los marcadores de la sintaxis: `#`, `**`, `[[`, `` ` ``, `>`, `\`.
    Markup,
    Heading,
    /// El texto de un link markdown a una nota o archivo del vault.
    Link,
    /// El texto de un link markdown que sale del vault.
    Url,
    Wikilink,
    Embed,
    Tag,
    Code,
    Quote,
    ListMarker,
    Checkbox,
    /// El texto de una tarea marcada.
    TaskDone,
    Comment,
    BlockId,
    Frontmatter,
    FrontmatterKey,
    /// El lenguaje tras la cerca de un bloque de código.
    FenceInfo,
    /// Dentro de un bloque de código de un lenguaje conocido.
    Token(Token),
    /// El LaTeX de una fórmula, sin sus `$`.
    Math,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Token {
    Keyword,
    String,
    Number,
    Comment,
}

/// Lo que el live preview (paso 2) hace con un tramo de sintaxis.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Conceal {
    /// Se oculta: `**`, `[[`, `#` de un heading, la URL de un link.
    Hide,
    /// El `-`, `*` o `+` de un ítem se ve como viñeta.
    Bullet,
    /// `[ ]` o `[x]` se ve como casilla.
    Checkbox { checked: bool },
    /// Cada `>` de una cita se ve como una barra.
    QuoteBar,
    /// Una regla horizontal se ve como una línea.
    Rule,
    /// El LaTeX de una fórmula de una sola línea: el tramo, con sus `$`,
    /// se reemplaza por la fórmula compuesta ([`MathSpot`]). Un bloque de
    /// varias líneas no se reemplaza: su fórmula va debajo de su cierre.
    Math { display: bool },
}

/// Una fórmula que se compone en una línea.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MathSpot {
    /// Lo que ocupa en la línea, con sus `$`. En la línea que cierra un
    /// bloque de varias líneas, hasta su `$$`.
    pub range: Range<usize>,
    /// El LaTeX, sin los `$`; el de todas las líneas de un bloque.
    pub latex: String,
    /// `$$…$$`: se compone como bloque, más grande.
    pub display: bool,
    /// Cuántas líneas arriba abrió: 0 si está entera en esta línea. Si no,
    /// la línea cierra un bloque y la fórmula va debajo de ella.
    pub above: usize,
}

impl MathSpot {
    /// Si va debajo de la línea en vez de reemplazar su tramo.
    #[must_use]
    pub fn below(&self) -> bool {
        self.above > 0
    }
}

/// Un link de una línea: dónde está y a qué apunta.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinkSpot {
    /// Todo el link, en bytes desde el inicio de la línea.
    pub range: Range<usize>,
    /// El destino como lo resuelve el vault (`Link::target_text`); vacío si
    /// apunta a la misma nota (`[[#Heading]]`).
    pub target: String,
    pub embed: bool,
}
