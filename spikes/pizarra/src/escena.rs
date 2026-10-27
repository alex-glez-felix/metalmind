//! Lo que se pinta de cada elemento, en coordenadas de la pizarra: mallas
//! de triángulos ya teseladas (lyon) y textos. Se calcula al cargar y al
//! editar un elemento, no en cada frame: el pan y el zoom solo transforman.

use lyon::math::{Box2D, point, vector};
use lyon::path::builder::BorderRadii;
use lyon::path::{Path, Winding};
use lyon::tessellation::{
    BuffersBuilder, FillOptions, FillTessellator, FillVertex, StrokeOptions, StrokeTessellator,
    StrokeVertex, TessellationError, VertexBuffers,
};

use crate::formato::{Board, Edge, Node, NodeKind, Shape, Tip};
use crate::geometria::{self, EdgeGeometry, EdgePath, P, Rect};

/// `u32`: un trazo largo pasa de los 65 536 vértices de `u16`.
pub type Mesh = VertexBuffers<lyon::math::Point, u32>;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Style {
    Clean,
    /// A mano alzada con roughr; sin la feature `rough`, igual que `Clean`.
    Rough,
}

/// De qué color va una pieza: el tema decide el color real.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Role {
    /// El trazo de una forma o una flecha, con el color del elemento.
    Line(Option<String>),
    /// El relleno de una forma: el color del elemento, muy tenue.
    Fill(Option<String>),
    CardFill,
    CardAccent,
    GroupFill(Option<String>),
    LabelBackground,
}

