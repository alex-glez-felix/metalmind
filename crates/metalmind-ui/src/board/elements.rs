//! Qué piezas y textos lleva cada nodo y cada flecha.

use lyon::tessellation::TessellationError;
use metalmind_board::{Edge, EdgeGeometry, EdgePath, Node, NodeKind, Rect, Shape, Tip, arrowhead};
use metalmind_syntax::note_name;

use super::scene::{ElementScene, Label, LabelKind, Piece, SceneStyle};
use super::shapes;
use super::tint::{Role, Tint};

type Parts = (Vec<Piece>, Vec<Label>);

pub(super) fn node_scene(
    node: &Node,
    style: SceneStyle,
) -> Result<ElementScene, TessellationError> {
    let rect = node.rect();
    let tint = Tint::parse(node.color.as_deref());
    let (pieces, labels) = match &node.kind {
        NodeKind::Text { text, shape } => text_box(rect, text, *shape, tint, style)?,
        NodeKind::File { file, subpath } => note_card(rect, file, subpath.as_deref(), tint, style)?,
        NodeKind::Link { url } => link_card(rect, url, tint, style)?,
        NodeKind::Group { label } => group(rect, label.as_deref(), tint, style)?,
        NodeKind::Stroke {
            stroke_width,
            points,
        } => {
            let at: Vec<_> = points
                .iter()
                .map(metalmind_board::StrokePoint::position)
                .map(|p| metalmind_board::Point::new(rect.x + p.x, rect.y + p.y))
                .collect();
            let mesh = shapes::stroke(&shapes::smooth(&at), *stroke_width)?;
            let line = Piece {
                role: Role::Line(tint),
                mesh,
            };
            (vec![line], Vec::new())
        }
    };
    Ok(ElementScene::new(pieces, labels, rect))
}

fn inner(rect: Rect, style: SceneStyle) -> Rect {
    let padding = style.padding;
    Rect {
        x: rect.x + padding,
        y: rect.y + padding,
        w: (rect.w - padding - padding).max(1.0),
        h: (rect.h - padding - padding).max(1.0),
    }
}

/// Lo que se ve de un texto: el markdown de un nodo sin sus marcas, porque la
/// pizarra no lo compone (todavía no hay texto con estilo por tramos).
fn visible_text(raw: &str) -> (String, bool) {
    let heading = raw.strip_prefix("# ");
    (heading.unwrap_or(raw).replace("**", ""), heading.is_some())
}

/// El rectángulo que queda libre para el texto en la parte ancha de la silueta.
fn text_area(rect: Rect, shape: Shape, style: SceneStyle) -> Rect {
    const SIDE_INSET: f32 = 0.18;
    let inner = inner(rect, style);
    match shape {
        Shape::Ellipse | Shape::Diamond => {
            let inset = rect.w * SIDE_INSET;
            Rect {
                x: rect.x + inset,
                w: (rect.w - inset - inset).max(1.0),
                ..inner
            }
        }
        Shape::Rectangle | Shape::None => inner,
    }
}

fn text_box(
    rect: Rect,
    raw: &str,
    shape: Shape,
    tint: Tint,
    style: SceneStyle,
) -> Result<Parts, TessellationError> {
    let (text, heading) = visible_text(raw);
    let kind = match (heading, shape) {
        (true, _) => LabelKind::Heading,
        (false, Shape::None) => LabelKind::Free,
        (false, _) => LabelKind::Body,
    };
    let label = Label {
        text: text.into(),
        kind,
        area: text_area(rect, shape, style),
    };
    if shape == Shape::None {
        return Ok((Vec::new(), vec![label]));
    }
    let silhouette = shapes::silhouette(rect, shape, style.radius);
    let pieces = vec![
        Piece {
            role: Role::Fill(tint),
            mesh: shapes::fill(&silhouette)?,
        },
        Piece {
            role: Role::Line(tint),
            mesh: shapes::stroke(&silhouette, style.line_width)?,
        },
    ];
    Ok((pieces, vec![label]))
}

fn card_pieces(rect: Rect, tint: Tint, style: SceneStyle) -> Result<Vec<Piece>, TessellationError> {
    let outline = shapes::rounded(rect, style.radius);
    Ok(vec![
        Piece {
            role: Role::Card,
            mesh: shapes::fill(&outline)?,
        },
        Piece {
            role: Role::Line(tint),
            mesh: shapes::stroke(&outline, style.line_width)?,
        },
    ])
}

