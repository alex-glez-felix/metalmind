#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "en tests, un unwrap que falla es un test que falla"
)]

//! Los subcomandos contra el binario de verdad, sin pantalla: si alguno
//! tocara GPUI, sin `DISPLAY` ni `WAYLAND_DISPLAY` fallaría o se colgaría.

use std::fs::{self, Permissions};
use std::io::Write as _;
use std::os::unix::fs::{PermissionsExt as _, symlink};
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::time::{Duration, Instant};

use pretty_assertions::assert_eq;
use serde_json::Value;
use tempfile::TempDir;

/// Un `HOME` y un `XDG_*` vacíos, para que nada del usuario se cuele, y un vault.
struct Sandbox {
    home: TempDir,
    vault: PathBuf,
}

impl Sandbox {
    fn new() -> Self {
        let home = TempDir::new().unwrap();
        let vault = home.path().join("vault");
        fs::create_dir(&vault).unwrap();
        Self { home, vault }
    }

    /// `nota` sin vault elegido: ni `--vault` ni `NOTA_VAULT`.
    fn bare(&self) -> Command {
        let home = self.home.path();
        let mut command = Command::new(env!("CARGO_BIN_EXE_nota"));
        command
            .current_dir(home)
            .env_clear()
            .env("HOME", home)
            .env("XDG_CONFIG_HOME", home.join("config"))
            .env("XDG_STATE_HOME", home.join("state"))
            .env("XDG_DATA_HOME", home.join("data"))
            .env("XDG_CACHE_HOME", home.join("cache"))
            .stdin(Stdio::null());
        command
    }

    /// `nota` con el vault del sandbox en `NOTA_VAULT`.
    fn nota(&self) -> Command {
        let mut command = self.bare();
        command.env("NOTA_VAULT", &self.vault);
        command
    }

    fn read(&self, path: &str) -> String {
        fs::read_to_string(self.vault.join(path)).unwrap()
    }
}

fn run(command: &mut Command, args: &[&str]) -> Output {
    command.args(args).output().unwrap()
}

fn run_with_stdin(command: &mut Command, args: &[&str], stdin: &[u8]) -> Output {
    let mut child = command
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(stdin).unwrap();
    child.wait_with_output().unwrap()
}

fn stdout(output: &Output) -> &str {
    std::str::from_utf8(&output.stdout).unwrap()
}

fn stderr(output: &Output) -> &str {
    std::str::from_utf8(&output.stderr).unwrap()
}

fn assert_ok(output: &Output) {
    assert!(output.status.success(), "{output:?}");
    assert_eq!(stderr(output), "");
}

/// Una sola línea de JSON, que es lo que promete `--json`.
fn json_line(text: &str) -> Value {
    assert!(text.ends_with('\n'), "{text:?}");
    assert_eq!(text.lines().count(), 1, "{text:?}");
    serde_json::from_str(text).unwrap()
}

/// El error de stderr, que es su última línea: antes puede haber avisos.
fn json_error(text: &str) -> Value {
    assert!(text.ends_with('\n'), "{text:?}");
    serde_json::from_str(text.lines().last().unwrap()).unwrap()
}

/// Si un valor JSON es del tipo que espera la forma documentada.
type IsType = fn(&Value) -> bool;

/// `value` es un objeto con exactamente estas claves, cada una de este tipo.
fn assert_shape(value: &Value, fields: &[(&str, IsType)]) {
    let object = value.as_object().unwrap_or_else(|| panic!("{value}"));
    let mut keys: Vec<&str> = object.keys().map(String::as_str).collect();
    keys.sort_unstable();
    let mut expected: Vec<&str> = fields.iter().map(|(key, _)| *key).collect();
    expected.sort_unstable();
    assert_eq!(keys, expected, "{value}");
    for (key, is_type) in fields {
        assert!(is_type(&object[*key]), "`{key}` tiene otro tipo en {value}");
    }
}

fn is_string_array(value: &Value) -> bool {
    value
        .as_array()
        .is_some_and(|items| items.iter().all(Value::is_string))
}

/// La forma de `docs/cli.md` para un error, con su `kind`.
fn assert_json_error(output: &Output, kind: &str, code: i32) {
    assert_eq!(output.status.code(), Some(code), "{output:?}");
    assert_eq!(stdout(output), "");
    let value = json_error(stderr(output));
    assert_shape(&value, &[("error", Value::is_object)]);
    assert_shape(
        &value["error"],
        &[("kind", Value::is_string), ("message", Value::is_string)],
    );
    assert_eq!(value["error"]["kind"], kind, "{value}");
}

fn canonical(path: &Path) -> String {
    path.canonicalize().unwrap().to_str().unwrap().to_owned()
}

