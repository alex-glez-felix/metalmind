//! Lo que el harness le hace al servidor X: buscar las ventanas de un proceso,
//! leer su título, teclear y hacer clic con `XTest`, pedir el cierre y el
//! tamaño como un gestor de ventanas y leer sus píxeles.

use x11rb::connection::Connection as _;
use x11rb::protocol::xproto::{
    self, AtomEnum, ClientMessageEvent, ConfigureWindowAux, ConnectionExt as _, EventMask,
    ImageFormat, ImageOrder, InputFocus, MapState,
};
use x11rb::protocol::xtest::ConnectionExt as _;
use x11rb::rust_connection::RustConnection;
use x11rb::wrapper::ConnectionExt as _;

use super::keys::Stroke;
use super::xvfb::Connection;

x11rb::atom_manager! {
    Atoms: AtomsCookie {
        WM_PROTOCOLS,
        WM_DELETE_WINDOW,
        _NET_WM_PID,
        _NET_WM_NAME,
        UTF8_STRING,
    }
}

const SHIFT_L: u32 = 0xffe1;
const CONTROL_L: u32 = 0xffe3;

pub struct Display {
    conn: RustConnection,
    root: xproto::Window,
    atoms: Atoms,
    keymap: Keymap,
}

impl Display {
    pub fn new(Connection { conn, screen }: Connection) -> Self {
        let root = conn.setup().roots[screen].root;
        let atoms = Atoms::new(&conn)
            .expect("pedir los átomos")
            .reply()
            .expect("leer los átomos");
        let keymap = Keymap::load(&conn);
        Self {
            conn,
            root,
            atoms,
            keymap,
        }
    }

    /// Las ventanas visibles de `pid`, sin asumir cuántas hay. Sin gestor de
    /// ventanas, las de primer nivel son hijas directas de la raíz.
    pub fn windows_of(&self, pid: u32) -> Vec<xproto::Window> {
        let tree = self
            .conn
            .query_tree(self.root)
            .expect("pedir el árbol")
            .reply()
            .expect("leer el árbol");
        tree.children
            .into_iter()
            .filter(|&window| self.pid_of(window) == Some(pid) && self.is_viewable(window))
            .collect()
    }

    pub fn title(&self, window: xproto::Window) -> String {
        let reply = self
            .conn
            .get_property(
                false,
                window,
                self.atoms._NET_WM_NAME,
                self.atoms.UTF8_STRING,
                0,
                u32::MAX,
            )
            .expect("pedir el título")
            .reply()
            .expect("leer el título");
        String::from_utf8_lossy(&reply.value).into_owned()
    }

    /// Enfoca `window` y teclea `strokes` con `XTest`. Vuelve cuando el servidor
    /// ya las procesó; que la app las haya leído se comprueba por sus efectos.
    pub fn type_keys(&self, window: xproto::Window, strokes: &[Stroke]) {
        self.conn
            .set_input_focus(InputFocus::PARENT, window, x11rb::CURRENT_TIME)
            .expect("enfocar la ventana");
        for stroke in strokes {
            let (keycode, shift) = self.keymap.find(stroke.keysym).unwrap_or_else(|| {
                panic!(
                    "el mapa de teclado del Xvfb no tiene el keysym {:#x}",
                    stroke.keysym
                )
            });
            let modifiers = [(stroke.ctrl, CONTROL_L), (shift, SHIFT_L)];
            let held: Vec<u8> = modifiers
                .into_iter()
                .filter(|&(active, _)| active)
                .map(|(_, keysym)| self.keymap.find(keysym).expect("ctrl y shift en el mapa").0)
                .collect();
            for &modifier in &held {
                self.fake_key(xproto::KEY_PRESS_EVENT, modifier);
            }
            self.fake_key(xproto::KEY_PRESS_EVENT, keycode);
            self.fake_key(xproto::KEY_RELEASE_EVENT, keycode);
            for &modifier in held.iter().rev() {
                self.fake_key(xproto::KEY_RELEASE_EVENT, modifier);
            }
        }
        self.conn.sync().expect("esperar al servidor");
    }

    /// Un clic con `button` (1 el izquierdo, 3 el derecho) en `(x, y)` de
    /// `window`, con `XTest`. Vuelve cuando el servidor lo procesó.
    pub fn click(&self, window: xproto::Window, button: u8, x: i16, y: i16) {
        self.move_pointer(window, x, y);
        for kind in [xproto::BUTTON_PRESS_EVENT, xproto::BUTTON_RELEASE_EVENT] {
            self.conn
                .xtest_fake_input(kind, button, x11rb::CURRENT_TIME, self.root, 0, 0, 0)
                .expect("mandar el clic");
        }
        self.conn.sync().expect("esperar al servidor");
    }

    /// Lleva el puntero a `(x, y)` de `window`, con `XTest`.
    pub fn move_pointer(&self, window: xproto::Window, x: i16, y: i16) {
        let at = self
            .conn
            .translate_coordinates(window, self.root, x, y)
            .expect("pedir la posición")
            .reply()
            .expect("leer la posición");
        self.conn
            .xtest_fake_input(
                xproto::MOTION_NOTIFY_EVENT,
                0,
                x11rb::CURRENT_TIME,
                self.root,
                at.dst_x,
                at.dst_y,
                0,
            )
            .expect("mover el puntero");
        self.conn.sync().expect("esperar al servidor");
    }