/// Una tarjeta de nota: su título y su ruta, con la marca de acento.
fn note_card(
    rect: Rect,
    file: &str,
    subpath: Option<&str>,
    tint: Tint,
    style: SceneStyle,
) -> Result<Parts, TessellationError> {
    let mut pieces = card_pieces(rect, tint, style)?;
    let accent = Rect {
        w: style.card_accent,
        ..rect
    };
    pieces.push(Piece {
        role: Role::CardAccent,
        mesh: shapes::fill(&shapes::rounded(
            accent,
            style.radius.min(style.card_accent),
        ))?,
    });
    let area = inner(rect, style);
    let title_height = style.text_line().min(area.h);
    let labels = vec![
        Label {
            text: card_title(file, subpath).into(),
            kind: LabelKind::CardTitle,
            area: Rect {
                h: title_height,
                ..area
            },
        },
        Label {
            text: file.to_owned().into(),
            kind: LabelKind::CardPath,
            area: Rect {
                y: area.y + title_height,
                h: (area.h - title_height).max(0.0),
                ..area
            },
        },
    ];
    Ok((pieces, labels))
}

fn card_title(file: &str, subpath: Option<&str>) -> String {
    let name = file.rsplit('/').next().unwrap_or(file);
    let stem = note_name::strip_extension(name).unwrap_or(name);
    format!("{stem}{}", subpath.unwrap_or_default())
}

fn link_card(
    rect: Rect,
    url: &str,
    tint: Tint,
    style: SceneStyle,
) -> Result<Parts, TessellationError> {
    let label = Label {
        text: url.to_owned().into(),
        kind: LabelKind::CardPath,
        area: inner(rect, style),
    };
    Ok((card_pieces(rect, tint, style)?, vec![label]))
}

fn group(
    rect: Rect,
    title: Option<&str>,
    tint: Tint,
    style: SceneStyle,
) -> Result<Parts, TessellationError> {
    let piece = Piece {
        role: Role::Group(tint),
        mesh: shapes::fill(&shapes::rounded(rect, style.radius))?,
    };
    let height = style.text_line();
    let labels = title
        .map(|title| Label {
            text: title.to_owned().into(),
            kind: LabelKind::Group,
            area: Rect {
                y: rect.y - height,
                h: height,
                ..rect
            },
        })
        .into_iter()
        .collect();
    Ok((vec![piece], labels))
}

pub(super) fn edge_scene(
    edge: &Edge,
    geometry: &EdgeGeometry,
    style: SceneStyle,
) -> Result<ElementScene, TessellationError> {
    const HALF: f32 = 0.5;
    let tint = Tint::parse(edge.color.as_deref());
    let path = match &geometry.path {
        EdgePath::Polyline(points) => shapes::polyline(points, false),
        EdgePath::Cubic(curve) => shapes::cubic(*curve),
    };
    let line = shapes::stroke(&path, style.line_width)?;
    let outline = shapes::mesh_bounds(&line).unwrap_or(Rect {
        x: geometry.label_at.x,
        y: geometry.label_at.y,
        w: 0.0,
        h: 0.0,
    });
    let mut pieces = vec![Piece {
        role: Role::Line(tint),
        mesh: line,
    }];
    let tips = [
        (edge.from.tip, geometry.start(), geometry.start_from),
        (edge.to.tip, geometry.end(), geometry.end_from),
    ];
    for (tip, at, from) in tips {
        if tip == Tip::Arrow {
            let head = arrowhead(at, from, style.arrow_size);
            pieces.push(Piece {
                role: Role::Line(tint),
                mesh: shapes::fill(&shapes::polyline(&head, true))?,
            });
        }
    }
    let labels = edge
        .label
        .as_ref()
        .map(|label| {
            let (text, _) = visible_text(label);
            let (w, h) = (style.edge_label_width, style.small_line());
            Label {
                text: text.into(),
                kind: LabelKind::Edge,
                area: Rect {
                    x: geometry.label_at.x - w * HALF,
                    y: geometry.label_at.y - h * HALF,
                    w,
                    h,
                },
            }
        })
        .into_iter()
        .collect();
    Ok(ElementScene::new(pieces, labels, outline))
}