#[test]
fn add_con_texto_imprime_la_ruta_y_cae_en_el_buzon() {
    let sandbox = Sandbox::new();
    let output = run(&mut sandbox.nota(), &["add", "Comprar", "leche"]);
    assert_ok(&output);
    let path = stdout(&output).strip_suffix('\n').unwrap();
    assert_eq!(path, "comprar-leche.md");
    let text = sandbox.read(path);
    assert_eq!(text, "Comprar leche\n");
    assert_eq!(nota_syntax::scan(&text).tags, []);
}

#[test]
fn add_lee_stdin() {
    let sandbox = Sandbox::new();
    let output = run_with_stdin(
        &mut sandbox.nota(),
        &["add"],
        b"idea desde un pipe\nsegunda\n",
    );
    assert_ok(&output);
    let path = stdout(&output).trim_end();
    assert_eq!(path, "idea-desde-un-pipe.md");
    assert_eq!(sandbox.read(path), "idea desde un pipe\nsegunda\n");
}

#[test]
fn add_json_con_tags_y_titulo() {
    let sandbox = Sandbox::new();
    let output = run_with_stdin(
        &mut sandbox.nota(),
        &[
            "add", "--json", "-t", "rust", "--tag", "#gpui", "--title", "Plan",
        ],
        b"cuerpo\n",
    );
    assert_ok(&output);
    let value = json_line(stdout(&output));
    assert_shape(
        &value,
        &[
            ("vault", Value::is_string),
            ("path", Value::is_string),
            ("tags", is_string_array),
        ],
    );
    assert_eq!(value["vault"], canonical(&sandbox.vault));
    assert_eq!(value["tags"], serde_json::json!(["rust", "gpui"]));
    let path = value["path"].as_str().unwrap();
    assert_eq!(path, "plan.md");
    let text = sandbox.read(path);
    assert_eq!(
        text,
        "---\ntags: [\"rust\", \"gpui\"]\n---\n# Plan\n\ncuerpo\n"
    );
    let tags: Vec<String> = nota_syntax::scan(&text)
        .frontmatter
        .unwrap()
        .tags
        .into_iter()
        .map(|tag| tag.text)
        .collect();
    assert_eq!(tags, ["rust", "gpui"]);
}

#[test]
fn add_no_pisa_una_nota_con_el_mismo_nombre() {
    let sandbox = Sandbox::new();
    let first = run(&mut sandbox.nota(), &["add", "idea"]);
    let second = run(&mut sandbox.nota(), &["add", "idea"]);
    assert_ok(&first);
    assert_ok(&second);
    let third = run(&mut sandbox.nota(), &["add", "Idea"]);
    assert_ok(&third);
    assert_eq!(stdout(&first), "idea.md\n");
    assert_eq!(stdout(&second), "idea-2.md\n");
    assert_eq!(stdout(&third), "idea-3.md\n");
    assert_eq!(sandbox.read("idea.md"), "idea\n");
    assert_eq!(sandbox.read("idea-3.md"), "Idea\n");
}

#[test]
fn add_sin_texto_es_error_de_uso() {
    let sandbox = Sandbox::new();
    let output = run_with_stdin(&mut sandbox.nota(), &["add", "--json"], b"  \n");
    assert_json_error(&output, "usage", 2);
    let output = run_with_stdin(&mut sandbox.nota(), &["add"], b"\xff\xfe");
    assert_eq!(output.status.code(), Some(2), "{output:?}");
    assert!(stderr(&output).starts_with("nota: "), "{output:?}");
    let output = run(&mut sandbox.nota(), &["add", "x", "-t", "con espacio"]);
    assert_eq!(output.status.code(), Some(2), "{output:?}");
    assert_eq!(fs::read_dir(&sandbox.vault).unwrap().count(), 0);
}

#[test]
fn search_busca_rutas_del_mejor_al_peor() {
    let sandbox = Sandbox::new();
    for path in [
        "proyectos/gpui.md",
        "gpui-notas.md",
        "otra.md",
        "img/gpui.png",
    ] {
        let full = sandbox.vault.join(path);
        fs::create_dir_all(full.parent().unwrap()).unwrap();
        fs::write(full, "x").unwrap();
    }
    let output = run(&mut sandbox.nota(), &["search", "gpui"]);
    assert_ok(&output);
    let mut lines: Vec<&str> = stdout(&output).lines().collect();
    lines.sort_unstable();
    assert_eq!(
        lines,
        ["gpui-notas.md", "img/gpui.png", "proyectos/gpui.md"]
    );

    let output = run(
        &mut sandbox.nota(),
        &["search", "--json", "-n", "2", "gpui"],
    );
    assert_ok(&output);
    let value = json_line(stdout(&output));
    assert_shape(
        &value,
        &[
            ("vault", Value::is_string),
            ("query", Value::is_string),
            ("results", Value::is_array),
        ],
    );
    assert_eq!(value["query"], "gpui");
    let results = value["results"].as_array().unwrap();
    assert_eq!(results.len(), 2);
    for result in results {
        assert_shape(
            result,
            &[("path", Value::is_string), ("kind", Value::is_string)],
        );
        let kind = result["kind"].as_str().unwrap();
        assert!(["note", "attachment"].contains(&kind), "{result}");
    }
}

