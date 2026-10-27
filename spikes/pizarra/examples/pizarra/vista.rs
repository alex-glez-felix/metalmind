//! La vista de la pizarra: la escena cacheada, la cámara, el ratón y el
//! teclado. Editar es lo mínimo para medir: arrastrar una caja (sus flechas
//! se re-enrutan), crear una caja o una elipse con `r` u `o` y un clic.

use std::cell::RefCell;
use std::rc::Rc;
use std::time::Instant;

use gpui_kit::{
    Bounds, Context, FocusHandle, InteractiveElement, IntoElement, KeyDownEvent, MouseButton,
    MouseDownEvent, MouseMoveEvent, MouseUpEvent, ParentElement, Pixels, Point, Render,
    ScrollWheelEvent, Styled, Window, canvas, div, px,
};
use metalmind_spike_pizarra::escena::{self, ElementScene, Style};
use metalmind_spike_pizarra::formato::{Board, Node, NodeKind, Shape};
use metalmind_spike_pizarra::geometria::{self, P, Rect};

use crate::banco::{self, Fase, Medidas};
use crate::pintar::{self, Camara, Pintado};
use crate::tema::Tema;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Herramienta {
    Seleccion,
    Caja,
    Elipse,
}

/// Las herramientas de la barra, con su tecla. El spike solo implementa las
/// tres primeras; las demás están para ver la barra completa (09 §5).
const BARRA: [(&str, &str); 8] = [
    ("v", "Selección"),
    ("r", "Caja"),
    ("o", "Elipse"),
    ("d", "Rombo"),
    ("a", "Flecha"),
    ("t", "Texto"),
    ("p", "Lápiz"),
    ("n", "Nota"),
];

/// Cómo se abre la vista: lo que viene de la línea de comandos.
pub struct Ajustes {
    pub estilo: Style,
    pub tema: Tema,
    pub titulo: String,
    pub banco: bool,
    pub camara: Option<Camara>,
    pub detalle: bool,
}

enum Arrastre {
    Pan {
        desde: Point<Pixels>,
        camara: Camara,
    },
    Caja {
        nodo: usize,
        desde: Point<Pixels>,
        x: i32,
        y: i32,
    },
}

pub struct Pizarra {
    board: Board,
    estilo: Style,
    nodos: Rc<Vec<ElementScene>>,
    flechas: Rc<Vec<ElementScene>>,
    camara: Option<Camara>,
    area: Bounds<Pixels>,
    tema: Tema,
    herramienta: Herramienta,
    arrastre: Option<Arrastre>,
    foco: FocusHandle,
    medidas: Rc<RefCell<Medidas>>,
    pintado: Rc<RefCell<Pintado>>,
    banco: Option<usize>,
    titulo: String,
    camara_inicial: Option<Camara>,
    /// Nivel de detalle: de lejos, cada nodo es un quad (`pintar::ZOOM_DETALLE`).
    detalle: bool,
    /// La barra de estado muestra el frame anterior: hace falta uno más al abrir.
    estado_al_dia: bool,
}

impl Pizarra {
    /// `nodos` y `flechas` son la escena de `board`, elemento por elemento
    /// (`escena::build`): mover un nodo reemplaza el suyo por índice.
    pub fn new(
        board: Board,
        (nodos, flechas): (Vec<ElementScene>, Vec<ElementScene>),
        ajustes: Ajustes,
        cx: &mut Context<Self>,
    ) -> Self {
        let Ajustes {
            estilo,
            tema,
            titulo,
            banco,
            camara,
            detalle,
        } = ajustes;
        Self {
            board,
            estilo,
            nodos: Rc::new(nodos),
            flechas: Rc::new(flechas),
            camara: None,
            area: Bounds::default(),
            tema,
            herramienta: Herramienta::Seleccion,
            arrastre: None,
            foco: cx.focus_handle(),
            medidas: Rc::default(),
            pintado: Rc::default(),
            banco: banco.then_some(0),
            titulo,
            camara_inicial: camara,
            detalle,
            estado_al_dia: false,
        }
    }

    pub fn foco(&self) -> &FocusHandle {
        &self.foco
    }

    fn caja_de_todo(&self) -> Rect {
        let todo = self
            .nodos
            .iter()
            .chain(self.flechas.iter())
            .map(|e| e.bounds);
        todo.reduce(|a, b| {
            let (x, y) = (a.x.min(b.x), a.y.min(b.y));
            Rect {
                x,
                y,
                w: (a.x + a.w).max(b.x + b.w) - x,
                h: (a.y + a.h).max(b.y + b.h) - y,
            }
        })
        .unwrap_or(Rect {
            x: 0.0,
            y: 0.0,
            w: 800.0,
            h: 600.0,
        })
    }