#[derive(Clone, Debug)]
pub struct Piece {
    pub role: Role,
    pub mesh: Mesh,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LabelKind {
    /// Texto dentro de una caja: centrado y partido al ancho de la caja.
    Body,
    /// Texto suelto (`shape: none`): alineado a la izquierda.
    Free,
    CardTitle,
    CardPath,
    Group,
    Edge,
}

#[derive(Clone, Debug)]
pub struct Label {
    pub text: String,
    pub kind: LabelKind,
    /// El área donde va el texto, en coordenadas de la pizarra.
    pub area: Rect,
    pub color: Option<String>,
}

#[derive(Clone, Debug)]
pub struct ElementScene {
    pub pieces: Vec<Piece>,
    pub labels: Vec<Label>,
    /// La caja del nodo, o la de la línea de una flecha.
    pub outline: Rect,
    /// Todo lo que pinta: las mallas y los textos. Es lo que decide si se ve.
    pub bounds: Rect,
}

impl ElementScene {
    fn new(pieces: Vec<Piece>, labels: Vec<Label>, outline: Rect) -> Self {
        let painted = pieces
            .iter()
            .flat_map(|p| &p.mesh.vertices)
            .map(|v| Rect {
                x: v.x,
                y: v.y,
                w: 0.0,
                h: 0.0,
            })
            .chain(labels.iter().map(|l| l.area));
        let bounds = painted.fold(outline, union);
        Self {
            pieces,
            labels,
            outline,
            bounds,
        }
    }
}

fn union(a: Rect, b: Rect) -> Rect {
    let (x, y) = (a.x.min(b.x), a.y.min(b.y));
    Rect {
        x,
        y,
        w: (a.x + a.w).max(b.x + b.w) - x,
        h: (a.y + a.h).max(b.y + b.h) - y,
    }
}

pub const LINE_WIDTH: f32 = 2.0;
const RADIUS: f32 = 8.0;
const ARROW_SIZE: f32 = 12.0;
const TOLERANCE: f32 = 0.05;
pub const PADDING: f32 = 12.0;

/// La escena de toda la pizarra: un elemento por nodo y uno por flecha.
///
/// # Errors
///
/// Si lyon no puede teselar una forma.
pub fn build(
    board: &Board,
    style: Style,
) -> Result<(Vec<ElementScene>, Vec<ElementScene>), TessellationError> {
    let nodes = board
        .nodes
        .iter()
        .map(|node| node_scene(node, style))
        .collect::<Result<_, _>>()?;
    // `Board::parse` ya comprobó que cada flecha une dos nodos que existen.
    let edges = board
        .edges
        .iter()
        .filter_map(|edge| geometria::route(board, edge).map(|g| edge_scene(edge, &g, style)))
        .collect::<Result<_, _>>()?;
    Ok((nodes, edges))
}

/// # Errors
///
/// Si lyon no puede teselar la forma.
pub fn node_scene(node: &Node, style: Style) -> Result<ElementScene, TessellationError> {
    let rect = geometria::Rect::of(node);
    let (pieces, labels) = match &node.kind {
        NodeKind::Text { text, shape } => text_box(node, rect, text, *shape, style)?,
        NodeKind::File { file, subpath } => {
            let title = note_title(file, subpath.as_deref());
            card(node, rect, style, Some(title), file)?
        }
        NodeKind::Link { url } => card(node, rect, style, None, url)?,
        NodeKind::Group { label } => group(node, rect, label.as_deref())?,
        NodeKind::Stroke {
            stroke_width,
            points,
        } => {
            let path = smooth(points.iter().map(|p| P::new(rect.x + p.x, rect.y + p.y)));
            let mesh = stroke(&path, *stroke_width)?;
            (
                vec![Piece {
                    role: Role::Line(node.color.clone()),
                    mesh,
                }],
                Vec::new(),
            )
        }
    };
    Ok(ElementScene::new(pieces, labels, rect))
}

type Parts = (Vec<Piece>, Vec<Label>);

fn inner(rect: Rect) -> Rect {
    Rect {
        x: rect.x + PADDING,
        y: rect.y + PADDING,
        w: (rect.w - 2.0 * PADDING).max(1.0),
        h: (rect.h - 2.0 * PADDING).max(1.0),
    }
}

fn label(text: &str, kind: LabelKind, area: Rect, color: Option<String>) -> Label {
    Label {
        text: text.to_owned(),
        kind,
        area,
        color,
    }
}

/// Una caja con forma y su texto; con `Shape::None`, solo el texto.
fn text_box(
    node: &Node,
    rect: Rect,
    text: &str,
    shape: Shape,
    style: Style,
) -> Result<Parts, TessellationError> {
    if shape == Shape::None {
        return Ok((
            Vec::new(),
            vec![label(text, LabelKind::Free, inner(rect), None)],
        ));
    }
    let fill_piece = Piece {
        role: Role::Fill(node.color.clone()),
        mesh: fill(&shape_path(rect, shape))?,
    };
    let pieces = vec![
        fill_piece,
        outline_piece(node, rect, shape, style, node.color.clone())?,
    ];
    // En una elipse o un rombo el texto va en la parte ancha del centro.
    let area = match shape {
        Shape::Ellipse | Shape::Diamond => Rect {
            x: rect.x + rect.w * 0.18,
            w: rect.w * 0.64,
            ..inner(rect)
        },
        Shape::Rectangle | Shape::None => inner(rect),
    };
    Ok((pieces, vec![label(text, LabelKind::Body, area, None)]))
}

/// Una tarjeta: de una nota (título y ruta, con la marca de acento) o de un link.
fn card(
    node: &Node,
    rect: Rect,
    style: Style,
    title: Option<String>,
    path: &str,
) -> Result<Parts, TessellationError> {
    let outline = shape_path(rect, Shape::Rectangle);
    let mut pieces = vec![
        Piece {
            role: Role::CardFill,
            mesh: fill(&outline)?,
        },
        outline_piece(node, rect, Shape::Rectangle, style, node.color.clone())?,
    ];
    let area = inner(rect);
    let Some(title) = title else {
        return Ok((pieces, vec![label(path, LabelKind::CardPath, area, None)]));
    };
    let accent = rounded(Rect { w: 4.0, ..rect }, 2.0);
    pieces.push(Piece {
        role: Role::CardAccent,
        mesh: fill(&accent)?,
    });
    let labels = vec![
        label(&title, LabelKind::CardTitle, area, None),
        label(
            path,
            LabelKind::CardPath,
            Rect {
                y: area.y + 24.0,
                ..area
            },
            None,
        ),
    ];
    Ok((pieces, labels))
}

fn group(node: &Node, rect: Rect, title: Option<&str>) -> Result<Parts, TessellationError> {
    let mesh = fill(&shape_path(rect, Shape::Rectangle))?;
    let pieces = vec![Piece {
        role: Role::GroupFill(node.color.clone()),
        mesh,
    }];
    let above = Rect {
        y: rect.y - 26.0,
        h: 22.0,
        ..rect
    };
    let labels = title
        .map(|t| label(t, LabelKind::Group, above, node.color.clone()))
        .into_iter()
        .collect();
    Ok((pieces, labels))
}

/// # Errors
///
/// Si lyon no puede teselar la flecha.
pub fn edge_scene(
    edge: &Edge,
    geometry: &EdgeGeometry,
    style: Style,
) -> Result<ElementScene, TessellationError> {
    let color = edge.color.clone();
    let path = match &geometry.path {
        EdgePath::Polyline(points) => polyline(points),
        EdgePath::Cubic(c) => cubic(*c),
    };
    let mut pieces = vec![Piece {
        role: Role::Line(color.clone()),
        mesh: edge_line(edge, geometry, &path, style)?,
    }];
    for (tip, at, from) in [
        (edge.from.tip, geometry.start(), geometry.start_from),
        (edge.to.tip, geometry.end(), geometry.end_from),
    ] {
        if tip == Tip::Arrow {
            let head = geometria::arrowhead(at, from, ARROW_SIZE);
            let mut builder = Path::builder();
            builder.begin(point(head[0].x, head[0].y));
            builder.line_to(point(head[1].x, head[1].y));
            builder.line_to(point(head[2].x, head[2].y));
            builder.end(true);
            pieces.push(Piece {
                role: Role::Line(color.clone()),
                mesh: fill(&builder.build())?,
            });
        }
    }
    let mut labels = Vec::new();
    if let Some(label) = &edge.label {
        let area = Rect {
            x: geometry.label_at.x - 80.0,
            y: geometry.label_at.y - 11.0,
            w: 160.0,
            h: 22.0,
        };
        labels.push(Label {
            text: label.clone(),
            kind: LabelKind::Edge,
            area,
            color: None,
        });
    }
    let outline = path_bounds(&path).unwrap_or(Rect {
        x: geometry.label_at.x,
        y: geometry.label_at.y,
        w: 0.0,
        h: 0.0,
    });
    Ok(ElementScene::new(pieces, labels, outline))
}

fn note_title(file: &str, subpath: Option<&str>) -> String {
    let name = file.rsplit('/').next().unwrap_or(file);
    let stem = name.strip_suffix(".md").unwrap_or(name);
    match subpath {
        Some(sub) => format!("{stem}{sub}"),
        None => stem.to_owned(),
    }
}

fn path_bounds(path: &Path) -> Option<Rect> {
    let b = lyon::algorithms::aabb::bounding_box(path.iter());
    b.is_finite().then(|| Rect {
        x: b.min.x,
        y: b.min.y,
        w: b.width(),
        h: b.height(),
    })
}

fn shape_path(r: Rect, shape: Shape) -> Path {
    match shape {
        Shape::Rectangle | Shape::None => rounded(r, RADIUS),
        Shape::Ellipse => {
            let mut builder = Path::builder();
            let c = r.center();
            builder.add_ellipse(
                point(c.x, c.y),
                vector(r.w / 2.0, r.h / 2.0),
                lyon::math::Angle::zero(),
                Winding::Positive,
            );
            builder.build()
        }
        Shape::Diamond => {
            let c = r.center();
            polygon(&[
                P::new(c.x, r.y),
                P::new(r.x + r.w, c.y),
                P::new(c.x, r.y + r.h),
                P::new(r.x, c.y),
            ])
        }
    }
}

fn rounded(r: Rect, radius: f32) -> Path {
    let mut builder = Path::builder();
    builder.add_rounded_rectangle(
        &Box2D::new(point(r.x, r.y), point(r.x + r.w, r.y + r.h)),
        &BorderRadii::new(radius),
        Winding::Positive,
    );
    builder.build()
}

fn polygon(points: &[P]) -> Path {
    let mut builder = Path::builder();
    let mut iter = points.iter();
    if let Some(first) = iter.next() {
        builder.begin(point(first.x, first.y));
        for p in iter {
            builder.line_to(point(p.x, p.y));
        }
        builder.end(true);
    }
    builder.build()
}

fn polyline(points: &[P]) -> Path {
    let mut builder = Path::builder();
    let mut iter = points.iter();
    if let Some(first) = iter.next() {
        builder.begin(point(first.x, first.y));
        for p in iter {
            builder.line_to(point(p.x, p.y));
        }
        builder.end(false);
    }
    builder.build()
}

fn cubic(c: [P; 4]) -> Path {
    let mut builder = Path::builder();
    builder.begin(point(c[0].x, c[0].y));
    builder.cubic_bezier_to(
        point(c[1].x, c[1].y),
        point(c[2].x, c[2].y),
        point(c[3].x, c[3].y),
    );
    builder.end(false);
    builder.build()
}

/// Un trazo de puntos crudos, suavizado con curvas por los puntos medios.
fn smooth(points: impl Iterator<Item = P>) -> Path {
    let points: Vec<P> = points.collect();
    let mut builder = Path::builder();
    let Some(first) = points.first() else {
        return builder.build();
    };
    builder.begin(point(first.x, first.y));
    for pair in points.windows(2) {
        let mid = P::new(
            f32::midpoint(pair[0].x, pair[1].x),
            f32::midpoint(pair[0].y, pair[1].y),
        );
        builder.quadratic_bezier_to(point(pair[0].x, pair[0].y), point(mid.x, mid.y));
    }
    if let Some(last) = points.last() {
        builder.line_to(point(last.x, last.y));
    }
    builder.end(false);
    builder.build()
}

/// # Errors
///
/// Si lyon no puede teselar el camino.
pub fn fill(path: &Path) -> Result<Mesh, TessellationError> {
    let mut mesh = Mesh::new();
    FillTessellator::new().tessellate_path(
        path,
        &FillOptions::tolerance(TOLERANCE),
        &mut BuffersBuilder::new(&mut mesh, |v: FillVertex| v.position()),
    )?;
    Ok(mesh)
}

/// # Errors
///
/// Si lyon no puede teselar el camino.
pub fn stroke(path: &Path, width: f32) -> Result<Mesh, TessellationError> {
    let mut mesh = Mesh::new();
    let options = StrokeOptions::tolerance(TOLERANCE)
        .with_line_width(width)
        .with_line_join(lyon::tessellation::LineJoin::Round)
        .with_line_cap(lyon::tessellation::LineCap::Round);
    StrokeTessellator::new().tessellate_path(
        path,
        &options,
        &mut BuffersBuilder::new(&mut mesh, |v: StrokeVertex| v.position()),
    )?;
    Ok(mesh)
}

fn outline_piece(
    node: &Node,
    r: Rect,
    shape: Shape,
    style: Style,
    color: Option<String>,
) -> Result<Piece, TessellationError> {
    let path = match style {
        Style::Rough => rough::outline(&node.id, r, shape),
        Style::Clean => None,
    };
    let mesh = stroke(&path.unwrap_or_else(|| shape_path(r, shape)), LINE_WIDTH)?;
    Ok(Piece {
        role: Role::Line(color),
        mesh,
    })
}

fn edge_line(
    edge: &Edge,
    geometry: &EdgeGeometry,
    clean: &Path,
    style: Style,
) -> Result<Mesh, TessellationError> {
    let path = match style {
        Style::Rough => rough::edge(&edge.id, &geometry.path),
        Style::Clean => None,
    };
    stroke(path.as_ref().unwrap_or(clean), LINE_WIDTH)
}

#[cfg(feature = "rough")]
mod rough {
    //! La semilla sale del id del elemento: el mismo trazo en cada apertura
    //! sin guardar nada en el archivo (Excalidraw guarda `seed`).

