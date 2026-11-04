//! Las siluetas y las líneas de la pizarra como mallas de triángulos de lyon,
//! en coordenadas de la pizarra.

use lyon::math::{Angle, Box2D, point, vector};
use lyon::path::builder::BorderRadii;
use lyon::path::{Path, Winding};
use lyon::tessellation::{
    BuffersBuilder, FillOptions, FillTessellator, FillVertex, LineCap, LineJoin, StrokeOptions,
    StrokeTessellator, StrokeVertex, TessellationError, VertexBuffers,
};
use metalmind_board::{Point, Rect, Shape};

/// `u32`: un trazo largo pasa de los 65 536 vértices de `u16`.
pub type Mesh = VertexBuffers<lyon::math::Point, u32>;

/// Lo que se aparta una curva de la original, en unidades de la pizarra. Así
/// se ve lisa hasta con bastante zoom.
const TOLERANCE: f32 = 0.05;
const HALF: f32 = 0.5;

pub fn fill(path: &Path) -> Result<Mesh, TessellationError> {
    let mut mesh = Mesh::new();
    FillTessellator::new().tessellate_path(
        path,
        &FillOptions::tolerance(TOLERANCE),
        &mut BuffersBuilder::new(&mut mesh, |vertex: FillVertex| vertex.position()),
    )?;
    Ok(mesh)
}

pub fn stroke(path: &Path, width: f32) -> Result<Mesh, TessellationError> {
    let mut mesh = Mesh::new();
    let options = StrokeOptions::tolerance(TOLERANCE)
        .with_line_width(width)
        .with_line_join(LineJoin::Round)
        .with_line_cap(LineCap::Round);
    StrokeTessellator::new().tessellate_path(
        path,
        &options,
        &mut BuffersBuilder::new(&mut mesh, |vertex: StrokeVertex| vertex.position()),
    )?;
    Ok(mesh)
}

pub fn silhouette(rect: Rect, shape: Shape, radius: f32) -> Path {
    let center = rect.center();
    match shape {
        Shape::Rectangle | Shape::None => rounded(rect, radius),
        Shape::Ellipse => {
            let mut builder = Path::builder();
            builder.add_ellipse(
                point(center.x, center.y),
                vector(rect.w * HALF, rect.h * HALF),
                Angle::zero(),
                Winding::Positive,
            );
            builder.build()
        }
        Shape::Diamond => polyline(
            &[
                Point::new(center.x, rect.y),
                Point::new(rect.x + rect.w, center.y),
                Point::new(center.x, rect.y + rect.h),
                Point::new(rect.x, center.y),
            ],
            true,
        ),
    }
}

pub fn rounded(rect: Rect, radius: f32) -> Path {
    let mut builder = Path::builder();
    builder.add_rounded_rectangle(
        &Box2D::new(
            point(rect.x, rect.y),
            point(rect.x + rect.w, rect.y + rect.h),
        ),
        &BorderRadii::new(radius),
        Winding::Positive,
    );
    builder.build()
}

pub fn polyline(points: &[Point], closed: bool) -> Path {
    let mut builder = Path::builder();
    let mut points = points.iter();
    if let Some(first) = points.next() {
        builder.begin(point(first.x, first.y));
        for next in points {
            builder.line_to(point(next.x, next.y));
        }
        builder.end(closed);
    }
    builder.build()
}

pub fn cubic(curve: [Point; 4]) -> Path {
    let [start, c1, c2, end] = curve;
    let mut builder = Path::builder();
    builder.begin(point(start.x, start.y));
    builder.cubic_bezier_to(point(c1.x, c1.y), point(c2.x, c2.y), point(end.x, end.y));
    builder.end(false);
    builder.build()
}

/// Un trazo de puntos crudos, suavizado con curvas por los puntos medios.
pub fn smooth(points: &[Point]) -> Path {
    let mut builder = Path::builder();
    let (Some(first), Some(last)) = (points.first(), points.last()) else {
        return builder.build();
    };
    builder.begin(point(first.x, first.y));
    for pair in points.windows(2) {
        builder.quadratic_bezier_to(
            point(pair[0].x, pair[0].y),
            point(
                f32::midpoint(pair[0].x, pair[1].x),
                f32::midpoint(pair[0].y, pair[1].y),
            ),
        );
    }
    builder.line_to(point(last.x, last.y));
    builder.end(false);
    builder.build()
}

/// Lo que ocupa una malla.
#[must_use]
pub fn mesh_bounds(mesh: &Mesh) -> Option<Rect> {
    let mut vertices = mesh.vertices.iter();
    let first = vertices.next()?;
    let start = Rect {
        x: first.x,
        y: first.y,
        w: 0.0,
        h: 0.0,
    };
    Some(vertices.fold(start, |bounds, vertex| {
        bounds.union(&Rect {
            x: vertex.x,
            y: vertex.y,
            w: 0.0,
            h: 0.0,
        })
    }))
}

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use super::*;

    const BOX: Rect = Rect {
        x: 10.0,
        y: 20.0,
        w: 200.0,
        h: 100.0,
    };

    #[rstest]
    #[case(Shape::Rectangle)]
    #[case(Shape::Ellipse)]
    #[case(Shape::Diamond)]
    fn el_relleno_de_una_silueta_ocupa_su_caja(#[case] shape: Shape) {
        let mesh = fill(&silhouette(BOX, shape, 8.0)).unwrap();
        let bounds = mesh_bounds(&mesh).unwrap();
        assert!((bounds.x - BOX.x).abs() < 0.1 && (bounds.w - BOX.w).abs() < 0.1);
        assert!((bounds.y - BOX.y).abs() < 0.1 && (bounds.h - BOX.h).abs() < 0.1);
    }

    #[test]
    fn el_trazo_se_sale_la_mitad_de_su_ancho() {
        let mesh = stroke(&silhouette(BOX, Shape::Rectangle, 8.0), 2.0).unwrap();
        let bounds = mesh_bounds(&mesh).unwrap();
        assert!((bounds.x - (BOX.x - 1.0)).abs() < 0.1);
        assert!((bounds.w - (BOX.w + 2.0)).abs() < 0.1);
    }

    #[test]
    fn un_trazo_sin_puntos_no_pinta_nada() {
        assert!(mesh_bounds(&stroke(&smooth(&[]), 2.0).unwrap()).is_none());
    }
}