    /// Le da a `window` ese tamaño, como un gestor de ventanas: sin gestor,
    /// el servidor lo aplica tal cual.
    pub fn resize(&self, window: xproto::Window, width: u32, height: u32) {
        let size = ConfigureWindowAux::new().width(width).height(height);
        self.conn
            .configure_window(window, &size)
            .expect("cambiar el tamaño");
        self.conn.sync().expect("esperar al servidor");
    }

    /// El tamaño de `window` ahora.
    pub fn size(&self, window: xproto::Window) -> (u16, u16) {
        let geometry = self
            .conn
            .get_geometry(window)
            .expect("pedir el tamaño")
            .reply()
            .expect("leer el tamaño");
        (geometry.width, geometry.height)
    }

    /// Los píxeles de `window` en RGB, fila por fila.
    pub fn pixels(&self, window: xproto::Window) -> (u16, u16, Vec<u8>) {
        self.try_pixels(window).expect("leer la ventana")
    }

    /// Como `pixels`, o `None` si la ventana ya no existe (la app salió).
    pub fn try_pixels(&self, window: xproto::Window) -> Option<(u16, u16, Vec<u8>)> {
        let geometry = self
            .conn
            .get_geometry(window)
            .expect("pedir el tamaño")
            .reply()
            .ok()?;
        let (width, height) = (geometry.width, geometry.height);
        let image = self
            .conn
            .get_image(ImageFormat::Z_PIXMAP, window, 0, 0, width, height, !0)
            .expect("pedir la imagen")
            .reply()
            .ok()?;
        self.assert_bgrx(image.depth, image.visual);
        let rgb = image
            .data
            .as_chunks::<4>()
            .0
            .iter()
            .flat_map(|&[blue, green, red, _]| [red, green, blue])
            .collect();
        Some((width, height, rgb))
    }

    /// Que cada píxel de una imagen de `depth` bits con `visual` sean 4 bytes:
    /// azul, verde, rojo y el alfa (o nada), que sin compositor no se ve. Es
    /// lo que da el Xvfb del harness; otro formato leería colores falsos.
    fn assert_bgrx(&self, depth: u8, visual: xproto::Visualid) {
        let setup = self.conn.setup();
        assert_eq!(
            setup.image_byte_order,
            ImageOrder::LSB_FIRST,
            "orden de bytes"
        );
        let bits = setup
            .pixmap_formats
            .iter()
            .find(|format| format.depth == depth)
            .map(|format| format.bits_per_pixel);
        assert_eq!(bits, Some(32), "bits por píxel con profundidad {depth}");
        let masks = setup
            .roots
            .iter()
            .flat_map(|screen| &screen.allowed_depths)
            .flat_map(|allowed| &allowed.visuals)
            .find(|candidate| candidate.visual_id == visual)
            .map(|found| (found.red_mask, found.green_mask, found.blue_mask));
        assert_eq!(
            masks,
            Some((0xff_00_00, 0xff_00, 0xff)),
            "máscaras del visual"
        );
    }

    /// Lo que manda un gestor de ventanas al pulsar la "x": `WM_DELETE_WINDOW`.
    pub fn request_close(&self, window: xproto::Window) {
        let message = ClientMessageEvent::new(
            32,
            window,
            self.atoms.WM_PROTOCOLS,
            [self.atoms.WM_DELETE_WINDOW, x11rb::CURRENT_TIME, 0, 0, 0],
        );
        self.conn
            .send_event(false, window, EventMask::NO_EVENT, message)
            .expect("pedir el cierre");
        self.conn.sync().expect("esperar al servidor");
    }

    fn fake_key(&self, kind: u8, keycode: u8) {
        self.conn
            .xtest_fake_input(kind, keycode, x11rb::CURRENT_TIME, self.root, 0, 0, 0)
            .expect("mandar la tecla");
    }

    fn pid_of(&self, window: xproto::Window) -> Option<u32> {
        let reply = self
            .conn
            .get_property(
                false,
                window,
                self.atoms._NET_WM_PID,
                AtomEnum::CARDINAL,
                0,
                1,
            )
            .ok()?
            .reply()
            .ok()?;
        reply.value32()?.next()
    }

    fn is_viewable(&self, window: xproto::Window) -> bool {
        self.conn
            .get_window_attributes(window)
            .ok()
            .and_then(|cookie| cookie.reply().ok())
            .is_some_and(|attributes| attributes.map_state == MapState::VIEWABLE)
    }
}

/// El mapa de teclado del núcleo de X: por keycode, el keysym sin y con shift.
struct Keymap {
    min_keycode: u8,
    per_keycode: usize,
    keysyms: Vec<u32>,
}

impl Keymap {
    fn load(conn: &RustConnection) -> Self {
        let setup = conn.setup();
        let (min_keycode, max_keycode) = (setup.min_keycode, setup.max_keycode);
        let reply = conn
            .get_keyboard_mapping(min_keycode, max_keycode - min_keycode + 1)
            .expect("pedir el mapa de teclado")
            .reply()
            .expect("leer el mapa de teclado");
        Self {
            min_keycode,
            per_keycode: usize::from(reply.keysyms_per_keycode),
            keysyms: reply.keysyms,
        }
    }

    /// El keycode de `keysym` y si hace falta shift; sin shift si se puede.
    fn find(&self, keysym: u32) -> Option<(u8, bool)> {
        [false, true].into_iter().find_map(|shift| {
            let column = usize::from(shift);
            let index = self
                .keysyms
                .chunks(self.per_keycode)
                .position(|row| row.get(column) == Some(&keysym))?;
            let keycode = self.min_keycode + u8::try_from(index).ok()?;
            Some((keycode, shift))
        })
    }
}