    fn ver_todo(&self) -> Camara {
        Camara::para_ver(
            self.caja_de_todo(),
            f32::from(self.area.size.width),
            f32::from(self.area.size.height),
        )
    }

    /// Mover un nodo vuelve a construir solo su escena y la de sus flechas.
    fn mover_nodo(&mut self, indice: usize, x: i32, y: i32) {
        let Some(nodo) = self.board.nodes.get_mut(indice) else {
            return;
        };
        (nodo.x, nodo.y) = (x, y);
        let id = nodo.id.clone();
        if let Ok(escena) = escena::node_scene(nodo, self.estilo) {
            Rc::make_mut(&mut self.nodos)[indice] = escena;
        }
        for (i, flecha) in self.board.edges.iter().enumerate() {
            if flecha.from.node != id && flecha.to.node != id {
                continue;
            }
            let Some(ruta) = geometria::route(&self.board, flecha) else {
                continue;
            };
            if let Ok(escena) = escena::edge_scene(flecha, &ruta, self.estilo) {
                Rc::make_mut(&mut self.flechas)[i] = escena;
            }
        }
    }

    fn crear(&mut self, forma: Shape, en: (f32, f32)) {
        let (x, y) = (ajustar(en.0 - 80.0), ajustar(en.1 - 40.0));
        let nodo = Node {
            id: format!("nuevo-{}", self.board.nodes.len()),
            x,
            y,
            width: 160,
            height: 80,
            color: None,
            kind: NodeKind::Text {
                text: "Nueva".to_owned(),
                shape: forma,
            },
            extra: serde_json::Map::new(),
        };
        if let Ok(escena) = escena::node_scene(&nodo, self.estilo) {
            self.board.nodes.push(nodo);
            Rc::make_mut(&mut self.nodos).push(escena);
        }
    }

    fn al_bajar(&mut self, evento: &MouseDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        window.focus(&self.foco, cx);
        let Some(camara) = self.camara else {
            return;
        };
        let (x, y) = camara.a_pizarra(self.area.origin, evento.position);
        match self.herramienta {
            Herramienta::Caja | Herramienta::Elipse => {
                let forma = if self.herramienta == Herramienta::Caja {
                    Shape::Rectangle
                } else {
                    Shape::Ellipse
                };
                self.crear(forma, (x, y));
                self.herramienta = Herramienta::Seleccion;
            }
            Herramienta::Seleccion => {
                self.arrastre = Some(match geometria::hit(&self.board, P::new(x, y)) {
                    Some(nodo) => Arrastre::Caja {
                        nodo,
                        desde: evento.position,
                        x: self.board.nodes[nodo].x,
                        y: self.board.nodes[nodo].y,
                    },
                    None => Arrastre::Pan {
                        desde: evento.position,
                        camara,
                    },
                });
            }
        }
        cx.notify();
    }

    fn al_mover(&mut self, evento: &MouseMoveEvent, _window: &mut Window, cx: &mut Context<Self>) {
        if !evento.dragging() {
            return;
        }
        let Some(camara) = self.camara else {
            return;
        };
        match self.arrastre {
            Some(Arrastre::Pan {
                desde,
                camara: inicial,
            }) => {
                let delta = evento.position - desde;
                self.camara = Some(Camara {
                    x: inicial.x - f32::from(delta.x) / camara.zoom,
                    y: inicial.y - f32::from(delta.y) / camara.zoom,
                    ..camara
                });
            }
            Some(Arrastre::Caja { nodo, desde, x, y }) => {
                let delta = evento.position - desde;
                #[expect(clippy::cast_precision_loss, reason = "coordenadas de una pizarra")]
                let (nx, ny) = (
                    ajustar(x as f32 + f32::from(delta.x) / camara.zoom),
                    ajustar(y as f32 + f32::from(delta.y) / camara.zoom),
                );
                self.mover_nodo(nodo, nx, ny);
            }
            None => return,
        }
        cx.notify();
    }

    fn al_soltar(&mut self, _evento: &MouseUpEvent, _window: &mut Window, _cx: &mut Context<Self>) {
        self.arrastre = None;
    }

