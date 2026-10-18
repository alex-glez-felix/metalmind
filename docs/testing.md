# Tests

```sh
cargo nextest run --workspace     # todo lo que no necesita pantalla
```

Los casos de `nota-vim` también corren en `nvim --headless` 0.12.6; sin ese `nvim` en el `PATH` se ignoran (ver `crates/nota-vim/tests/support/nvim.rs`). Lo mismo los de `nota-ui` que cambian de panel o de archivo desde un modo a medias, que comparan modo, archivo y cursor de cada panel con nvim (`crates/nota-ui/src/workspace/panes_oracle.rs`).

## Smoke de la app real

`crates/nota/tests/gui_smoke.rs` lanza el binario `nota` de verdad, le teclea con XTest y comprueba el **efecto en disco** y **cómo termina el proceso**, no los píxeles. Cada test levanta su propio Xvfb, en el primer display libre desde `:100`, así que corren en paralelo sin pisarse el foco. Están en `#[ignore]` porque necesitan Xvfb y un driver que dibuje sin GPU; el job `gui-smoke` del CI los corre en cada push a `main` (en una PR, sin caché, compilar le toma más de 10 min; los tests, segundos). En una PR que toque la app, córrelo en local o lanza el CI a mano sobre la rama: `gh workflow run CI --ref <rama>`.

### Correrlo en local