    use lyon::math::point;
    use lyon::path::Path;
    use roughr::Point2D;
    use roughr::core::{Drawable, OpSetType, OpType, Options, OptionsBuilder};
    use roughr::generator::Generator;

    use super::{LINE_WIDTH, Rect, Shape};
    use crate::geometria::{EdgePath, P};

    /// FNV-1a: estable entre versiones de Rust, a diferencia de `DefaultHasher`.
    fn seed(id: &str) -> u64 {
        id.bytes().fold(0xcbf2_9ce4_8422_2325_u64, |h, b| {
            (h ^ u64::from(b)).wrapping_mul(0x100_0000_01b3)
        })
    }

    fn options(id: &str) -> Option<Options> {
        OptionsBuilder::default()
            .seed(seed(id))
            .roughness(1.0)
            .stroke_width(LINE_WIDTH)
            .build()
            .ok()
    }

    /// `None` si roughr no acepta las opciones: se pinta con trazo limpio.
    pub fn outline(id: &str, r: Rect, shape: Shape) -> Option<Path> {
        let generator = Generator::default();
        let options = options(id)?;
        let options = Some(options);
        let drawable = match shape {
            Shape::Ellipse => {
                let c = r.center();
                generator.ellipse(c.x, c.y, r.w, r.h, &options)
            }
            Shape::Diamond => {
                let c = r.center();
                let points = [(c.x, r.y), (r.x + r.w, c.y), (c.x, r.y + r.h), (r.x, c.y)]
                    .map(|(x, y)| Point2D::new(x, y));
                generator.polygon(&points, &options)
            }
            Shape::Rectangle | Shape::None => generator.rectangle(r.x, r.y, r.w, r.h, &options),
        };
        Some(to_path(&drawable))
    }