#[test]
fn search_vacio_da_las_primeras_n_por_ruta() {
    let sandbox = Sandbox::new();
    for name in ["c.md", "a.md", "b.md"] {
        fs::write(sandbox.vault.join(name), "x").unwrap();
    }
    let output = run(&mut sandbox.nota(), &["search", "-n", "2", ""]);
    assert_ok(&output);
    assert_eq!(stdout(&output), "a.md\nb.md\n");
}

#[test]
fn search_sin_coincidencias_no_es_error() {
    let sandbox = Sandbox::new();
    let output = run(&mut sandbox.nota(), &["search", "--json", "nada"]);
    assert_ok(&output);
    assert_eq!(json_line(stdout(&output))["results"], serde_json::json!([]));
}

#[test]
fn search_por_tags_todavia_no_existe() {
    let sandbox = Sandbox::new();
    let output = run(
        &mut sandbox.nota(),
        &["search", "--json", "parser -#archivado"],
    );
    assert_json_error(&output, "usage", 2);
}

#[test]
fn sin_vault_lo_dice_con_su_codigo() {
    let sandbox = Sandbox::new();
    let output = run(&mut sandbox.bare(), &["add", "--json", "x"]);
    assert_json_error(&output, "vault", 3);
    let output = run(&mut sandbox.bare(), &["search", "x"]);
    assert_eq!(output.status.code(), Some(3));
    assert!(stderr(&output).contains("--vault"), "{output:?}");
}

#[test]
fn un_vault_inexistente_es_error_de_vault() {
    let sandbox = Sandbox::new();
    let missing = sandbox.home.path().join("no-existe");
    let output = run(
        &mut sandbox.nota(),
        &["add", "--json", "--vault", missing.to_str().unwrap(), "x"],
    );
    assert_json_error(&output, "vault", 3);
    let message = json_error(stderr(&output))["error"]["message"].clone();
    assert!(
        message
            .as_str()
            .unwrap()
            .contains("no se pudo abrir el vault"),
        "{message}"
    );
}

#[test]
fn un_vault_sin_permiso_de_lectura_es_error_de_vault() {
    let sandbox = Sandbox::new();
    fs::set_permissions(&sandbox.vault, Permissions::from_mode(0o300)).unwrap();
    let output = run(&mut sandbox.nota(), &["search", "--json", "x"]);
    fs::set_permissions(&sandbox.vault, Permissions::from_mode(0o700)).unwrap();
    assert_json_error(&output, "vault", 3);
}

#[test]
fn un_vault_sin_permiso_de_escritura_es_error_de_io() {
    let sandbox = Sandbox::new();
    fs::set_permissions(&sandbox.vault, Permissions::from_mode(0o500)).unwrap();
    let output = run(&mut sandbox.nota(), &["add", "--json", "x"]);
    fs::set_permissions(&sandbox.vault, Permissions::from_mode(0o700)).unwrap();
    assert_json_error(&output, "io", 1);
    assert_eq!(fs::read_dir(&sandbox.vault).unwrap().count(), 0);
}

#[test]
fn el_vault_sale_de_flag_entorno_ultimo_abierto_y_config() {
    let sandbox = Sandbox::new();
    let other = sandbox.home.path().join("otro");
    fs::create_dir(&other).unwrap();
    let vault_of = |command: &mut Command, args: &[&str]| {
        let output = run(command, &[&["search", "--json", ""], args].concat());
        assert_ok(&output);
        json_line(stdout(&output))["vault"]
            .as_str()
            .unwrap()
            .to_owned()
    };
    let home = sandbox.home.path();
    fs::create_dir_all(home.join("config/nota")).unwrap();
    fs::write(home.join("config/nota/config.yaml"), "vault: ~/otro\n").unwrap();
    assert_eq!(vault_of(&mut sandbox.bare(), &[]), canonical(&other));
    fs::create_dir_all(home.join("state/nota")).unwrap();
    fs::write(
        home.join("state/nota/last-vault"),
        format!("{}\n", sandbox.vault.display()),
    )
    .unwrap();
    assert_eq!(
        vault_of(&mut sandbox.bare(), &[]),
        canonical(&sandbox.vault)
    );
    let with_env = || {
        let mut command = sandbox.bare();
        command.env("NOTA_VAULT", &other);
        command
    };
    assert_eq!(vault_of(&mut with_env(), &[]), canonical(&other));
    let flag = sandbox.vault.to_str().unwrap();
    assert_eq!(
        vault_of(&mut with_env(), &["--vault", flag]),
        canonical(&sandbox.vault)
    );
}