Hace falta Xvfb, un driver que dibuje sin GPU (lavapipe, el Vulkan por software de mesa, es el del CI; ver abajo) y una fuente monoespaciada, además de las librerías de GPUI de [Primeros pasos](GETTING-STARTED.md#correr-nota):

```sh
sudo pacman -S --needed xorg-server-xvfb vulkan-swrast ttf-dejavu                       # Arch
sudo apt-get install -y xvfb mesa-vulkan-drivers fonts-dejavu-core                      # Debian/Ubuntu

cargo nextest run -p nota --test gui_smoke --run-ignored only

# En un display fijo, de uno en uno:
NOTA_SMOKE_DISPLAY=97 cargo nextest run -p nota --test gui_smoke --run-ignored only --test-threads 1
```

No hace falta `xvfb-run` ni un `DISPLAY`: el harness abre su servidor. `nota` corre con `HOME` y `XDG_*` apuntando a una carpeta vacía (config por defecto) y sin `WAYLAND_DISPLAY` ni `WAYLAND_SOCKET`, para que GPUI use X11 y no abra nada en tu escritorio. Si un test falla, la salida trae el log de `nota` con `RUST_LOG=info`, que dice qué adaptador eligió GPUI. Los logs de `nota` y de Xvfb quedan en la carpeta temporal del test mientras corre.

### Nunca sobre tu pantalla

El smoke teclea con XTest, que manda teclas al servidor X entero: corrido sobre tu escritorio, escribiría en lo que tengas enfocado. El harness (`support/xvfb.rs`) lo hace imposible:

- El display solo sale de un Xvfb que lanzó el propio test, sin `-displayfd`, para que Xvfb cree y respete el lock de X. Si el display está ocupado, Xvfb sale y se prueba el siguiente.
- Antes de conectarse, el test espera a que `/tmp/.XN-lock` tenga el pid de **su** Xvfb, y se niega si el número es el del `DISPLAY` del entorno.
- Si Xvfb no está o no arranca, el test **aborta**; nunca cae al `DISPLAY` del entorno, que no se lee más que para esa comprobación.
- `DISPLAY` solo se le pasa a `nota` (`Command::env`), nunca al proceso de los tests, y `nota` corre sin `WAYLAND_DISPLAY` ni `WAYLAND_SOCKET`.
- Al terminar, el Xvfb recibe SIGTERM, que borra su socket y su lock.

`sin_xvfb_aborta_en_vez_de_usar_el_display_del_entorno` y `un_display_ocupado_no_se_comparte` lo prueban.

### Dibujar sin GPU

GPUI (`gpui-pre-wgpu`) junta los adaptadores de Vulkan y de GL, los ordena (GPU discreta, integrada, …, CPU; a igual tipo, Vulkan antes que GL) y usa el primero que puede configurar la superficie de la ventana. Sin GPU hay dos por software, ambos de mesa y `type=Cpu` en el log: lavapipe (Vulkan) y llvmpipe por EGL (GL). Gana lavapipe si está; si no, GL basta para el smoke. Con lavapipe, cada caso tarda menos de 1 s en un build de debug.

- **Sin ningún adaptador, no abre**: `nota` dice `no se pudo abrir la ventana: Failed to create surface…`, sugiere instalar el driver y sale con código 1; el test que espera la ventana falla en cuanto sale, con ese mensaje en el log. `sin_adaptador_termina_con_error` lo comprueba con `smoke.launch_without_gpu`, que apunta los loaders de Vulkan y EGL a un archivo que no existe.
- **En una máquina con GPU**, el driver de la GPU va primero. El Vulkan de NVIDIA presenta en Xvfb y el smoke pasa igual; con otros drivers no se verificó. Para forzar lavapipe, apunta el loader a su ICD; el harness le pasa el entorno a `nota`:

  ```sh
  VK_DRIVER_FILES=/usr/share/vulkan/icd.d/lvp_icd.json \
    cargo nextest run -p nota --test gui_smoke --run-ignored only
  ```

  El nombre del archivo cambia según la distro (`lvp_icd.json` en Ubuntu; en otras lleva la arquitectura, `lvp_icd.x86_64.json`): búscalo en `/usr/share/vulkan/icd.d/`. `VK_ICD_FILENAMES` es el nombre viejo de la misma variable. `ZED_DEVICE_ID` (el id PCI en hex) elige el adaptador dentro de GPUI.

### Agregar un caso

Si el caso es "nota con este texto → teclas → `:wq` → este texto", es una línea más en la tabla de `edita_y_guarda`:

```rust
#[case::nombre("antes\n", "teclas<Esc>:wq<CR>", "después\n")]
```

Las teclas van en la notación de vim: `<Esc>`, `<CR>`, `<BS>`, `<Tab>`, `<Space>`, `<Up>`… `<C-r>` para control, `<C-S-p>` para control con shift y `<lt>` para un `<` literal (`support/keys.rs`). Las mayúsculas y los símbolos con shift los resuelve el mapa de teclado del Xvfb.

Para lo demás, un `#[test]` con `#[ignore = "necesita Xvfb y Vulkan: ver docs/testing.md"]` y la API de `support/`:

- `Smoke::new()`: un Xvfb y una carpeta de notas vacía. `write`, `read`, `set_mode`, `mode`, `files` (para ver que no quedan temporales), `wait_files` (que la carpeta tenga justo esos archivos, por ejemplo tras renombrar desde el árbol) y `wait_contents` trabajan sobre esa carpeta.
- `smoke.launch(&["a.md", …])`: lanza `nota` con esos argumentos desde la carpeta de notas, que es también el vault (con `&["."]`, sin archivo y con el foco en el árbol). Devuelve un `Nota`.
- `smoke.launch_without_gpu(&["a.md"])`: igual, pero sin ningún driver de Vulkan ni EGL a la vista, para probar que `nota` sale con error cuando no puede dibujar.
- `nota.window()` espera la primera ventana; `nota.windows()` da todas las que tiene abiertas ahora, sin asumir una sola. `nota.wait_exit()` espera la salida y devuelve el `ExitStatus`; `nota.is_running()` no espera.
- `window.type_keys("…")`, `window.title()`, `window.wait_title(…)` y `window.request_close()`, que manda `WM_DELETE_WINDOW` como la "x" de un gestor de ventanas.

**Nada de `sleep`.** Teclear vuelve cuando el servidor X procesó las teclas, no cuando `nota` las leyó; lo que se espera es un efecto: que el archivo cambie (`wait_contents`), que el proceso termine (`wait_exit`) o el título (`wait_title`, que tiene `[+]` si hay cambios). Para probar que algo **no** pasó (`:q` con cambios no cierra), se teclea detrás algo con efecto visible y se espera ese efecto: las teclas llegan en orden, así que si `:q` hubiera cerrado, el `:w` siguiente nunca llegaría al disco.

Cada espera tiene tope (`support/wait.rs`): 1 min para la primera ventana, 15 s para lo demás. Si se acaba, el test falla con lo último que vio.

### Lo que encontró

- nota no tenía el `fixeol` de vim: `ihola<Esc>:wq` escribía `hola` sin `\n`, y en `uno\ndos\n`, `"ayyjdd"ap` dejaba `uno\n\nuno` por la línea vacía de más al final. Arreglado; los dos casos quedan en el smoke.
- Si GPUI no puede crear la superficie, `nota` imprime el error pero el proceso no termina: el `cx.quit()` de `main.rs` se pide antes de que arranque el bucle de eventos y se pierde ([#31](https://github.com/alex-glez-felix/nota/issues/31)).
- Con `-displayfd`, Xvfb ni mira ni escribe el lock de X (`/tmp/.XN-lock`), aunque se le dé el display: en un escritorio con XWayland, cuyo socket de `:0` no siempre está en `/tmp/.X11-unix`, se quedó con el `:0` de la sesión. Por eso el harness no lo usa (ver "Nunca sobre tu pantalla").