    pub fn edge(id: &str, path: &EdgePath) -> Option<Path> {
        let generator = Generator::default();
        let options = Some(options(id)?);
        let to2d = |p: &P| Point2D::new(p.x, p.y);
        let drawable = match path {
            EdgePath::Polyline(points) => {
                let points: Vec<_> = points.iter().map(to2d).collect();
                generator.linear_path(&points, false, &options)
            }
            EdgePath::Cubic(c) => {
                generator.bezier_cubic(to2d(&c[0]), to2d(&c[1]), to2d(&c[2]), to2d(&c[3]), &options)
            }
        };
        Some(to_path(&drawable))
    }

    fn to_path(drawable: &Drawable<f32>) -> Path {
        let mut builder = Path::builder().with_svg();
        for set in drawable
            .sets
            .iter()
            .filter(|s| s.op_set_type == OpSetType::Path)
        {
            for op in &set.ops {
                match (&op.op, op.data.as_slice()) {
                    (OpType::Move, [x, y, ..]) => {
                        builder.move_to(point(*x, *y));
                    }
                    (OpType::LineTo, [x, y, ..]) => {
                        builder.line_to(point(*x, *y));
                    }
                    (OpType::BCurveTo, [x1, y1, x2, y2, x, y, ..]) => {
                        builder.cubic_bezier_to(point(*x1, *y1), point(*x2, *y2), point(*x, *y));
                    }
                    _ => {}
                }
            }
        }
        builder.build()
    }
}

#[cfg(not(feature = "rough"))]
mod rough {
    use lyon::path::Path;

