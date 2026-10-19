//! Harness de punta a punta: el binario `metalmind` de verdad, en un Xvfb propio,
//! con `XTest` tecleando. Se comprueba el efecto en disco y cómo termina el
//! proceso; los píxeles, para las capturas (`tests/capturas.rs`) y para ver si
//! algo aparece, como un tooltip. Cómo agregar un caso: `docs/testing.md`.

pub mod keys;
pub mod wait;
mod x11;
mod xvfb;

use std::fs::{self, File};
use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::thread;
use std::time::Duration;

use tempfile::TempDir;

use self::wait::{TIMEOUT, WINDOW_TIMEOUT};
use self::x11::Display;
use self::xvfb::Xvfb;

/// Un Xvfb y una carpeta de notas vacía, solo para este test: los tests
/// corren en paralelo sin pisarse el foco ni los archivos.
pub struct Smoke {
    // `display` se cierra antes que el servidor; el orden de los campos es el de drop.
    display: Display,
    xvfb: Xvfb,
    dir: TempDir,
}

impl Smoke {
    pub fn new() -> Self {
        Self::with_screen(xvfb::SCREEN)
    }

    /// Con una pantalla de `screen` (`1920x1080`), para que quepa una ventana de ese tamaño.
    pub fn with_screen(screen: &str) -> Self {
        let dir = tempfile::tempdir().expect("carpeta temporal");
        for sub in ["home", "notas"] {
            fs::create_dir(dir.path().join(sub)).expect("crear la carpeta");
        }
        let (xvfb, connection) = Xvfb::start(&dir.path().join("xvfb.log"), screen);
        let display = Display::new(connection);
        Self { display, xvfb, dir }
    }

    /// La carpeta desde la que se lanza `metalmind`: los nombres son relativos a ella.
    pub fn notes(&self) -> PathBuf {
        self.dir.path().join("notas")
    }

    pub fn write(&self, name: &str, contents: &str) {
        fs::write(self.notes().join(name), contents).expect("escribir la nota");
    }

    pub fn read(&self, name: &str) -> String {
        fs::read_to_string(self.notes().join(name)).expect("leer la nota")
    }

    pub fn set_mode(&self, name: &str, mode: u32) {
        fs::set_permissions(self.notes().join(name), fs::Permissions::from_mode(mode))
            .expect("cambiar los permisos");
    }

    pub fn mode(&self, name: &str) -> u32 {
        let metadata = fs::metadata(self.notes().join(name)).expect("leer los permisos");
        metadata.permissions().mode() & 0o777
    }

    /// Lo que hay en la carpeta de notas, ordenado: así se ven los temporales.
    pub fn files(&self) -> Vec<String> {
        let mut names: Vec<String> = fs::read_dir(self.notes())
            .expect("listar las notas")
            .map(|entry| {
                let entry = entry.expect("leer la entrada");
                entry.file_name().to_string_lossy().into_owned()
            })
            .collect();
        names.sort();
        names
    }

    /// Espera a que la carpeta de notas tenga justo `expected`, como lo da
    /// [`Smoke::files`]: así se ve un archivo que el árbol creó o renombró.
    pub fn wait_files(&self, expected: &[&str]) {
        wait::until(&format!("las notas {expected:?}"), TIMEOUT, || {
            let files = self.files();
            if files == expected {
                Ok(())
            } else {
                Err(format!("{files:?}"))
            }
        });
    }

    pub fn wait_contents(&self, name: &str, expected: &str) {
        let path = self.notes().join(name);
        wait::until(
            &format!("que {name} diga {expected:?}"),
            TIMEOUT,
            || match fs::read_to_string(&path) {
                Ok(contents) if contents == expected => Ok(()),
                Ok(contents) => Err(format!("{contents:?}")),
                Err(error) => Err(error.to_string()),
            },
        );
    }

