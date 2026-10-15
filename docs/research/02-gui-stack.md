# 02 — Stack de GUI en Rust para "nota"

> Investigación del 2026-10-15. Datos de versiones y fechas sacados de la API de crates.io y de GitHub ese día; arquitectura de Zed leída de sus `Cargo.toml` en `main`. Lo que no pude comprobar va marcado **(sin verificar)**. La búsqueda web estuvo caída durante la investigación, así que todo salió de fetches directos a repos, manifests y APIs.

## TL;DR

- **Recomendación: GPUI**, usado a través de **GPUI Kit / gpui-component (Longbridge)**, que publica en crates.io snapshots fijados de upstream (`gpui-pre`). Es el único stack de esta lista que ya se probó en un editor de texto de alto rendimiento en Linux (Zed). Trae un editor de código que aguanta 200K líneas, virtual lists y render de Markdown.
- **Alternativa: iced + cosmic-text.** Más estable para depender de él, la IA lo conoce mejor y COSMIC lo usa en producción en Linux. El editor serio te lo tendrías que construir tú encima de `cosmic-text::Editor`.
- **Riesgo principal de GPUI:** Zed no publica `gpui` en crates.io desde 0.2.2 (2025-10-22), la API cambia seguido y la IA la conoce peor que la de iced o egui. Para mitigarlo: fijar una versión exacta, meter docs y ejemplos dentro del repo como contexto para los agentes, y aislar la UI detrás de un core propio.

---

## 1. Cómo funciona una app GUI nativa en Linux (para principiantes)

Una ventana con texto pasa por estas capas:

1. **Windowing (servidor gráfico).** En Linux hay dos protocolos:
   - **Wayland**: el moderno. La app es un cliente que habla con el *compositor* (GNOME/Mutter, KDE/KWin, Hyprland, Sway…) por un socket. Lo que no está en el núcleo (decoraciones, IME, portapapeles primario, fraccional scaling) va en **protocolos extra** (`xdg-shell`, `text-input-v3`, `fractional-scale-v1`, `xdg-decoration`…) y no todos los compositores los implementan igual.
   - **X11**: el viejo. Sigue siendo necesario por XWayland y por usuarios en X. Se usa con `x11rb`/`xcb`; el IME va por **XIM**.
   - **winit** es la crate estándar que abstrae ambos (y macOS/Windows) y te da una ventana más eventos. La usan iced, egui, Slint, Xilem, Floem y Bevy. Zed **no** usa winit: tiene su propia capa de plataforma (`gpui_linux`) sobre `wayland-client` y `x11rb`.
