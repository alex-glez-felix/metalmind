# 01 — Panorama: apps de notas markdown que se solapan con "nota"

Fecha: 2026-10-15. Objetivo de nota: app de notas markdown open-source, nativa (Linux primero), en Rust, centrada en (1) rendimiento, (2) UX y (3) edición modal vim y atajos de teclado en toda la app. La compatibilidad con vaults de Obsidian no es requisito: se adopta su sintaxis (`[[wikilinks]]`, `#tags`, frontmatter) solo donde sea la mejor opción, no por obligación.

## Cómo se verificó

- Los metadatos (stars, `pushed_at`, licencia, releases) salen de la API de GitHub (`gh api`) y de la de GitLab el 2026-10-15. Las stars son aproximadas.
- Las dependencias salen de leer el `Cargo.toml` o el `package.json` de cada repo. Las funciones salen del README, de la doc del repo o de la web oficial.
- El buscador web (DuckDuckGo) falló en todos los intentos. Por eso el descubrimiento se hizo con `gh search repos` y con la búsqueda del foro users.rust-lang.org. Puede que falten proyectos que no están en GitHub.
- Lo marcado "(sin verificar)" viene de memoria o de una fuente secundaria.

---

## 1. Rust nativo con GUI (lo más cercano a nota)

### Velotype — el más cercano en stack
- **URL:** https://github.com/manyougz/velotype · hilo en el foro: https://users.rust-lang.org/t/140046
- **Stack:** Rust + **GPUI 0.2** (el framework de Zed, con render por GPU) y tree-sitter para el resaltado. No usa WebView.
- **Licencia:** Apache-2.0 según `Cargo.toml` (GitHub la muestra como "Other").
- **Actividad:** unas 600 stars. Repo creado el 2026-05-09, último push el 2026-10-15. Release v0.7.5 del 2026-10-13. Es muy activo.
- **Vim:** ninguno. La búsqueda de código "vim" en el repo no devuelve resultados.
- **Compatibilidad con Obsidian:** no hay wikilinks, backlinks ni grafo. Es un editor de documentos con "Workspace Mode" (carpeta + outline), no una herramienta PKM.
- **Lo bueno:** WYSIWYG por bloques con opción de editar la fuente, serialización canónica a markdown, themes con tokens y un binario único. El autor dice que GPUI 0.2 no trae **IME ni menús nativos** y que tuvo que implementarlos él. Eso explica unas 37k LOC de Rust en mayo de 2026. Es el mejor ejemplo vivo de qué cuesta hacer un editor de texto rico sobre GPUI fuera de Zed.
- **Lo que falta:** vim, el modelo PKM (links, backlinks, búsqueda en el vault) y una paleta de comandos para manejarlo todo con teclado.
- **Derivado:** https://github.com/yuWorm/gpui-markdeditor extrae el núcleo de edición de Velotype como componente reutilizable (2 stars, julio de 2026).

### Ferrite
- **URL:** https://getferrite.dev · https://github.com/OlaProeis/Ferrite
- **Stack:** Rust + **egui/eframe 0.34**, `ropey` y `comrak`. Usa un motor de edición propio con virtual scrolling: según el README, un archivo de 80 MB ocupa unos 80 MB de RAM.
- **Licencia:** MIT.
- **Actividad:** unas 1.8k stars. Repo creado en diciembre de 2025, último push el 2026-10-08. Release v0.3.0 del 2026-05-22; la v0.3.1 está anunciada.
- **Vim:** **opcional y básico.** Viene desactivado por defecto. Tiene modos Normal, Insert, Visual y V-Line con motions básicos (`hjkl w b e 0 $ gg G`, `dd yy x D`, `o/O`). Los atajos de egui (`Ctrl+C`/`Ctrl+V`) no los intercepta. Fuente: `docs/technical/editor/vim-mode.md` y `src/editor/ferrite/vim.rs`. No hay gramática de operadores con text objects completa (sin verificar en el código; la tabla de la doc es corta). Vim **solo funciona dentro del editor**.
- **Compatibilidad con Obsidian:** la web anuncia wikilinks y backlinks; el README no los menciona (sin verificar en profundidad). Tiene Workspace Mode con árbol de archivos, `Ctrl+P` y búsqueda en todos los archivos. No hay grafo ni se menciona `.obsidian`.
- **Lo bueno:** paquetes .deb, .rpm, AUR y tar.gz. Tiene Mermaid nativo, export a PDF/HTML y bloques de código ejecutables. **Está desarrollado con un flujo de IA documentado** (`docs/ai-workflow/`, PRDs por versión, implementación con "Claude Opus"). Es un precedente directo de lo que se quiere hacer con nota.
- **Lo que falta:** es un editor de texto generalista (también abre JSON, YAML y TOML), no una herramienta PKM. En **GNOME/Wayland el teclado no funciona** y el README recomienda forzar X11. egui es de modo inmediato y no tiene shaping de texto avanzado comparable al de GPUI (sin verificar).

