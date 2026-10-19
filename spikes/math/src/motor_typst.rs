//! `mitex` convierte el LaTeX a código de math de Typst, que se evalúa con
//! el `mitex-scope` del paquete de Typst de mitex y se compone con un
//! `World` en memoria: sin disco, sin red, sin paquetes.
//!
//! El paquete de Typst va embebido (`assets/mitex`, de upstream: el de
//! crates.io 0.2.4 usa nombres que Typst 0.14 y 0.15 quitaron), y la spec
//! de comandos que usa el convertidor sale de ese mismo paquete al
//! arrancar, como la arma el `build.rs` de mitex con `typst query`: así la
//! spec y el scope no pueden discrepar.

use std::collections::HashMap;
use std::fmt::Write as _;

use typst::diag::{FileError, FileResult, SourceDiagnostic};
use typst::foundations::{Bytes, Datetime, Duration, Label};
use typst::introspection::{Introspector, MetadataElem};
use typst::layout::{Abs, FrameItem};
use typst::syntax::{FileId, RootedPath, Source, VirtualPath, VirtualRoot};
use typst::text::{Font, FontBook, FontWeight};
use typst::utils::{LazyHash, PicoStr};
use typst::{Library, LibraryExt, World};
use typst_layout::PagedDocument;

use crate::{Engine, Image, Look, MathError, Mode, Phases, Rendered, timed};

/// Los archivos del paquete de Typst de mitex.
const MITEX: [(&str, &str); 3] = [
    ("/mitex/mod.typ", include_str!("../assets/mitex/mod.typ")),
    (
        "/mitex/prelude.typ",
        include_str!("../assets/mitex/prelude.typ"),
    ),
    (
        "/mitex/latex/standard.typ",
        include_str!("../assets/mitex/latex/standard.typ"),
    ),
];

/// Donde va el código que sale de mitex; el documento lo lee con `read`.
const FORMULA: &str = "/formula.txt";

pub struct TypstEngine {
    world: MathWorld,
    spec: mitex::CommandSpec,
}

impl TypstEngine {
    /// Carga la biblioteca y las fuentes embebidas de `typst-assets`.
    ///
    /// # Errors
    /// Si un archivo embebido no es una ruta válida.
    pub fn new() -> Result<Self, MathError> {
        let mut world = MathWorld::new()?;
        let spec = world.query_spec()?;
        Ok(Self { world, spec })
    }

    /// El código de Typst que mitex saca del LaTeX.
    ///
    /// # Errors
    /// Si mitex no entiende el LaTeX.
    pub fn convert(&self, latex: &str) -> Result<String, MathError> {
        mitex::convert_math(latex, Some(self.spec.clone())).map_err(MathError::Convert)
    }

    /// Compone `code` (lo que dio [`Self::convert`]).
    ///
    /// # Errors
    /// Si Typst no puede evaluar o componer el código.
    pub fn compose(
        &mut self,
        code: &str,
        mode: Mode,
        look: &Look,
    ) -> Result<PagedDocument, MathError> {
        self.world.load(code, mode, look);
        self.world.compile()
    }

    /// Los píxeles de un documento ya compuesto.
    ///
    /// # Errors
    /// Si el documento no tiene página.
    pub fn rasterize(document: &PagedDocument, look: &Look) -> Result<Image, MathError> {
        let page = document
            .pages()
            .first()
            .ok_or_else(|| MathError::Render("el documento no tiene páginas".into()))?;
        let options = typst_render::RenderOptions {
            pixel_per_pt: f64::from(look.scale).into(),
            render_bleed: false,
        };
        let pixmap = typst_render::render(page, &options);
        let baseline = baseline(&page.frame).to_pt() * f64::from(look.scale);
        #[expect(
            clippy::cast_possible_truncation,
            reason = "una línea base en píxeles cabe en f32"
        )]
        Ok(Image::from_premultiplied(
            pixmap.width(),
            pixmap.height(),
            baseline as f32,
            pixmap.take(),
        ))
    }
}

impl Engine for TypstEngine {
    fn render(&mut self, latex: &str, mode: Mode, look: &Look) -> Result<Rendered, MathError> {
        let (code, convert) = timed(|| self.convert(latex));
        let code = code?;
        let (document, layout) = timed(|| self.compose(&code, mode, look));
        let document = document?;
        let (image, raster) = timed(|| Self::rasterize(&document, look));
        Ok(Rendered {
            image: image?,
            phases: Phases {
                convert,
                layout,
                raster,
            },
        })
    }
}

/// La New Computer Modern Math de `typst-assets`, para que el camino de
/// MathML use la misma fuente y se comparen solo los motores.
///
/// # Errors
/// Si `typst-assets` no la trae.
pub fn math_font() -> Result<&'static [u8], MathError> {
    typst_assets::fonts()
        .find(|data| {
            Font::iter(Bytes::new(*data)).any(|font| {
                font.info().family == "New Computer Modern Math"
                    && font.info().variant.weight == FontWeight::REGULAR
            })
        })
        .ok_or_else(|| MathError::Layout("falta New Computer Modern Math".into()))
}