2. **Event loop.** Un bucle que espera eventos del sistema (teclado, ratón, resize, "frame callback" del compositor), los despacha a tu app y pide redibujar. En Linux suele montarse sobre `calloop` o `epoll`. Con un buen event loop la app se mantiene en 0% de CPU cuando nada cambia y solo redibuja cuando toca, sincronizado con el refresco del monitor (60/120/144 Hz).
3. **Teclado.** El compositor manda *keycodes*. **xkbcommon** los traduce a símbolos según el layout (us, latam, dead keys). Para un modo vim esto importa: hay que distinguir bien keycode, símbolo y texto producido.
4. **IME (input method).** Para escribir acentos compuestos, CJK, emoji, etc. La app recibe *preedit* (texto provisional subrayado) y *commit*. En Wayland va por `text-input-v3` hacia IBus o Fcitx5, en X11 por XIM. Es una de las partes donde más fallan los toolkits jóvenes.
5. **Render por GPU.** La app arma una lista de primitivas (rects, glifos, imágenes) y la manda a la GPU:
   - **wgpu**: abstracción de WebGPU sobre Vulkan, Metal, DX12 y GL. Es el estándar de facto en Rust (v30 hoy).
   - **Blade**: la abstracción más delgada que hizo Zed. Zed **la quitó** y reescribió su renderer de Linux con wgpu (PR #46758, enero 2026).
   - **Vello**: renderer 2D vectorial por compute shaders (Linebender), encima de wgpu.
   - **Skia**: el de Chrome/Flutter (Freya, Slint opcional). Es C++, así que el build pesa más.
   - **femtovg / tiny-skia**: OpenGL ES 2 y CPU respectivamente; sirven de fallback.
6. **Texto.** Es la parte más difícil y la más importante para "nota":
   - **Font discovery**: encontrar qué fuentes hay instaladas y cuál usar como fallback (emoji, CJK). En Linux se resuelve con **fontconfig** (vía `font-kit` o `fontique`).
   - **Shaping**: convertir una cadena en glifos posicionados (ligaduras, árabe, kerning). El estándar es **HarfBuzz**; en Rust puro hay **rustybuzz** y **harfrust**.
   - **Layout**: saltos de línea, bidi, wrapping. Lo hacen **cosmic-text** (pop-os) y **parley** (Linebender).
   - **Rasterización**: glifo → bitmap con antialiasing. **swash** en Rust puro. Los bitmaps se guardan en un *atlas* de textura en la GPU (`etagere`).
7. **Integración con el escritorio.** El **portapapeles** (Wayland `wl_data_device`, primario con `zwp_primary_selection`; X11 selections), **drag & drop**, **notificaciones**, **keyring** (Secret Service) y los **xdg-desktop-portal** (vía `ashpd`): diálogos de abrir/guardar nativos, abrir URLs y tema oscuro/claro del sistema. Los portals son lo que hace que funcione dentro de Flatpak.
8. **Accesibilidad.** En Linux se expone por **AT-SPI** (D-Bus) para lectores como Orca. En Rust, la crate **AccessKit** lo abstrae (`accesskit_unix`).

### Cómo lo hace Zed (GPUI) hoy

Verificado en `crates/gpui_linux/Cargo.toml` y `crates/gpui_wgpu/Cargo.toml` de `main`:

- **Plataforma propia, sin winit**: `wayland-client` + `wayland-protocols` (+ protocolos `plasma` y `wlr`), `x11rb` con xkb/randr/xinput/dri3, `xkbcommon`, event loop con `calloop`.
- **IME**: XIM en X11 (fork `zed-xim`). En Wayland usa `text-input` **(sin verificar el protocolo exacto, se asume v3)**.
- **Render**: `gpui_wgpu`, con wgpu en Linux con backends Vulkan y GLES. Blade ya no está.
- **Texto en Linux**: `cosmic-text 0.19` + `swash` + atlas con `etagere`; `font-kit` opcional para fontconfig.
- **Desktop**: `ashpd` (portals), `oo7` (keyring), `notify-rust`, y `accesskit` / `accesskit_unix` ya aparecen como dependencias. Qué tan completa está la accesibilidad está **(sin verificar)**.
- **Modelo de UI**: híbrido entre modo inmediato y retenido. Cada frame se reconstruye un árbol de *elements* con layout de **Taffy** (flexbox), y el estado vive en *entities* con referencia contada que maneja `App`/`Context`. El estilo se escribe tipo Tailwind (`div().flex().p_2()`). Hay *actions* y *keymaps* por contexto, que es justo lo que necesita un modo vim.

---

## 2. Comparativa

Datos del 2026-10-15. "Conocimiento de la IA" es una estimación mía según volumen de código público, edad de la API y churn; **no es una medición**.

| Toolkit | Última versión (crates.io) | ★ GitHub | Licencia | Madurez | Wayland/X11 | Texto / IME | Widget editor | Render custom | A11y | Docs | Conocimiento IA |
|---|---|---|---|---|---|---|---|---|---|---|---|
| **GPUI (Zed)** | `gpui` 0.2.2 (2025-10-22, viejo); `gpui-pre` 0.3.9 (2026-10-12); `gpui-ce` 0.2.2 (2026-08-28); `gpui-unofficial` 1.25.0-pre | Zed 91.7k, gpui-ce 1.4k | Apache-2.0 (gpui) | Probada en producción (Zed), pero como librería independiente es "alpha" | Muy buena, la mantiene un equipo pago y la usan miles a diario | cosmic-text + swash; IME XIM/Wayland | Sí: gpui-component (200K líneas, tree-sitter, LSP, Markdown) | Total (elements propios, `canvas`, shaders no) | Inicial (accesskit en deps) | Escasas; se aprende leyendo Zed y gpui-component | **Media-baja**, API cambia rápido |
| **iced** | 0.14.0 (2025-12-07) | 31.8k | MIT | "Experimental" según el README, pero en producción (COSMIC, Halloy) | Buena vía winit; COSMIC es su caso de uso principal | cosmic-text; IME añadido en 0.14 | `text_editor` básico (highlighting, sin vim/virtualización avanzada) | Alta (`Widget` trait, `shader`, `canvas`) | Débil/nula en upstream **(sin verificar)** | Book incompleto, buenos ejemplos | **Alta** (pero mezcla APIs viejas, ver riesgos) |
| **Slint** | 1.18.1 (2026-09-21) | 24.2k | GPLv3 / royalty-free / comercial | Estable, API 1.x | Buena (winit, Qt, linuxkms) | Correcto; `TextEdit` simple | No apto para editor serio | Limitada (DSL `.slint`) | Buena (AccessKit) **(sin verificar detalle)** | Excelentes | Media |
| **egui** | 0.37.0 (2026-10-15) | 31.5k | MIT/Apache-2.0 | Muy madura en su nicho (herramientas) | Buena vía winit | Shaping limitado (sin HarfBuzz completo **(sin verificar estado actual)**); IME básico | `TextEdit` no escala a documentos grandes | Alta (painter), pero estética de "debug tool" | AccessKit | Muy buenas | **Muy alta** |
| **gtk4-rs + libadwaita (relm4)** | gtk4 0.11.5, libadwaita 0.9.2, relm4 0.11.0 | 2.4k / 2.0k | MIT (bindings), LGPL (GTK) | Muy madura (GNOME) | Excelente (la referencia en Linux) | Pango + HarfBuzz, IME perfecto | `GtkTextView` / **GtkSourceView 5** (`sourceview5` 0.11.2) | Media (snapshot/GSK), pelear con GTK | La mejor en Linux (AT-SPI nativo) | Buenas (C) + gtk-rs book | Media (los bindings Rust confunden a la IA) |
| **Xilem / Masonry** | 0.4.0 (2025-10-29) | 5.6k | Apache-2.0 | "Experimental" | winit | Parley (muy bueno), IME en progreso **(sin verificar)** | No | Alta (Vello) | AccessKit desde el diseño | Pocas | Baja |
| **Floem (Lapce)** | 0.2.0 (2024-11-14) en crates.io; git activo | 4.3k | MIT | "Still maturing", releases parados | winit | cosmic-text **(sin verificar versión)** | `editor-core` (el de Lapce) | Alta (vello/vger/skia) | Escasa **(sin verificar)** | Pocas | Baja |
| **Freya** | 0.5.0-rc.10 (estable 0.4) | 3.2k | MIT | Joven, un mantenedor principal | winit | Skia Paragraph (HarfBuzz) | Rich text editor + editor con rope y tree-sitter | Alta (Skia) | `a11y_id` + AccessKit | Medias | Baja (ya no usa Dioxus desde 0.4, así que el conocimiento de Dioxus no aplica) |
| **Makepad** | `makepad-widgets` 1.0.0 (2025-05-13); dev activo en git | 7.2k | MIT | Única, DSL live-editable | X11 y Wayland en deps | Propio | `code_editor` en repo | Total (shaders propios) | Débil **(sin verificar)** | Pobres | **Muy baja** |
| **Tauri** (baseline no nativo) | 3.0.0-alpha.5 (2.x estable) | 111.9k | Apache-2.0/MIT | Muy madura | WebKitGTK; en Linux tiene problemas conocidos de rendimiento y Wayland **(sin verificar estado 2026)** | El del navegador (perfecto) | CodeMirror 6 + vim, todo resuelto | DOM/Canvas | La del navegador | Excelentes | **Máxima** |

### Notas por opción

**GPUI.** El estado de publicación es el problema central. La discusión #55271 confirma que el `gpui` actual **no se puede publicar ni por Zed**: `gpui_macros`, `gpui_shared_string` y `gpui_util` tienen `publish = false`, y después del *platform split* (PR #49277) la 0.2.2 de crates.io es otra crate estructuralmente distinta. El ecosistema lo resolvió por fuera:
- **`gpui-pre`** (Longbridge): republica snapshots de upstream (`zed@93d7456`, 0.3.9, 2026-10-12). Es de lo que depende **gpui-component/gpui-kit 0.7.3**, que sí está en crates.io (75+ componentes, editor de 200K líneas con tree-sitter y LSP, Markdown/HTML, virtual list de alturas variables, soporte wasm). Longbridge lo usa en su app de trading Longbridge Pro y tiene además `gpui-fast` (optimizaciones de render y layout).
- **`gpui-ce`**: fork comunitario, "mostly API compatible with upstream, but that is changing", Apache-2.0, sincroniza con upstream, gobernanza "still forming". Política "Made by Humans" (sigue la política de IA de la Rust Foundation): ojo si piensas **contribuir** código vibe-codeado a ese fork.
- **`gpui-unofficial`**: espejo automático de upstream; según gpui-archipelago "needs a maintainer".
- **gpui-archipelago.github.io**: hub comunitario que mapea los forks (también `kael`, `gpui-box`, WGPUI) y se describe como "still extremely alpha".
- Licencia: `gpui` es Apache-2.0, pero **el `editor` y el `vim` de Zed son GPL** **(sin verificar crate por crate)**. Si "nota" no es GPL, no se puede copiar código de ahí, solo estudiarlo.

**iced.** Arquitectura Elm (state, message, update, view). Es predecible y a la IA le sale bien. 0.14 añadió IME, hot reloading y time-travel debugging. El `text_editor` sirve para formularios o un editor sencillo; para un vault enorme con vim y bloques renderizados en línea vas a escribir tu propio widget sobre cosmic-text. libcosmic (COSMIC) es un fork o extensión de iced con widgets extra. Tiene releases espaciadas (la última hace ~10 meses).

**gtk4 + libadwaita.** Es lo más "nativo Linux" de verdad: IME, a11y, portals y HiDPI ya resueltos, y GtkSourceView da un editor de código. Las contras para tus prioridades: el look GNOME es difícil de "pulir" a tu gusto, macOS queda de segunda, el arranque es algo más lento que GPUI o iced **(sin verificar cifras)** y el modo vim habría que implementarlo a mano sobre `GtkTextView`, peleando con su modelo de eventos.

**egui / Slint.** Buenos en lo suyo, malos para un editor de documentos con prosa larga, wrapping, bidi y rich text en línea. Quedan descartados como base.

**Xilem / Floem / Freya / Makepad.** Técnicamente interesantes (Parley y Vello son de lo mejor en texto y 2D), pero con poca masa crítica, releases irregulares (Floem no publica desde 2024) y poquísimo material para que la IA genere código correcto. Freya tiene el editor de texto más completo de este grupo; vale la pena vigilarlo.

**Tauri.** Es el baseline: con CodeMirror 6 + `@replit/codemirror-vim` tendrías el 80% del editor en un fin de semana. No cumple "nativo, arranque instantáneo, 120 fps en cualquier compositor", y WebKitGTK en Linux es el punto débil.

---

## 3. Recomendación

### Elegido: GPUI vía GPUI Kit (`gpui-kit` / `gpui-component` sobre `gpui-pre`)

Por qué encaja con las prioridades de "nota":
1. **Rendimiento probado justo en este dominio.** Zed es un editor que arranca rápido, renderiza a la tasa del monitor y abre archivos enormes. Ningún otro toolkit de la lista tiene una app de esa escala en Linux.
2. **Linux de primera.** Wayland y X11 propios, IME por XIM y Wayland, portals, keyring. Ahora sobre wgpu, el backend más probado del ecosistema.
3. **Keyboard-driven de serie.** El sistema de *actions* + *keymap* + *key contexts* de GPUI es exactamente la base para un modo vim y un command palette. Zed ya tiene un modo vim completo como referencia de diseño (aunque no se pueda copiar si es GPL).
4. **Bloques ya hechos.** Editor de código con tree-sitter, render de Markdown, virtual lists, temas, 75+ componentes.
5. **Licencia Apache-2.0** (gpui y gpui-component).
6. **macOS después**: GPUI tiene backend Metal maduro (es la plataforma original de Zed).

Cómo usarlo para reducir el riesgo:
- Depender de **`gpui-kit` fijado a una versión exacta** (`=0.7.3`), que trae su `gpui-pre` fijado. No depender de git HEAD de Zed.
- Meter en el repo una carpeta `docs/vendor/` con ejemplos de gpui-component y notas de API, más un `CLAUDE.md` / `AGENTS.md` con patrones validados. La IA va a inventar APIs de GPUI si no tiene contexto.
- Arquitectura en capas: **core sin GUI** (vault, índice, parser Markdown, buffer/rope, máquina de estados vim, keymap) en crates propias, probadas con tests. GPUI solo como capa de vista. Si GPUI se complica, el core se migra a iced sin reescribirlo.
- Hacer un spike de 1–2 semanas antes de decidir del todo: ventana + editor de un archivo de 50 MB + scroll a 120 Hz + IME (acentos con dead keys, Fcitx5) + portapapeles primario, en GNOME, KDE y un wlroots (Hyprland/Sway).

### Alternativa: iced (+ cosmic-text, opcionalmente libcosmic)

- Crates.io al día, licencia MIT, comunidad grande y la IA lo maneja bien.
- COSMIC demuestra que funciona como escritorio completo en Linux/Wayland.
- El costo es construir el widget editor propio (virtualización, wrapping, decoraciones, vim) sobre `cosmic-text::Editor`. Es mucho trabajo de bajo nivel, justo el que GPUI y gpui-component ya traen hecho.

Mención: **gtk4 + libadwaita + sourceview5** si en algún momento la prioridad pasa a ser "integración GNOME perfecta y accesibilidad" por encima del look propio y el rendimiento extremo.

---

## 4. Riesgos

| Riesgo | Prob. | Impacto | Mitigación |
|---|---|---|---|
| Zed no publica GPUI y cambia la API sin aviso; los forks (`gpui-pre`, `gpui-ce`) se dividen o se abandonan | Alta | Alto | Versión exacta fijada, actualizar a propósito, core desacoplado de la UI |
| La IA genera código GPUI incorrecto (poco training data, API que cambió entre 2024 y 2026) | Alta | Medio | Docs vendorizados, ejemplos que compilan como "golden", `cargo check` en el loop del agente |
| Confundir el gpui 0.2.2 viejo con el actual (tanto la IA como los tutoriales) | Media | Medio | Fijarlo en AGENTS.md; prohibir `gpui = "0.2"` |
| GPL del editor y vim de Zed: copiar código contamina la licencia | Media | Alto | Solo leer como referencia; implementar vim propio (o evaluar crates de vim independientes) |
| Accesibilidad inmadura en GPUI | Media | Medio (OSS) | Aceptarlo en v1; seguir el trabajo de accesskit en Zed |
| Bugs específicos de compositor (fractional scaling, IME en Hyprland, NVIDIA) | Media | Medio | Matriz de pruebas en el spike; fallback GLES de wgpu |
| Con iced (si cambias): releases lentas, widget editor a construir desde cero | — | Alto en esfuerzo | Medirlo en el mismo spike |
| Build time y tamaño de binario de GPUI (deps grandes) | Media | Bajo | `mold`/`wild` como linker, perfiles dev optimizados para deps |

## Fuentes consultadas (2026-10-15)

- github.com/zed-industries/zed/discussions/55271 (estado de publicación de gpui)
- `crates/gpui_linux/Cargo.toml` y `crates/gpui_wgpu/Cargo.toml` en zed `main`; PRs #7343 (Linux via Blade, 2024-02), #46758 (wgpu sustituye a Blade, 2026-01)
- gpui-archipelago.github.io; github.com/gpui-ce/gpui-ce; github.com/longbridge/gpui-component (gpui-kit)
- API de crates.io para versiones y fechas; API de GitHub para estrellas, licencias y actividad
- READMEs de iced (+ CHANGELOG 0.14), Slint, Xilem, Floem, Freya y Makepad