    /// Lanza `metalmind args…` con la config por defecto: `HOME` y XDG apuntan a
    /// una carpeta vacía, y sin `WAYLAND_DISPLAY` ni `WAYLAND_SOCKET` GPUI usa
    /// X11: nunca abre en el escritorio de quien corre los tests.
    pub fn launch(&self, args: &[&str]) -> Metalmind<'_> {
        self.launch_with_env(&self.notes(), args, &[])
    }

    /// Como `launch`, pero desde el `HOME` vacío del test y no desde la
    /// carpeta de notas: así se ve qué depende del directorio de trabajo.
    pub fn launch_from_home(&self, args: &[&str]) -> Metalmind<'_> {
        self.launch_with_env(&self.home(), args, &[])
    }

    pub fn home(&self) -> PathBuf {
        self.dir.path().join("home")
    }

    /// Escribe el `config.yaml` con el que se lanza `metalmind`.
    pub fn config(&self, yaml: &str) {
        let dir = self.home().join(".config/metalmind");
        fs::create_dir_all(&dir).expect("crear la carpeta de config");
        fs::write(dir.join("config.yaml"), yaml).expect("escribir la config");
    }

    /// Sin autoguardado: guardar y salir son como en vim.
    pub fn without_autosave(&self) {
        self.config("editor:\n  autosave: false\n");
    }

    /// Cuántas notas tienen historial en la carpeta de datos.
    pub fn history_logs(&self) -> usize {
        let vaults = self.home().join(".local/share/metalmind/vaults");
        let Ok(vaults) = fs::read_dir(vaults) else {
            return 0;
        };
        vaults
            .map(|vault| vault.expect("leer la carpeta del vault").path())
            .filter_map(|vault| fs::read_dir(vault.join("history/notes")).ok())
            .map(Iterator::count)
            .sum()
    }

    /// Como `launch`, pero los loaders de Vulkan y de EGL no encuentran
    /// ningún driver: GPUI no tiene con qué dibujar la ventana.
    pub fn launch_without_gpu(&self, args: &[&str]) -> Metalmind<'_> {
        let missing = "/no-existe.json";
        let env = [
            ("VK_DRIVER_FILES", missing),
            ("__EGL_VENDOR_LIBRARY_FILENAMES", missing),
        ];
        self.launch_with_env(&self.notes(), args, &env)
    }

    fn launch_with_env(&self, cwd: &Path, args: &[&str], env: &[(&str, &str)]) -> Metalmind<'_> {
        let home = self.home();
        let log_path = self.dir.path().join("metalmind.log");
        let log = File::create(&log_path).expect("crear el log");
        let child = Command::new(env!("CARGO_BIN_EXE_metalmind"))
            .args(args)
            .current_dir(cwd)
            .env("DISPLAY", self.xvfb.name())
            .env_remove("WAYLAND_DISPLAY")
            .env_remove("WAYLAND_SOCKET")
            .env_remove("METALMIND_VAULT")
            .env("HOME", &home)
            .env("XDG_CONFIG_HOME", home.join(".config"))
            .env("XDG_DATA_HOME", home.join(".local/share"))
            .env("XDG_CACHE_HOME", home.join(".cache"))
            .env("XDG_STATE_HOME", home.join(".local/state"))
            .env("RUST_LOG", "info")
            .envs(env.iter().copied())
            .stdin(Stdio::null())
            .stdout(log.try_clone().expect("duplicar el log"))
            .stderr(log)
            .spawn()
            .expect("lanzar metalmind");
        Metalmind {
            child,
            display: &self.display,
            log: log_path,
        }
    }
}

/// Un proceso `metalmind` corriendo. Si el test falla, su log sale en la salida.
pub struct Metalmind<'a> {
    child: Child,
    display: &'a Display,
    log: PathBuf,
}

impl<'a> Metalmind<'a> {
    /// Las ventanas que tiene abiertas ahora, sin esperar.
    pub fn windows(&self) -> Vec<Window<'a>> {
        self.display
            .windows_of(self.child.id())
            .into_iter()
            .map(|id| Window {
                id,
                display: self.display,
            })
            .collect()
    }

    /// Espera la primera ventana, o falla si el proceso termina antes.
    pub fn window(&mut self) -> Window<'a> {
        wait::until("la ventana de Metalmind", WINDOW_TIMEOUT, || {
            if let Some(status) = self.try_exit() {
                panic!("metalmind terminó con {status} sin abrir la ventana");
            }
            self.windows()
                .into_iter()
                .next()
                .ok_or_else(|| "ninguna ventana".to_owned())
        })
    }

    /// Lo que lleva escrito en stdout y stderr.
    pub fn log(&self) -> String {
        fs::read_to_string(&self.log).expect("leer el log de metalmind")
    }

    pub fn is_running(&mut self) -> bool {
        self.try_exit().is_none()
    }

    pub fn wait_exit(&mut self) -> ExitStatus {
        wait::until("que metalmind termine", TIMEOUT, || {
            self.try_exit().ok_or_else(|| "sigue corriendo".to_owned())
        })
    }

    fn try_exit(&mut self) -> Option<ExitStatus> {
        self.child.try_wait().expect("consultar el proceso")
    }
}

