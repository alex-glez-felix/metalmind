//! Pinta la escena: cada malla, ya teselada en coordenadas de la pizarra,
//! solo se transforma con la cámara. Lo que queda fuera de la ventana no se
//! toca.

use gpui_kit::{
    App, BorderStyle, Bounds, ContentMask, Font, FontWeight, Hsla, Path, Pixels, Point,
    SharedString, TextAlign, TextRun, Window, fill, font, point, px, quad, size,
};
use metalmind_spike_pizarra::escena::{ElementScene, Label, LabelKind, Mesh, Role};
use metalmind_spike_pizarra::geometria::Rect;

use crate::tema::Tema;

/// Qué parte de la pizarra se ve: `(x, y)` de la pizarra en la esquina de
/// arriba a la izquierda y cuántos píxeles mide una unidad.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Camara {
    pub x: f32,
    pub y: f32,
    pub zoom: f32,
}

impl Camara {
    pub const ZOOM_MIN: f32 = 0.05;
    pub const ZOOM_MAX: f32 = 8.0;

    pub fn a_pantalla(&self, origen: Point<Pixels>, x: f32, y: f32) -> Point<Pixels> {
        point(
            origen.x + px((x - self.x) * self.zoom),
            origen.y + px((y - self.y) * self.zoom),
        )
    }

    pub fn a_pizarra(&self, origen: Point<Pixels>, p: Point<Pixels>) -> (f32, f32) {
        (
            self.x + f32::from(p.x - origen.x) / self.zoom,
            self.y + f32::from(p.y - origen.y) / self.zoom,
        )
    }

    pub fn visible(&self, ancho: f32, alto: f32) -> Rect {
        Rect {
            x: self.x,
            y: self.y,
            w: ancho / self.zoom,
            h: alto / self.zoom,
        }
    }

    /// Cambia el zoom dejando quieto el punto de la pizarra bajo `ancla`.
    pub fn acercar(&mut self, factor: f32, origen: Point<Pixels>, ancla: Point<Pixels>) {
        let (ax, ay) = self.a_pizarra(origen, ancla);
        let zoom = (self.zoom * factor).clamp(Self::ZOOM_MIN, Self::ZOOM_MAX);
        self.x = ax - f32::from(ancla.x - origen.x) / zoom;
        self.y = ay - f32::from(ancla.y - origen.y) / zoom;
        self.zoom = zoom;
    }

    /// La cámara que deja `caja` entera en una ventana de `ancho × alto`.
    pub fn para_ver(caja: Rect, ancho: f32, alto: f32) -> Self {
        let margen = 40.0;
        let zoom = ((ancho - 2.0 * margen) / caja.w.max(1.0))
            .min((alto - 2.0 * margen) / caja.h.max(1.0))
            .clamp(Self::ZOOM_MIN, Self::ZOOM_MAX);
        Self {
            x: caja.x + caja.w / 2.0 - ancho / 2.0 / zoom,
            y: caja.y + caja.h / 2.0 - alto / 2.0 / zoom,
            zoom,
        }
    }
}

/// Por debajo de este zoom un nodo se pinta como un quad de GPUI (su caja,
/// relleno y borde) y no como su malla: de lejos no se distingue, y una
/// pizarra grande entera no cabe en el búfer de un frame de GPUI (09 §3).
pub const ZOOM_DETALLE: f32 = 0.35;

/// Lo que se pintó en un frame, para el HUD y el benchmark.
#[derive(Clone, Copy, Debug, Default)]
pub struct Pintado {
    pub elementos: usize,
    pub triangulos: usize,
    pub textos: usize,
}

/// Primero las flechas, luego los nodos encima y al final las etiquetas de
/// las flechas, para que una tarjeta nunca tape una etiqueta.
/// Dónde y cómo se pinta un frame.
pub struct Marco<'a> {
    pub camara: &'a Camara,
    pub area: Bounds<Pixels>,
    pub tema: &'a Tema,
    /// Nivel de detalle: de lejos, cada nodo es un quad.
    pub detalle: bool,
}