    fn al_rodar(&mut self, evento: &ScrollWheelEvent, window: &mut Window, cx: &mut Context<Self>) {
        let Some(camara) = self.camara.as_mut() else {
            return;
        };
        let delta = evento.delta.pixel_delta(window.line_height());
        if evento.modifiers.control {
            camara.acercar(
                (f32::from(delta.y) / 200.0).exp(),
                self.area.origin,
                evento.position,
            );
        } else {
            camara.x -= f32::from(delta.x) / camara.zoom;
            camara.y -= f32::from(delta.y) / camara.zoom;
        }
        cx.notify();
    }

    fn al_teclear(&mut self, evento: &KeyDownEvent, _window: &mut Window, cx: &mut Context<Self>) {
        let centro = self.area.center();
        let todo = self.ver_todo();
        let Some(camara) = self.camara.as_mut() else {
            return;
        };
        let paso = 80.0 / camara.zoom;
        match evento.keystroke.key.as_str() {
            "v" => self.herramienta = Herramienta::Seleccion,
            "r" => self.herramienta = Herramienta::Caja,
            "o" => self.herramienta = Herramienta::Elipse,
            "h" | "left" => camara.x -= paso,
            "l" | "right" => camara.x += paso,
            "k" | "up" => camara.y -= paso,
            "j" | "down" => camara.y += paso,
            "+" | "=" => camara.acercar(1.25, self.area.origin, centro),
            "-" => camara.acercar(0.8, self.area.origin, centro),
            "0" => *camara = todo,
            _ => return,
        }
        cx.notify();
    }

    /// Un paso del guion del benchmark; `None` si no hay benchmark o ya acabó.
    fn paso_del_banco(&mut self, window: &mut Window, cx: &mut Context<Self>) -> Option<Fase> {
        let frame = self.banco?;
        self.banco = Some(frame + 1);
        let Some((fase, t)) = banco::paso(frame) else {
            if frame >= banco::CALENTAR {
                let reporte = self.medidas.borrow().reporte(&self.titulo);
                println!("{reporte}");
                self.banco = None;
                window.on_next_frame(|_, cx| cx.quit());
                return None;
            }
            window.request_animation_frame();
            return None;
        };
        let todo = self.ver_todo();
        let onda = (t * std::f32::consts::TAU).sin();
        let camara = match fase {
            Fase::TodoALaVista => Camara {
                x: todo.x + onda * 30.0 / todo.zoom,
                ..todo
            },
            Fase::Zoom => {
                let mut c = todo;
                c.acercar(
                    (3.0 / todo.zoom).max(1.0).powf(1.0 - (2.0 * t - 1.0).abs()),
                    self.area.origin,
                    self.area.center(),
                );
                c
            }
            Fase::PanAlCien => {
                let caja = self.caja_de_todo();
                Camara {
                    x: caja.x + t * (caja.w - f32::from(self.area.size.width)).max(0.0),
                    y: caja.y,
                    zoom: 1.0,
                }
            }
            Fase::ArrastrarCaja => {
                let (x, y) = (
                    self.board.nodes.first().map_or(0, |n| n.x),
                    self.board.nodes.first().map_or(0, |n| n.y),
                );
                #[expect(clippy::cast_possible_truncation, reason = "un desplazamiento chico")]
                let dx = (onda * 6.0).round() as i32;
                self.mover_nodo(0, x + dx, y);
                #[expect(clippy::cast_precision_loss, reason = "coordenadas de una pizarra")]
                Camara {
                    x: x as f32 - 200.0,
                    y: y as f32 - 200.0,
                    zoom: 1.0,
                }
            }
        };
        self.camara = Some(camara);
        window.request_animation_frame();
        cx.notify();
        Some(fase)
    }
}

/// Las cajas se alinean a una rejilla de 10 al moverlas o crearlas.
#[expect(
    clippy::cast_possible_truncation,
    reason = "coordenadas de una pizarra"
)]
fn ajustar(valor: f32) -> i32 {
    ((valor / 10.0).round() * 10.0) as i32
}

