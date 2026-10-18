//! Lo que el harness le hace al servidor X: buscar las ventanas de un proceso,
//! leer su título, teclear con `XTest` y pedir el cierre como un gestor de ventanas.

use x11rb::connection::Connection as _;
use x11rb::protocol::xproto::{
    self, AtomEnum, ClientMessageEvent, ConnectionExt as _, EventMask, InputFocus, MapState,
};
use x11rb::protocol::xtest::ConnectionExt as _;
use x11rb::rust_connection::RustConnection;
use x11rb::wrapper::ConnectionExt as _;

use super::keys::Stroke;

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
    pub fn connect(name: &str) -> Self {
        let (conn, screen) = x11rb::connect(Some(name)).expect("conectar al Xvfb");
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