### Bloom — el más cercano en filosofía
- **URL:** https://github.com/hindol/Bloom
- **Stack:** Rust + **iced 0.14** (wgpu) en el crate `bloom-gui`, más `ropey`, SQLite (FTS5) y un crate `bloom-vim` propio. Workspace modular con `bloom-core`, `bloom-buffer`, `bloom-md`, `bloom-store`, `bloom-history`, `bloom-import`, `bloom-mcp` y `bloom-test-harness`.
- **Licencia:** AGPL-3.0.
- **Actividad:** **1 star y un solo contribuidor.** Último commit el 2026-04-01 y release v0.4.0-alpha del 2026-03-23. Lleva unos 6 meses parado. Los commits están co-firmados por Copilot y hay `AGENTS.md`, así que también se hizo con IA.
- **Vim:** **el más completo encontrado.** Tiene gramática `[count][operator][motion/text-object]` y modos Normal, Insert, Visual y Command. Añade text objects propios: `il/al` (links), `i#` (tags) e `ih` (headings). Incluye undo tree, which-key con `SPC`, splits de ventanas con navegación espacial, `:messages` y `:log`. La interfaz es al estilo Doom Emacs.
- **Compatibilidad con Obsidian:** **NO.** Los links son por ID (`[[page-id|display]]`) y cada página lleva `id:` en el frontmatter (`docs/FILE_FORMAT.md`). Solo trae importador de Logseq.
- **Lo bueno:** la documentación de diseño es muy buena (`ARCHITECTURE.md`, `UNIFIED_BUFFER.md`, `KEYBINDINGS.md`, `PICKER_SURFACES.md`, `WINDOW_LAYOUTS.md`). Tiene benchmarks: 10,365 páginas indexadas en 2.4 s y el ack del watcher tras guardar en menos de 1 ms. El core no hace polling.
- **Lo que falta:** comunidad, el formato de Obsidian y algo más que el alfa.

