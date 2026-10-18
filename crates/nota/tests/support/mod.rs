//! Harness de punta a punta: el binario `nota` de verdad, en un Xvfb propio,
//! con `XTest` tecleando. Se comprueba el efecto en disco y cómo termina el
//! proceso, no los píxeles. Cómo agregar un caso: `docs/testing.md`.

mod keys;
mod wait;
mod x11;
mod xvfb;

use std::fs::{self, File};
use std::os::unix::fs::PermissionsExt as _;
use std::path::PathBuf;
use std::process::{Child, Command, ExitStatus, Stdio};

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
        let dir = tempfile::tempdir().expect("carpeta temporal");
        for sub in ["home", "notas"] {
            fs::create_dir(dir.path().join(sub)).expect("crear la carpeta");
        }
        let (xvfb, connection) = Xvfb::start(&dir.path().join("xvfb.log"));
        let display = Display::new(connection);
        Self { display, xvfb, dir }
    }

    /// La carpeta desde la que se lanza `nota`: los nombres son relativos a ella.
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

    /// Lanza `nota args…` con la config por defecto: `HOME` y XDG apuntan a
    /// una carpeta vacía, y sin `WAYLAND_DISPLAY` ni `WAYLAND_SOCKET` GPUI usa
    /// X11: nunca abre en el escritorio de quien corre los tests.
    pub fn launch(&self, args: &[&str]) -> Nota<'_> {
        self.launch_with_env(args, &[])
    }

    /// Como `launch`, pero los loaders de Vulkan y de EGL no encuentran
    /// ningún driver: GPUI no tiene con qué dibujar la ventana.
    pub fn launch_without_gpu(&self, args: &[&str]) -> Nota<'_> {
        let missing = "/no-existe.json";
        let env = [
            ("VK_DRIVER_FILES", missing),
            ("__EGL_VENDOR_LIBRARY_FILENAMES", missing),
        ];
        self.launch_with_env(args, &env)
    }

    fn launch_with_env(&self, args: &[&str], env: &[(&str, &str)]) -> Nota<'_> {
        let home = self.dir.path().join("home");
        let log_path = self.dir.path().join("nota.log");
        let log = File::create(&log_path).expect("crear el log");
        let child = Command::new(env!("CARGO_BIN_EXE_nota"))
            .args(args)
            .current_dir(self.notes())
            .env("DISPLAY", self.xvfb.name())
            .env_remove("WAYLAND_DISPLAY")
            .env_remove("WAYLAND_SOCKET")
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
            .expect("lanzar nota");
        Nota {
            child,
            display: &self.display,
            log: log_path,
        }
    }
}

/// Un proceso `nota` corriendo. Si el test falla, su log sale en la salida.
pub struct Nota<'a> {
    child: Child,
    display: &'a Display,
    log: PathBuf,
}

impl<'a> Nota<'a> {
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
        wait::until("la ventana de nota", WINDOW_TIMEOUT, || {
            if let Some(status) = self.try_exit() {
                panic!("nota terminó con {status} sin abrir la ventana");
            }
            self.windows()
                .into_iter()
                .next()
                .ok_or_else(|| "ninguna ventana".to_owned())
        })
    }

    /// Lo que lleva escrito en stdout y stderr.
    pub fn log(&self) -> String {
        fs::read_to_string(&self.log).expect("leer el log de nota")
    }

    pub fn is_running(&mut self) -> bool {
        self.try_exit().is_none()
    }

    pub fn wait_exit(&mut self) -> ExitStatus {
        wait::until("que nota termine", TIMEOUT, || {
            self.try_exit().ok_or_else(|| "sigue corriendo".to_owned())
        })
    }

    fn try_exit(&mut self) -> Option<ExitStatus> {
        self.child.try_wait().expect("consultar el proceso")
    }
}

impl Drop for Nota<'_> {
    fn drop(&mut self) {
        if self.is_running() {
            // Un proceso colgado haría que el test nunca terminara.
            let _ = self.child.kill();
            let _ = self.child.wait();
        }
        if std::thread::panicking() {
            let log = fs::read_to_string(&self.log).unwrap_or_default();
            eprintln!("--- log de nota ---\n{log}--- fin del log ---");
        }
    }
}

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

    /// Pide el cierre como lo haría el gestor de ventanas.
    pub fn request_close(&self) {
        self.display.request_close(self.id);
    }
}