impl Render for Pizarra {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let empezo = Instant::now();
        if self.camara.is_none() && self.area.size.width > px(0.0) {
            self.camara = Some(self.camara_inicial.unwrap_or_else(|| self.ver_todo()));
        }
        let fase = self.paso_del_banco(window, cx);
        self.medidas.borrow_mut().empieza(empezo, fase);
        // Un frame más al abrir, para que la barra de estado muestre el primero.
        if self.camara.is_none() {
            window.request_animation_frame();
        } else if !self.estado_al_dia {
            self.estado_al_dia = true;
            window.request_animation_frame();
        }
        let camara = self.camara;
        let detalle = self.detalle;
        let tema = self.tema.clone();
        // La escena se presta al `paint` por un `Rc`: no se copian mallas.
        let nodos = Rc::clone(&self.nodos);
        let flechas = Rc::clone(&self.flechas);
        let medidas = Rc::clone(&self.medidas);
        let pintado = Rc::clone(&self.pintado);
        let vista = cx.entity().downgrade();
        let lienzo = canvas(
            move |area, _, cx| {
                vista
                    .update(cx, |vista, _| {
                        if vista.area != area {
                            vista.area = area;
                        }
                    })
                    .ok();
                area
            },
            move |_, area, window, cx| {
                let Some(camara) = camara else {
                    return;
                };
                window.paint_quad(gpui_kit::fill(area, tema.fondo));
                pintar::rejilla(&camara, area, &tema, window);
                let marco = pintar::Marco {
                    camara: &camara,
                    area,
                    tema: &tema,
                    detalle,
                };
                let hecho = pintar::escena(&nodos, &flechas, &marco, window, cx);
                *pintado.borrow_mut() = hecho;
                medidas.borrow_mut().pintado(empezo, hecho);
            },
        )
        .size_full();
        div()
            .size_full()
            .relative()
            .bg(self.tema.fondo)
            .font_family(self.tema.fuente.clone())
            .track_focus(&self.foco)
            .on_key_down(cx.listener(Self::al_teclear))
            .on_mouse_down(MouseButton::Left, cx.listener(Self::al_bajar))
            .on_mouse_move(cx.listener(Self::al_mover))
            .on_mouse_up(MouseButton::Left, cx.listener(Self::al_soltar))
            .on_scroll_wheel(cx.listener(Self::al_rodar))
            .child(lienzo)
            .child(self.barra())
            .child(self.estado())
    }
}

impl Pizarra {
    fn barra(&self) -> impl IntoElement {
        let activa = match self.herramienta {
            Herramienta::Seleccion => "v",
            Herramienta::Caja => "r",
            Herramienta::Elipse => "o",
        };
        let tema = &self.tema;
        div()
            .absolute()
            .top(px(12.0))
            .left(px(12.0))
            .flex()
            .gap(px(2.0))
            .p(px(3.0))
            .rounded(px(6.0))
            .bg(tema.barra)
            .border_1()
            .border_color(tema.borde)
            .text_size(px(13.0))
            .children(BARRA.iter().map(|(tecla, nombre)| {
                let es = *tecla == activa;
                div()
                    .flex()
                    .items_center()
                    .gap(px(6.0))
                    .px(px(8.0))
                    .py(px(4.0))
                    .rounded(px(4.0))
                    .bg(if es { tema.borde } else { tema.barra })
                    .text_color(if es { tema.tinta } else { tema.tenue })
                    .child(*nombre)
                    .child(
                        div()
                            .text_color(tema.tenue)
                            .text_size(px(11.0))
                            .child(*tecla),
                    )
            }))
    }

    fn estado(&self) -> impl IntoElement {
        let tema = &self.tema;
        let zoom = self.camara.map_or(100.0, |c| c.zoom * 100.0);
        let pintado = *self.pintado.borrow();
        let medidas = self.medidas.borrow();
        let total = self.board.nodes.len() + self.board.edges.len();
        let texto = format!(
            "{zoom:.0} %   ·   {total} elementos, {} a la vista   ·   frame {:.1} ms (CPU {:.1} ms)",
            pintado.elementos,
            medidas.ultimo_intervalo.as_secs_f64() * 1000.0,
            medidas.ultimo_cpu.as_secs_f64() * 1000.0,
        );
        let ayuda = "rueda  mover   ·   ctrl+rueda  zoom   ·   arrastrar  mover caja o pizarra   ·   0  ver todo   ·   h j k l  mover";
        div()
            .absolute()
            .bottom(px(0.0))
            .left(px(0.0))
            .right(px(0.0))
            .flex()
            .justify_between()
            .px(px(12.0))
            .py(px(5.0))
            .bg(tema.barra)
            .border_t_1()
            .border_color(tema.borde)
            .text_size(px(12.0))
            .text_color(tema.tenue)
            .child(div().overflow_hidden().whitespace_nowrap().child(texto))
            .child(div().overflow_hidden().whitespace_nowrap().child(ayuda))
    }
}