pub fn escena(
    nodos: &[ElementScene],
    flechas: &[ElementScene],
    marco: &Marco,
    window: &mut Window,
    cx: &mut App,
) -> Pintado {
    let Marco {
        camara,
        area,
        tema,
        detalle,
    } = *marco;
    let visible = camara.visible(f32::from(area.size.width), f32::from(area.size.height));
    let flechas: Vec<&ElementScene> = flechas
        .iter()
        .filter(|e| e.bounds.intersects(&visible))
        .collect();
    let nodos = nodos.iter().filter(|e| e.bounds.intersects(&visible));
    let mut pintado = Pintado::default();
    let de_lejos = detalle && camara.zoom < ZOOM_DETALLE;
    for elemento in flechas.iter().copied() {
        pintado.elementos += 1;
        for pieza in &elemento.pieces {
            pintado.triangulos += malla(
                &pieza.mesh,
                camara,
                area.origin,
                tema.de_rol(&pieza.role),
                window,
            );
        }
    }
    for elemento in nodos.clone() {
        pintado.elementos += 1;
        if de_lejos {
            caja(elemento, camara, area.origin, tema, window);
            continue;
        }
        for pieza in &elemento.pieces {
            pintado.triangulos += malla(
                &pieza.mesh,
                camara,
                area.origin,
                tema.de_rol(&pieza.role),
                window,
            );
        }
    }
    for etiqueta in nodos.chain(flechas.iter().copied()).flat_map(|e| &e.labels) {
        if texto(etiqueta, camara, area.origin, tema, window, cx) {
            pintado.textos += 1;
        }
    }
    pintado
}

/// Un nodo de lejos: un rectángulo redondeado con el relleno y el borde de su malla.
fn caja(
    elemento: &ElementScene,
    camara: &Camara,
    origen: Point<Pixels>,
    tema: &Tema,
    window: &mut Window,
) {
    let b = elemento.bounds;
    let esquina = camara.a_pantalla(origen, b.x, b.y);
    let limites = Bounds::new(esquina, size(px(b.w * camara.zoom), px(b.h * camara.zoom)));
    let rol = |buscado: fn(&Role) -> bool| {
        elemento
            .pieces
            .iter()
            .find(|p| buscado(&p.role))
            .map(|p| tema.de_rol(&p.role))
    };
    let relleno = rol(|r| !matches!(r, Role::Line(_))).unwrap_or(Hsla::transparent_black());
    let borde = rol(|r| matches!(r, Role::Line(_))).unwrap_or(Hsla::transparent_black());
    window.paint_quad(quad(
        limites,
        px(8.0 * camara.zoom),
        relleno,
        px(1.0),
        borde,
        BorderStyle::default(),
    ));
}

fn malla(
    malla: &Mesh,
    camara: &Camara,
    origen: Point<Pixels>,
    color: Hsla,
    window: &mut Window,
) -> usize {
    let vertice = |i: u32| {
        malla
            .vertices
            .get(i as usize)
            .map(|v| camara.a_pantalla(origen, v.x, v.y))
    };
    let Some(primero) = malla.indices.first().and_then(|&i| vertice(i)) else {
        return 0;
    };
    let mut camino = Path::new(primero);
    let lleno = (point(0.0, 1.0), point(0.0, 1.0), point(0.0, 1.0));
    let mut triangulos = 0;
    for tri in malla.indices.as_chunks::<3>().0 {
        if let (Some(a), Some(b), Some(c)) = (vertice(tri[0]), vertice(tri[1]), vertice(tri[2])) {
            camino.push_triangle((a, b, c), lleno);
            triangulos += 1;
        }
    }
    window.paint_path(camino, color);
    triangulos
}

/// Tamaño de la letra en la pizarra (no en pantalla) y si va en negrita.
fn letra(etiqueta: &Label) -> (f32, bool) {
    match etiqueta.kind {
        LabelKind::Free if etiqueta.text.starts_with("# ") => (32.0, true),
        LabelKind::Free | LabelKind::Body => (17.0, false),
        LabelKind::CardTitle => (17.0, true),
        LabelKind::CardPath => (12.5, false),
        LabelKind::Group => (15.0, true),
        LabelKind::Edge => (13.5, false),
    }
}

/// Por debajo de esto el texto es una mancha: no se compone (nivel de detalle).
const LETRA_MINIMA: f32 = 3.0;