impl Drop for Metalmind<'_> {
    fn drop(&mut self) {
        if self.is_running() {
            // Un proceso colgado haría que el test nunca terminara.
            let _ = self.child.kill();
            let _ = self.child.wait();
        }
        if std::thread::panicking() {
            let log = fs::read_to_string(&self.log).unwrap_or_default();
            eprintln!("--- log de metalmind ---\n{log}--- fin del log ---");
        }
    }
}

/// Un rectángulo de la ventana, en píxeles desde su esquina.
#[derive(Clone, Copy, Debug)]
pub struct Area {
    pub x: i16,
    pub y: i16,
    pub width: u16,
    pub height: u16,
}

/// Lo que tiene que quedar igual la ventana para darla por quieta: más que
/// un frame aun con lavapipe.
const STILL: Duration = Duration::from_millis(200);

#[derive(Clone, Copy)]
pub struct Window<'a> {
    id: x11rb::protocol::xproto::Window,
    display: &'a Display,
}

impl Window<'_> {
    pub fn title(&self) -> String {
        self.display.title(self.id)
    }

    /// Espera a que el título cumpla `matches`: así se sabe que la app ya
    /// procesó unas teclas que no dejan rastro en disco, como entrar en Insert.
    pub fn wait_title(&self, what: &str, matches: impl Fn(&str) -> bool) {
        wait::until(&format!("un título {what}"), TIMEOUT, || {
            let title = self.title();
            if matches(&title) { Ok(()) } else { Err(title) }
        });
    }

    /// Teclea `notation` en esta ventana, en la notación de vim.
    pub fn type_keys(&self, notation: &str) {
        self.display.type_keys(self.id, &keys::parse(notation));
    }

    /// Un clic en `(x, y)`, en píxeles desde la esquina de la ventana.
    pub fn click(&self, x: i16, y: i16) {
        self.display.click(self.id, 1, x, y);
    }

    /// Un clic con el botón derecho en `(x, y)`.
    pub fn right_click(&self, x: i16, y: i16) {
        self.display.click(self.id, 3, x, y);
    }

    /// Deja el puntero en `(x, y)`, por ejemplo donde no resalte nada.
    pub fn move_pointer(&self, x: i16, y: i16) {
        self.display.move_pointer(self.id, x, y);
    }

    /// Le pide a la ventana este tamaño, como un gestor de ventanas.
    pub fn resize(&self, width: u32, height: u32) {
        self.display.resize(self.id, width, height);
    }

    /// Lo que se ve en la ventana ahora: ancho, alto y píxeles en RGB.
    pub fn pixels(&self) -> (u16, u16, Vec<u8>) {
        self.display.pixels(self.id)
    }

    /// Una muesca de la rueda hacia abajo (el botón 5 de X) en `(x, y)`.
    pub fn scroll_down(&self, x: i16, y: i16) {
        self.display.click(self.id, 5, x, y);
    }

    /// Los píxeles de un rectángulo de la ventana, en RGB: para ver si algo
    /// apareció (un tooltip), no para comparar con una imagen guardada.
    pub fn area_pixels(&self, area: Area) -> Vec<u8> {
        let (width, _, rgb) = self.pixels();
        let row = usize::from(width) * 3;
        let (x, y) = (
            usize::try_from(area.x).unwrap(),
            usize::try_from(area.y).unwrap(),
        );
        rgb.chunks(row)
            .skip(y)
            .take(usize::from(area.height))
            .flat_map(|line| &line[x * 3..(x + usize::from(area.width)) * 3])
            .copied()
            .collect()
    }

    /// Espera a que `area` deje de verse como `before`: que la app ya pintó
    /// lo que se esperaba (`what`).
    pub fn wait_change(&self, what: &str, area: Area, before: &[u8]) {
        wait::until(what, TIMEOUT, || {
            if self.area_pixels(area) == before {
                Err("la ventana sigue igual".to_owned())
            } else {
                Ok(())
            }
        });
    }

    /// Espera a que `area` deje de cambiar, como tras un scroll.
    pub fn wait_still(&self, area: Area) -> Vec<u8> {
        let mut last = self.area_pixels(area);
        wait::until("que la ventana deje de cambiar", TIMEOUT, || {
            thread::sleep(STILL);
            let now = self.area_pixels(area);
            if now == last {
                Ok(now)
            } else {
                last = now;
                Err("cambió".to_owned())
            }
        })
    }

    /// Pide el cierre como lo haría el gestor de ventanas.
    pub fn request_close(&self) {
        self.display.request_close(self.id);
    }
}