/// Sin pantalla, una nota válida llega hasta la ventana, que no puede abrirse
/// (#31): eso prueba que `open` la resolvió y la leyó. El
/// error de la ventana también sale en JSON.
#[test]
fn open_de_una_nota_llega_a_la_ventana() {
    let sandbox = Sandbox::new();
    fs::create_dir(sandbox.vault.join("sub")).unwrap();
    fs::write(sandbox.vault.join("sub/a.md"), "x\n").unwrap();
    let output = run(&mut sandbox.nota(), &["open", "--json", "sub/a.md"]);
    assert_eq!(output.status.code(), Some(1), "{output:?}");
    let value = json_line(stdout(&output));
    assert_shape(
        &value,
        &[("vault", Value::is_string), ("path", Value::is_string)],
    );
    assert_eq!(value["path"], "sub/a.md");
    let error = json_error(stderr(&output));
    assert_eq!(error["error"]["kind"], "io", "{error}");
    let message = error["error"]["message"].as_str().unwrap();
    assert!(message.contains("no hay sesión gráfica"), "{message}");
}

/// Si la nota no se puede leer, `--json` no anuncia nada: solo el error.
#[test]
fn open_de_una_nota_ilegible_no_anuncia_exito() {
    let sandbox = Sandbox::new();
    fs::write(sandbox.vault.join("binaria.md"), b"\xff\xfe").unwrap();
    let output = run(&mut sandbox.nota(), &["open", "--json", "binaria.md"]);
    assert_json_error(&output, "io", 1);
}

/// Estos casos fallan antes de abrir la ventana.
#[test]
fn open_solo_acepta_notas_del_vault() {
    let sandbox = Sandbox::new();
    fs::write(sandbox.vault.join("img.png"), "x").unwrap();
    let outside = sandbox.home.path().join("fuera.md");
    fs::write(&outside, "x").unwrap();
    let elsewhere = sandbox.home.path().join("afuera");
    fs::create_dir(&elsewhere).unwrap();
    fs::write(elsewhere.join("nota.md"), "x").unwrap();
    symlink(&elsewhere, sandbox.vault.join("enlace")).unwrap();
    symlink(&outside, sandbox.vault.join("enlazada.md")).unwrap();
    for path in [
        "no-existe.md",
        "img.png",
        "../fuera.md",
        outside.to_str().unwrap(),
        "enlace/nota.md",
        "enlazada.md",
    ] {
        let output = run(&mut sandbox.nota(), &["open", "--json", path]);
        assert_json_error(&output, "usage", 2);
    }
    assert!(!sandbox.home.path().join("state/nota/last-vault").exists());
}

#[test]
fn los_errores_de_argumentos_salen_con_2() {
    let sandbox = Sandbox::new();
    let output = run(&mut sandbox.nota(), &["add", "--raro"]);
    assert_eq!(output.status.code(), Some(2), "{output:?}");
    let output = run(&mut sandbox.nota(), &["search"]);
    assert_eq!(output.status.code(), Some(2), "{output:?}");
}

#[test]
fn la_ayuda_de_un_subcomando_no_abre_nada() {
    let sandbox = Sandbox::new();
    let output = run(&mut sandbox.nota(), &["add", "--help"]);
    assert_ok(&output);
    assert!(stdout(&output).contains("uso: nota add"), "{output:?}");
}

/// Un subcomando no arranca la GUI: termina en lo que tarda leer el vault,
/// aun en un build de debug y en una máquina de CI cargada.
#[test]
fn add_y_search_terminan_rapido() {
    let sandbox = Sandbox::new();
    for i in 0..500 {
        fs::write(sandbox.vault.join(format!("nota-{i}.md")), "x\n").unwrap();
    }
    for args in [&["add", "rápida"][..], &["search", "nota 42"]] {
        let started = Instant::now();
        let output = run(&mut sandbox.nota(), args);
        let elapsed = started.elapsed();
        assert_ok(&output);
        assert!(
            elapsed < Duration::from_millis(500),
            "{args:?} tardó {elapsed:?}"
        );
    }
}