fn texto(
    etiqueta: &Label,
    camara: &Camara,
    origen: Point<Pixels>,
    tema: &Tema,
    window: &mut Window,
    cx: &mut App,
) -> bool {
    let (tamano, negrita) = letra(etiqueta);
    let en_pantalla = tamano * camara.zoom;
    if en_pantalla < LETRA_MINIMA {
        return false;
    }
    // El spike no compone markdown: quita lo que se vería como ruido.
    let limpio: String = etiqueta.text.trim_start_matches("# ").replace("**", "");
    let color = match etiqueta.kind {
        LabelKind::CardPath | LabelKind::Group => tema.tenue,
        _ => etiqueta
            .color
            .as_deref()
            .map_or(tema.tinta, |c| tema.color(Some(c))),
    };
    let fuente = Font {
        weight: if negrita {
            FontWeight::BOLD
        } else {
            FontWeight::NORMAL
        },
        ..font(SharedString::from(tema.fuente.clone()))
    };
    let corrida = TextRun {
        len: limpio.len(),
        font: fuente,
        color,
        background_color: None,
        underline: None,
        strikethrough: None,
    };
    let area = etiqueta.area;
    let ancho = px(area.w * camara.zoom);
    let alto_linea = px(en_pantalla * 1.3);
    // Título y ruta de una tarjeta caben en su alto fijo: una y dos líneas.
    let tope = match etiqueta.kind {
        LabelKind::CardTitle => Some(1),
        LabelKind::CardPath => Some(2),
        _ => None,
    };
    let Ok(lineas) = window.text_system().shape_text(
        limpio.into(),
        px(en_pantalla),
        &[corrida],
        Some(ancho),
        tope,
    ) else {
        return false;
    };
    let alto: Pixels = lineas.iter().map(|l| l.size(alto_linea).height).sum();
    let (alineacion, arriba) = match etiqueta.kind {
        LabelKind::Body | LabelKind::Edge => (
            TextAlign::Center,
            camara.a_pantalla(origen, area.x, area.y + area.h / 2.0).y - alto / 2.0,
        ),
        _ => (TextAlign::Left, camara.a_pantalla(origen, area.x, area.y).y),
    };
    let izquierda = camara.a_pantalla(origen, area.x, area.y).x;
    let caja = Bounds::new(point(izquierda, arriba), size(ancho, alto));
    if etiqueta.kind == LabelKind::Edge {
        let ancho_texto = lineas.iter().map(|l| l.width()).fold(px(0.0), Pixels::max) + px(8.0);
        let fondo = Bounds::new(
            point(izquierda + (ancho - ancho_texto) / 2.0, arriba),
            size(ancho_texto, alto),
        );
        window.paint_quad(fill(fondo, tema.fondo));
    }
    // El texto de una caja no se sale de ella, aunque no tenga por dónde partirse.
    let recorte = match etiqueta.kind {
        LabelKind::Free | LabelKind::Edge | LabelKind::Group => None,
        _ => Some(ContentMask {
            bounds: Bounds::new(
                camara.a_pantalla(origen, area.x, area.y),
                size(ancho, px(area.h * camara.zoom)),
            ),
        }),
    };
    window.with_content_mask(recorte, |window| {
        let mut y = arriba;
        lineas.iter().all(|linea| {
            let pintada = linea.paint(
                point(izquierda, y),
                alto_linea,
                alineacion,
                Some(caja),
                window,
                cx,
            );
            y += linea.size(alto_linea).height;
            pintada.is_ok()
        })
    })
}

/// Puntos cada 40 unidades de la pizarra, como guía. Se apagan al alejarse.
pub fn rejilla(camara: &Camara, area: Bounds<Pixels>, tema: &Tema, window: &mut Window) {
    let paso = 40.0;
    if paso * camara.zoom < 12.0 {
        return;
    }
    let visible = camara.visible(f32::from(area.size.width), f32::from(area.size.height));
    let lado = px((1.5 * camara.zoom).clamp(1.0, 2.5));
    let mut x = (visible.x / paso).floor() * paso;
    while x < visible.x + visible.w {
        let mut y = (visible.y / paso).floor() * paso;
        while y < visible.y + visible.h {
            let p = camara.a_pantalla(area.origin, x, y);
            window.paint_quad(fill(Bounds::new(p, size(lado, lado)), tema.punto));
            y += paso;
        }
        x += paso;
    }
}