/// La línea base del primer texto que encuentra: la de la ecuación.
fn baseline(frame: &typst::layout::Frame) -> Abs {
    frame
        .items()
        .find_map(|(point, item)| match item {
            FrameItem::Group(group) => Some(point.y + baseline(&group.frame)),
            FrameItem::Text(_) => Some(point.y),
            _ => None,
        })
        .unwrap_or_else(|| frame.baseline())
}

fn describe(error: &SourceDiagnostic) -> String {
    let mut text = error.message.to_string();
    for hint in &error.hints {
        let _ = write!(text, " (hint: {})", hint.v);
    }
    text
}

struct MathWorld {
    library: LazyHash<Library>,
    book: LazyHash<FontBook>,
    fonts: Vec<Font>,
    main: FileId,
    document: Source,
    formula: Bytes,
    sources: HashMap<FileId, Source>,
}

impl MathWorld {
    fn new() -> Result<Self, MathError> {
        let fonts: Vec<Font> = typst_assets::fonts()
            .flat_map(|data| Font::iter(Bytes::new(data)))
            .collect();
        let main = project_file("/main.typ")?;
        let mut sources = HashMap::new();
        for (path, text) in MITEX {
            let id = project_file(path)?;
            sources.insert(id, Source::new(id, text.to_owned()));
        }
        Ok(Self {
            library: LazyHash::new(Library::default()),
            book: LazyHash::new(FontBook::from_fonts(&fonts)),
            fonts,
            main,
            document: Source::new(main, String::new()),
            formula: Bytes::new(Vec::new()),
            sources,
        })
    }

    fn compile(&self) -> Result<PagedDocument, MathError> {
        typst::compile::<PagedDocument>(self)
            .output
            .map_err(|errors| {
                MathError::Layout(errors.iter().map(describe).collect::<Vec<_>>().join("; "))
            })
    }

    /// La spec de los paquetes de mitex, del `<mitex-packages>` que publica
    /// `mod.typ`.
    fn query_spec(&mut self) -> Result<mitex::CommandSpec, MathError> {
        let spec_error = |error: String| MathError::Layout(format!("spec de mitex: {error}"));
        self.document = Source::new(
            self.main,
            "#import \"/mitex/mod.typ\": packages\n#metadata(packages) <mitex-packages>\n"
                .to_owned(),
        );
        let document = self.compile()?;
        let label = Label::new(PicoStr::intern("mitex-packages"))
            .ok_or_else(|| spec_error("etiqueta vacía".into()))?;
        let content = document
            .introspector()
            .query_label(label)
            .map_err(|error| spec_error(error.to_string()))?;
        let metadata = content
            .to_packed::<MetadataElem>()
            .ok_or_else(|| spec_error("la etiqueta no es un metadata".into()))?;
        let value =
            serde_json::to_value(&metadata.value).map_err(|error| spec_error(error.to_string()))?;
        let packages: mitex::query::PackagesVec =
            serde_json::from_value(value).map_err(|error| spec_error(error.to_string()))?;
        let mut spec = mitex::JsonCommandSpec::default();
        for package in packages.0 {
            spec.commands.extend(package.spec.commands);
        }
        Ok(spec.into())
    }

    fn load(&mut self, code: &str, mode: Mode, look: &Look) {
        let [r, g, b] = look.color;
        let block = matches!(mode, Mode::Display);
        let text = format!(
            "#import \"/mitex/mod.typ\": mitex-scope\n\
             #set page(width: auto, height: auto, margin: (x: 1pt, y: 2pt), fill: none)\n\
             #set text(size: {size}pt, fill: rgb({r}, {g}, {b}), top-edge: \"bounds\", bottom-edge: \"bounds\")\n\
             #math.equation(block: {block}, eval(\"$\" + read(\"{FORMULA}\") + \"$\", scope: mitex-scope))\n",
            size = look.size,
        );
        self.document = Source::new(self.main, text);
        self.formula = Bytes::new(code.as_bytes().to_vec());
    }
}

impl World for MathWorld {
    fn library(&self) -> &LazyHash<Library> {
        &self.library
    }

    fn book(&self) -> &LazyHash<FontBook> {
        &self.book
    }

    fn main(&self) -> FileId {
        self.main
    }

    fn source(&self, id: FileId) -> FileResult<Source> {
        if id == self.main {
            return Ok(self.document.clone());
        }
        self.sources.get(&id).cloned().ok_or_else(|| not_found(id))
    }

    fn file(&self, id: FileId) -> FileResult<Bytes> {
        if id.get().vpath().get_with_slash() == FORMULA {
            return Ok(self.formula.clone());
        }
        self.sources
            .get(&id)
            .map(|source| Bytes::new(source.text().as_bytes().to_vec()))
            .ok_or_else(|| not_found(id))
    }

    fn font(&self, index: usize) -> Option<Font> {
        self.fonts.get(index).cloned()
    }

    fn today(&self, _offset: Option<Duration>) -> Option<Datetime> {
        None
    }
}

fn not_found(id: FileId) -> FileError {
    FileError::NotFound(id.get().vpath().get_without_slash().into())
}

fn project_file(path: &str) -> Result<FileId, MathError> {
    let vpath = VirtualPath::new(path).map_err(|error| MathError::Layout(error.to_string()))?;
    Ok(RootedPath::new(VirtualRoot::Project, vpath).intern())
}