### gpui-notes (natemcintosh)
- **URL:** https://github.com/natemcintosh/gpui-notes
- **Stack:** Rust + GPUI. Es una reimplementación de un subconjunto de Logseq (outliner, journal, `[[links]]`, block refs).
- **Licencia:** ninguna declarada. **Actividad:** 2 stars, creado en abril de 2026, push del 2026-10-15. Todavía no tiene releases. **Vim:** no se menciona.
- **Interés:** ejemplo pequeño y actual de GPUI fuera de Zed, distribuido con `cargo binstall --git` porque GPUI no está en crates.io.
- Un homónimo más antiguo (https://github.com/TheMysticLynx/gpui-notes, 15 stars, 2024) está abandonado.

### Proyectos mínimos o recién nacidos (ruido, 0–2 stars)
- `Saboor-Hamedi/mindforge`: egui con IA, se anuncia como "keyboard-driven", sept–oct de 2026.
- `ArchAstro/stet`: núcleo en Rust y "vim keys", creado en oct de 2026; no se verificó la GUI.
- `zetahiveco/kbwrite`, `Novus-Corvus/Runa`, `freakkxd/Fragile-Notes`: todos se presentan como "native Obsidian alternative in Rust" y son de oct de 2026.
- `hirodawn/lumen`: editor en egui.
- **Lectura:** cada semana sale un "Obsidian alternative in Rust" hecho con IA. Ninguno tiene tracción.

---

## 2. Rust en el core con shell web (Tauri)

| Proyecto | URL | Stack | Licencia | Actividad | Vim | Compatibilidad con Obsidian |
|---|---|---|---|---|---|---|
| **HelixNotes** | https://helixnotes.com · https://gitlab.com/ArkHost/HelixNotes | Tauri + SvelteKit; editor **TipTap** (ProseMirror) más paquetes `@codemirror/*` | AGPL-3.0 | 51 stars en GitLab; v1.3.6 del 2026-09-30, release mensual | La web no lo menciona; tampoco hay paquetes de vim en `package.json` (sin verificar más allá de eso) | `[[wikilinks]]` e importador de Obsidian |
| **Onyx** | https://github.com/onyx-notes/onyx | Tauri v2 + SolidJS + **CodeMirror 6**; core en Rust (tantivy, SQLite, grafo de links) | AGPL-3.0 | 3 stars; último commit el 2026-07-18 | No mencionado | **Abre un vault de Obsidian tal cual** e importa `.obsidian` |
| **outl** | https://outl.app · https://github.com/outlmd/outl | Outliner. Una crate `outl-core`/`outl-md` con varios frontends: TUI (vim-first), desktop (**Tauri**, beta en macOS) y móvil | MIT | unas 200 stars; v0.12.0-beta.218 del 2026-10-14, muy activo | Vim en la TUI ("vim-first, mouse-optional") | Markdown CommonMark sin IDs, `[[links]]` e importadores de Obsidian, Logseq y Roam. Su modelo es de outliner, no de documento |

- **Qué aprender:** el core de Onyx (`onyx-core` y `onyx-md`: watcher, escrituras atómicas, tantivy, extracción de wikilinks, tags y frontmatter) se parece mucho a la capa de vault que necesita nota.
- **Qué aprender:** de outl, el sync con CRDT de árbol en un sidecar que no ensucia el markdown, y la separación entre core y frontends.
- **Qué evitar:** los tres renderizan con WebView, que es justo lo que nota quiere evitar.

---

## 3. TUIs en Rust

| Proyecto | URL | Stars / actividad | Licencia | Vim | Obsidian |
|---|---|---|---|---|---|
| **emeraldian** | https://github.com/iamrohithrnair/emeraldian | unas 530 stars, creado en jul de 2026, push del 2026-09-26 | GPL-3.0 | Hay un modo vim que se activa y desactiva con `F4`; por defecto usa los atajos de Obsidian | **Sí, nativo**: lee el vault sin importar nada, entiende URIs `obsidian://`, tiene backlinks, grafo de fuerzas, Excalidraw e imágenes. Se integra con la CLI oficial de Obsidian |
| **kimun** | https://github.com/nico2sh/kimun | 60 stars, push del 2026-10-13 | MIT | Emulación vim propia o **Neovim embebido** | Wikilinks y backlinks; convive con vaults de Obsidian y Logseq |
| **oxid** | https://github.com/Ninso112/oxid | 1 star, push de may de 2026 | GPL-3.0 | Delega en `$EDITOR` | Básico |
| **scribble**, **notetui** | `Smoke516/scribble`, `KKodiac/notetui` | 0 stars | MIT / ninguna | vim-like | "Obsidian-style vaults" |
| **clin** | — | **(sin verificar)**: no se encontró en GitHub ni en el foro de Rust | | | |

- **Qué aprender:** emeraldian es la mejor referencia de **compatibilidad real con Obsidian** escrita en Rust: qué partes del formato y de `.obsidian/` hay que respetar para poder convivir con Obsidian abierto sobre la misma carpeta.
- **Qué aprender:** el modo "Neovim embebido" de kimun es una alternativa a escribir un vim propio (por ejemplo, vía `nvim --embed` y RPC).

---

## 4. LSP y editores como herramienta PKM

- **Markdown Oxide**: https://github.com/Feel-ix-343/markdown-oxide
  - Language server PKM en Rust, Apache-2.0. Unas 2.3k stars, push del 2026-10-15, v0.25.12 del 2026-06-29.
  - Pensado para vaults estilo Obsidian: wikilinks, backlinks vía references, daily notes, tags y completado. Se integra con Neovim, VSCode, Zed y Helix.
  - **Candidato a reutilizar directamente:** se puede embeber o hablarle por LSP para tener la semántica del vault gratis, o al menos estudiar su parser e indexador.
- **obsidian.nvim**: https://github.com/obsidian-nvim/obsidian.nvim
  - Lua, Apache-2.0. Unas 2.2k stars, push del 2026-10-15; es el fork mantenido.
  - Con Neovim da vim de verdad y compatibilidad con vaults. Lo que falta: render rico, imágenes y UX gráfica.
- **zk**: https://github.com/zk-org/zk
  - Go, GPL-3.0. Unas 2.8k stars, activo. CLI y LSP para Zettelkasten. No es una GUI.
- **Zed**: https://github.com/zed-industries/zed
  - Rust + GPUI. Unas 92k stars, v1.25.0-pre del 2026-10-14. Tiene modo vim maduro y preview de markdown.
  - Con Markdown Oxide como LSP sirve como "Obsidian para devs". Falta render en línea (WYSIWYG o live preview), grafo y embeds.
  - **Es la mejor implementación de vim sobre GPUI que existe** (`crates/vim`). Licencia mixta: GPL/AGPL/Apache según el crate; GPUI es Apache-2.0 (sin verificar crate por crate).
- **Helix**: https://github.com/helix-editor/helix
  - Rust, MPL-2.0. Unas 46.6k stars. Usa el modelo de edición de Kakoune (selección y luego acción), no el de vim. Es TUI.
- **Lapce**: https://github.com/lapce/lapce
  - Rust + Floem (wgpu), Apache-2.0. Unas 39k stars, último push y nightly del 2026-10-09; no tiene release estable reciente.
  - Su modo vim es nativo. No es una herramienta PKM.

---

## 5. Las apps grandes (referencia de UX y features)

| App | Stack | Licencia | Actividad | Vim | Notas |
|---|---|---|---|---|---|
| **Obsidian** | Electron + CodeMirror 6 | Propietaria (gratis) | — | Integrado: Settings → Editor → "Vim key bindings", vía `codemirror-vim` (replit, unas 480 stars, push de jul de 2026). Los mappings persistentes requieren el plugin `obsidian-vimrc-support` (unas 1.5k stars), cuyo **mantenedor busca relevo** | Vim **solo en el editor**; el explorador, la paleta y el grafo no son modales (de memoria, sin verificar en la doc actual porque la URL de la ayuda dio 404). Ya existe una CLI oficial (https://obsidian.md/cli, citada por emeraldian) |
| **Logseq** | Clojure(Script) + Electron | AGPL-3.0 | unas 45k stars; app 2.0.x en beta (2.0.2 del 2026-10-07), con la versión DB | Sin vim nativo según la búsqueda de código; solo atajos propios y plugins (sin verificar) | Es un outliner. La versión DB se aleja del markdown plano |
| **SilverBullet** | TypeScript + Lua, web/PWA | MIT | unas 6.3k stars, 2.12.1 del 2026-10-12 | **Integrado** (`plugs/editor/vim.ts`, `docs/Vim.md`) | Corre en servidor; es "malleable" con Lua |
| **Zettlr** | Electron + CodeMirror | GPL-3.0 | unas 13.8k stars, v4.8.0 del 2026-09-18 | Integrado (`plugins/vim-mode.ts`) | Orientado a publicación académica |
| **Joplin** | Electron / React Native | AGPL-3.0 (NOASSERTION en GitHub) | unas 57k stars, v3.7.21 del 2026-09-25 | Vim en el editor markdown CodeMirror (`setupVim.ts`) | Usa su propia base de datos y sync; no es vault de archivos |
| **AppFlowy** | Flutter + Rust | AGPL-3.0 | unas 77k stars | No encontrado | Clon de Notion; no usa markdown plano |
| **Anytype** | Electron (TS) + core en Go | Propia, "source-available" (sin verificar) | unas 9k stars | No encontrado | Basado en objetos; no es vault markdown |
| **VNote** | C++ + Qt (nativo) | LGPL-3.0 | unas 13k stars, activo | Tiene modo vim según su documentación conocida (sin verificar en el código; la búsqueda topó con el rate limit) | Ejemplo de app nativa, no web, que lleva una década viva |

---

## 6. Frameworks de GUI relevantes (estado a oct-2026)

- **GPUI**: el framework de Zed. En crates.io está la 0.2, que usa Velotype; la versión de Zed va por git. Le faltan piezas: IME y menús nativos, según el autor de Velotype.
- **gpui-kit**: https://github.com/longbridge/gpui-kit (antes `gpui-component`, unas 17.6k stars, activo).
  - Incluye un **Code Editor "estable a 200K líneas"** con tree-sitter y LSP, render nativo de Markdown y HTML, y tests de integración de UI en ventanas headless.
  - **Es probablemente la pieza que más acelera nota.**
- **iced**: unas 31.8k stars, activo; es la base de Bloom.
- **egui**: unas 31.5k stars, activo; es la base de Ferrite. Tiene problemas de teclado en GNOME/Wayland, según el README de Ferrite.
- **Floem**: el de Lapce; no se revisó a fondo.

---

## 7. Conclusión: ¿hay hueco?

**Sí, hay hueco.** Nadie cumple a la vez los cuatro requisitos: Rust nativo, render por GPU, vim modal en toda la app y compatibilidad con vaults de Obsidian.

| Requisito | Velotype | Ferrite | Bloom | emeraldian | Onyx / HelixNotes | Zed + Oxide |
|---|---|---|---|---|---|---|
| Rust nativo con GPU | ✅ GPUI | ✅ egui | ✅ iced | ❌ TUI | ❌ WebView | ✅ GPUI |
| Vim de primera clase | ❌ | ⚠️ básico, solo en el editor | ✅ completo | ⚠️ opcional | ❌ | ✅ en el editor, ⚠️ fuera |
| Vim o teclado "en todo" (paneles, pickers, árbol) | ❌ | ❌ | ✅ (which-key, splits) | ⚠️ | ❌ | ⚠️ |
| Compatible con vaults de Obsidian | ❌ | ⚠️ wikilinks | ❌ links por ID | ✅ | ✅ | ✅ vía LSP |
| Render rico (live preview o WYSIWYG) | ✅ | ✅ | ⚠️ resaltado semántico | ⚠️ TUI | ✅ | ❌ |
| Tracción | media | media-alta | nula | media | baja | alta (pero no es PKM) |

Lo más cercano es **Bloom**: tiene la filosofía y el vim, pero está alfa, parado y usa un formato propio. Velotype tiene el stack y el render, pero no tiene vim ni PKM. emeraldian tiene la compatibilidad con Obsidian, pero es terminal. El hueco concreto es la **combinación**: el render rico de Velotype, el vim y el teclado de Bloom extendidos a toda la app, y la fidelidad al vault de emeraldian y Onyx.

**Riesgo:** abundan los clones hechos con IA de 0 stars. El diferenciador tiene que ser calidad medible (latencia, vault de más de 10k notas) y la fidelidad del vim. La lista de features no basta.

## 8. Qué código fuente estudiar (top 3)

1. **Bloom** (`hindol/Bloom`): para el **modelo modal y el teclado en toda la app**.
   - Un crate `bloom-vim` separado del buffer.
   - Text objects específicos de markdown y PKM.
   - Which-key, pickers unificados, splits, undo tree y un harness de tests.
   - Sus `docs/*.md` (ARCHITECTURE, UNIFIED_BUFFER, KEYBINDINGS, PICKER_SURFACES) sirven casi como plantilla de ADRs.
   - Ojo: la licencia es AGPL. Si nota va a usar MIT o Apache, solo se pueden estudiar sus ideas, no copiar código.
2. **Velotype** (`manyougz/velotype`) **y gpui-kit**: para **render rico sobre GPUI en Linux**.
   - Velotype muestra cómo hacer bloques WYSIWYG con fallback a la fuente, qué hubo que implementar a mano (IME, menús) y cómo serializa a markdown.
   - gpui-kit aporta el editor de código a 200K líneas y el render de markdown.
   - Las dos son Apache-2.0 o compatibles, así que se puede reutilizar código.
3. **Zed `crates/vim`** más **Markdown Oxide**, o, como alternativa, **emeraldian** para fidelidad a Obsidian.
   - El vim de Zed es la implementación modal más madura sobre GPUI; conviene mirar cómo modela operadores y motions sobre un editor multibuffer. Su licencia es GPL, así que solo se pueden estudiar las ideas.
   - Markdown Oxide resuelve la semántica del vault de Obsidian en Rust (links, backlinks, daily notes) y se puede usar como librería o como LSP embebido.
   - emeraldian enseña qué partes de `.obsidian/` y del formato hay que respetar para convivir con Obsidian.

Lecturas secundarias:
- El flujo `docs/ai-workflow/` de Ferrite: PRDs por versión y desarrollo con Claude. Es un precedente de "vibe-coded" exitoso con unas 1.8k stars.
- `onyx-core` y `onyx-md` como capa de vault: watcher, escrituras atómicas y tantivy.