    use super::{Rect, Shape};
    use crate::geometria::EdgePath;

    pub fn outline(_id: &str, _r: Rect, _shape: Shape) -> Option<Path> {
        None
    }

    pub fn edge(_id: &str, _path: &EdgePath) -> Option<Path> {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::generar;

    #[test]
    fn every_element_of_the_example_paints_something() {
        let board = generar::example();
        for style in [Style::Clean, Style::Rough] {
            let (nodes, edges) = build(&board, style).unwrap();
            assert_eq!(
                (nodes.len(), edges.len()),
                (board.nodes.len(), board.edges.len())
            );
            for (node, scene) in board.nodes.iter().zip(&nodes) {
                let paints = scene.pieces.iter().any(|p| !p.mesh.indices.is_empty())
                    || !scene.labels.is_empty();
                assert!(paints, "{} no pinta nada", node.id);
            }
            assert!(
                edges
                    .iter()
                    .all(|e| e.pieces.iter().all(|p| !p.mesh.indices.is_empty()))
            );
        }
    }

    #[test]
    fn a_note_card_shows_its_title_and_path() {
        let board = generar::example();
        let card = board.node("nota-03").unwrap();
        let scene = node_scene(card, Style::Clean).unwrap();
        let texts: Vec<_> = scene
            .labels
            .iter()
            .map(|l| (l.kind, l.text.as_str()))
            .collect();
        assert!(
            texts.contains(&(LabelKind::CardTitle, "03-core-architecture")),
            "{texts:?}"
        );
        assert!(
            texts.contains(&(LabelKind::CardPath, "docs/research/03-core-architecture.md")),
            "{texts:?}"
        );
    }

    #[test]
    fn what_is_painted_is_inside_the_bounds() {
        let board = generar::example();
        let (nodes, edges) = build(&board, Style::Clean).unwrap();
        for scene in nodes.iter().chain(&edges) {
            let b = scene.bounds;
            for v in scene.pieces.iter().flat_map(|p| &p.mesh.vertices) {
                assert!(
                    v.x >= b.x && v.x <= b.x + b.w && v.y >= b.y && v.y <= b.y + b.h,
                    "{v:?} fuera de {b:?}"
                );
            }
            for l in &scene.labels {
                assert!(
                    l.area.y >= b.y && l.area.x >= b.x,
                    "{} fuera de {b:?}",
                    l.text
                );
            }
        }
        // La etiqueta de un grupo va arriba de su caja y aun así cuenta.
        let group = &nodes[0];
        assert!(group.bounds.y < group.outline.y);
    }

    #[test]
    fn a_wide_stroke_counts_its_whole_width() {
        let mut board = generar::example();
        let rodeo = board.nodes.iter_mut().find(|n| n.id == "rodeo").unwrap();
        if let NodeKind::Stroke { stroke_width, .. } = &mut rodeo.kind {
            *stroke_width = 60.0;
        }
        let scene = node_scene(board.node("rodeo").unwrap(), Style::Clean).unwrap();
        assert!(
            scene.bounds.x < scene.outline.x - 20.0,
            "{:?} {:?}",
            scene.bounds,
            scene.outline
        );
    }

    #[cfg(feature = "rough")]
    #[test]
    fn the_rough_stroke_is_the_same_every_time_and_differs_per_element() {
        let board = generar::example();
        let line = |id: &str| {
            let scene = node_scene(board.node(id).unwrap(), Style::Rough).unwrap();
            scene
                .pieces
                .iter()
                .find(|p| matches!(p.role, Role::Line(_)))
                .map(|p| p.mesh.vertices.clone())
                .unwrap()
        };
        assert_eq!(line("formato"), line("formato"));
        let clean = node_scene(board.node("formato").unwrap(), Style::Clean).unwrap();
        let clean_line = clean
            .pieces
            .iter()
            .find(|p| matches!(p.role, Role::Line(_)))
            .map(|p| p.mesh.vertices.clone())
            .unwrap();
        assert_ne!(line("formato"), clean_line);
    }
}
