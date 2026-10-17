# 03 — Arquitectura del núcleo

> Investigación del 2026-10-15. Independiente del framework de GUI (GPUI o iced, ver [02](02-gui-stack.md)).
> Las versiones y descargas salen de la API de crates.io consultada ese día. Lo que no pude comprobar va marcado **(sin verificar)**.

## TL;DR

- **Buffer**: `ropey` 1.6 (estándar de facto, usado por Helix). Undo/redo y multi-cursor son **nuestros**, encima del rope, no del crate.
- **Markdown**: dos parsers con dos trabajos. `tree-sitter-md` (incremental, con rangos de bytes) para el **live preview** y el resaltado mientras escribes; `pulldown-cmark` 0.13 (trae `ENABLE_WIKILINKS`) para **índice y export**. `comrak` si algún día hace falta un AST completo con round-trip.
- **Vim**: motor propio, puro, en su crate (`nota-vim`), sin dependencia de la GUI. Arquitectura copiada de Zed: **keymap declarativo + contextos + acciones con nombre**. Así vim aplica igual al buffer, al árbol de archivos, a la paleta y al picker.
- **Vault**: `ignore` (walk paralelo) + `notify` 8 con `notify-debouncer-full` + índice de links **en memoria** (fuente de verdad = los `.md`) con caché opcional en `redb`. `nucleo` para el fuzzy finder, `tantivy` para la búsqueda full-text (después del MVP).
- **Hilos**: la UI nunca bloquea. Un hilo de UI, un pool de fondo (el executor del framework o `rayon`), canales (`crossbeam-channel`/`flume`). Escrituras atómicas con `tempfile::NamedTempFile::persist`.
- **Repo**: cargo workspace con ~7 crates pequeños; `insta` + `proptest` como especificación ejecutable; `cargo-dist` para tarballs y GitHub Releases. AppImage/Flatpak/AUR van por fuera de cargo-dist.

---

## 1. Buffer de texto

### Opciones

| Crate | Versión (15-oct-2026) | Descargas recientes | Notas |
|---|---|---|---|
| `ropey` | 1.6.1 (ago-2025) | 3.7 M | B-tree rope, indexado por char/byte/línea, `RopeSlice`, chunks iterables. Lo usa Helix. API estable y documentada; los modelos de IA la conocen bien. |
| `crop` | 0.4.3 (abr-2025) | 188 k | B-tree rope, indexado por **byte**, clones O(1), pensado para editores colaborativos. Más joven, menos ejemplos. |
| `xi-rope` | 0.3.0 (**2019**) | 6 k | Abandonado. El fork vivo es `lapce-xi-rope` 0.4.0 (dic-2025), que usa Lapce/Floem. |
| `zed-sum-tree` | 0.2.0 (oct-2025) | 271 k | El `sum_tree` de Zed publicado en crates.io. El **rope** de Zed (`crates/rope`) y `text` (CRDT, buffer con anchors) **no** están publicados por separado; viven en el monorepo de Zed. |
| `jumprope` | 1.1.2 (2023) | 24 k | Skip list, muy rápido para inserciones, menos funciones de indexado por línea. |

### Recomendación

**`ropey`.** Para notas (archivos de KB, rara vez MB) cualquiera sobra en rendimiento; lo que decide es madurez, indexado por línea (vim lo necesita a cada tecla) y que los agentes generen código correcto a la primera. Si usamos GPUI, vale la pena estudiar el modelo de Zed (`sum_tree` con *summaries* agregados: líneas, UTF-16, etc.) pero **no** vendorearlo en M0.

Envolver el rope en un tipo propio para no casarnos con él:

```rust
pub struct Buffer { rope: Rope, history: History, version: u64 }
pub struct ByteOffset(usize);       // newtypes: nunca usize suelto
pub struct Point { row: u32, col: u32 } // col en bytes de la línea
```

Regla: **todo offset interno en bytes** (igual que tree-sitter y pulldown-cmark), conversión a chars/graphemes solo en el borde (render, movimiento de cursor con `unicode-segmentation`, ancho con `unicode-width`).

### Undo/redo

Ningún rope trae historial. Patrón que usan Helix y Zed:

- Toda mutación es un `Transaction { edits: Vec<Edit>, selections_before, selections_after }`, con `Edit { range: Range<ByteOffset>, text: SmolStr }`.
- `History` guarda transacciones invertibles. Undo = aplicar la inversa. Agrupar por "unidad vim": todo lo tecleado entre entrar y salir de insert es **una** transacción (así `u` deshace como en vim).
- **Árbol, no pilas** (implementado): como el undotree de vim, editar después de deshacer abre una rama nueva y la vieja sigue alcanzable. Los nodos viven en un arena (`Vec` + índice tipado `HistorySeq`, sin `Rc<RefCell>`): la raíz es el texto inicial y cada nodo guarda su padre, la transacción que lleva a él y sus hijos ordenados por visita. Se crean en orden y nunca se borran, así que el índice *es* el número de secuencia (`changenr()` en vim) y un padre siempre es menor que sus hijos.
  - `undo` = ir al padre; `redo` = ir al hijo visitado más recientemente.
  - `earlier(n)` / `later(n)` (`g-` / `g+`) van al estado con número `actual ∓ n`: suben deshaciendo hasta el ancestro común y bajan rehaciendo. El ancestro común sale de subir siempre el mayor de los dos números.
  - Cada nodo guarda ediciones invertibles, no un snapshot del rope: recorrer el árbol solo toca lo que cambió, y es lo que serializaría un futuro `undofile`.
- El mismo `Edit` alimenta `tree_sitter::InputEdit` → reparse incremental. Un solo tipo de edición para todo.

### Multi-cursor

`Selection { anchor: ByteOffset, head: ByteOffset }` y `Selections(SmallVec<[Selection; 1]>)`, siempre ordenadas y fusionadas. Cada operador vim se aplica por selección, en orden inverso de offset (para no invalidar las siguientes), dentro de una sola transacción. Es el modelo de Helix (selección como primitiva) y de Zed. No es MVP, pero **diseñar `Selections` como vector desde M1** evita reescribir el motor vim después.

---

## 2. Parsing de Markdown

### Opciones

| Crate | Versión | Incremental | Rangos de fuente | Obsidian | Uso ideal |
|---|---|---|---|---|---|
| `pulldown-cmark` | 0.13.4 | No | Sí (`into_offset_iter`) | **`ENABLE_WIKILINKS`** (`[[a]]`, `[[a\|b]]`, comprobado en 0.13.0), `ENABLE_YAML_STYLE_METADATA_BLOCKS`, `ENABLE_MATH`, `ENABLE_GFM` (callouts tipo `> [!NOTE]`), `ENABLE_HIGHLIGHT` (`==x==`) | Indexar el vault y exportar a HTML. Muy rápido, streaming. |
| `comrak` | 0.56.0 (oct-2026) | No | Sí (`sourcepos`) | `wikilinks_title_after_pipe` / `_before_pipe`, `front_matter_delimiter`, `alerts`, `math_dollars`, `highlight` | AST mutable y round-trip markdown→markdown (refactors, renombrar links). |
| `markdown` (markdown-rs) | 1.0.0 (abr-2025) | No | Sí (mdast con posiciones) | MDX, frontmatter, math; wikilinks no **(sin verificar)** | Spec-compliance estricta. Poca actividad en 2026. |
| `tree-sitter` + `tree-sitter-md` | 0.27.1 / 0.5.3 | **Sí** | Sí (byte + punto) | Nada nativo; dos gramáticas (block + inline) | **Editor**: resaltado y live preview a cada tecla. |

> Ojo: `tree-sitter-markdown` en crates.io es otro crate, de 2021, abandonado. El bueno es **`tree-sitter-md`** (tree-sitter-grammars/tree-sitter-markdown).

### Dos parsers, dos trabajos

1. **Editor / live preview → tree-sitter-md.** Reparse incremental en microsegundos tras cada `InputEdit`; da nodos con rangos exactos para decidir qué "ocultar" (los `**`, `#`, `[[ ]]`) cuando el cursor no está en esa línea/bloque — exactamente lo que hace el live preview de Obsidian (CodeMirror 6). La gramática no sabe de wikilinks, tags ni embeds; dos caminos:
   - **Recomendado para MVP**: post-pase propio sobre los nodos `inline` con un scanner pequeño (regex-automata o a mano) que detecte `[[...]]`, `![[...]]`, `#tag`, `^block-id`, `%%comentario%%`. Es código puro, fácil de testear con `insta`.
   - Más tarde: fork de la gramática inline con esos nodos (requiere C + `tree-sitter generate`; peor para vibe-coding).
2. **Índice y export → pulldown-cmark** con `ENABLE_WIKILINKS | ENABLE_YAML_STYLE_METADATA_BLOCKS | ENABLE_GFM | ENABLE_MATH | ENABLE_TABLES | ENABLE_TASKLISTS | ENABLE_FOOTNOTES | ENABLE_HIGHLIGHT`. Para indexar un vault de 10 k notas en frío, un parser de streaming sin árbol es lo más barato.

El **mismo** scanner de sintaxis Obsidian (crate `nota-syntax`) debe usarse en ambos caminos para que el editor y el índice nunca discrepen sobre qué es un link.

### Sintaxis Obsidian a cubrir

| Sintaxis | Ejemplo | Fase |
|---|---|---|
| Wikilink, alias, heading, bloque | `[[Nota]]`, `[[Nota\|alias]]`, `[[Nota#Sección]]`, `[[Nota#^id]]` | M3 |
| Embed | `![[Nota]]`, `![[img.png\|300]]`, `![[Nota#Sección]]` | M4–M5 |
| Tags | `#tag`, `#a/b/c`, `tags:` en frontmatter | M3 |
| Frontmatter YAML | `aliases`, `tags`, `cssclasses` | M3 |
| Callouts | `> [!note] Título` y plegables `> [!tip]-` | M5 |
| Highlight, comentarios | `==x==`, `%% x %%` | M4 |
| Math | `$x$`, `$$...$$` | post-MVP |
| Block IDs | `^abc123` al final de párrafo | M3 (índice) |

Resolución de wikilinks (como Obsidian): por nombre de archivo sin extensión, case-insensitive, desempate por ruta más corta / misma carpeta; también matchea `aliases` del frontmatter. Documentarlo como tabla de casos en un test `insta` — es la spec.

### Frontmatter YAML

`serde_yaml` está **deprecado** (0.9.34+deprecated). `serde_yml` tuvo polémica de calidad en 2025 **(sin verificar el estado actual)**. Opciones sanas:

- `serde_norway` 0.9.42 — fork mantenido de serde_yaml, misma API.
- `yaml-rust2` 0.13.0 (sep-2026) — parser puro sin serde, bueno para "leer lo que haya" sin fallar ante YAML raro.

Recomendación: extraer el bloque `---...---` a mano (rangos exactos para el editor), parsear con `yaml-rust2` a un `Frontmatter { aliases, tags, raw }` tolerante, y **nunca reescribir** el YAML del usuario salvo petición explícita.

---

## 3. Motor vim

### Cómo lo hacen otros

- **Zed** (`crates/vim` + `assets/keymaps/vim.json`): vim es un *addon* del editor, no un editor aparte. Las teclas se mapean a **acciones con nombre y argumentos** en JSON (`"w": "vim::NextWordStart"`, `"shift-w": ["vim::NextWordStart", {"ignore_punctuation": true}]`, `"i": ["vim::PushObject", {"around": false}]`). Cada binding vive en un **contexto** con predicados (`"VimControl && !menu"`, `"ProjectPanel && not_editing"`, `"Editor && vim_mode == normal"`, `"Picker > Editor"`). El estado del operador pendiente se expone como contexto (`vim_operator == ys`), así un operador es solo "una tecla que cambia el contexto". Multi-tecla con espacios (`"g e"`). El mismo archivo mapea el panel de proyecto, el outline, el panel git, etc.: **así es como "vim en toda la UI" sale gratis**. Comprobado leyendo `vim.json` en `main` hoy.
- **Helix** (`helix-term/src/keymap/default.rs`): `HashMap<Mode, KeyTrie>`; un trie de teclas a comandos con nombre (`move_next_word_start`). Modelo selección→acción (kakoune), no verbo→objeto. Bueno como referencia de trie y de comandos tipados; **no** es vim. `helix-core` en crates.io es un placeholder 0.0.0; no se consume como librería.
- **Lapce / Floem**: `floem-editor-core` 0.2.0 (nov-2024) incluye modo modal; sin releases desde 2024.
- **`modalkit`** 0.0.31 (oct-2026, ulyssa): librería real para apps modales (la usa el cliente Matrix `iamb`). Trae keybindings vim/emacs, registros, macros, histories; tipos en `editor-types`, máquina de teclas en `keybindings` 0.0.4. Pensada para TUI (`modalkit-ratatui`), pero el núcleo es agnóstico. Pre-1.0, API cambiante, 4 k descargas recientes.
- **`vim-like`**: **no existe** en crates.io.

### Recomendación: motor propio, inspirado en Zed, con modalkit como lectura

Razón: vim-everywhere es la tercera prioridad del proyecto y el corazón de su UX; depender de un crate 0.0.x de un solo mantenedor es riesgo. Pero leer `editor-types` de modalkit antes de diseñar nuestros tipos ahorra semanas.

Diseño en tres capas:

```
KeyEvent ─▶ [Keymap: trie + contextos] ─▶ Action (nombre + args, serde)
                                            │
                     ┌──────────────────────┴───────────────┐
              acciones globales                      acciones de vim
       (workspace::open_finder, pane::split)   [VimState: modo, count, registro,
                                                operador pendiente] ─▶ Command
                                                                         │
                                                       Command { op, motion|textobj, count }
                                                                         │
                                                       aplicar sobre &Buffer + Selections
                                                       ─▶ Transaction  (puro, testeable)
```

Tipos núcleo (`nota-vim`, sin GUI):

```rust
enum Mode { Normal, Insert, Visual(VisualKind), OperatorPending(Operator), CommandLine, Replace }
enum Operator { Delete, Change, Yank, Indent, Outdent, Lowercase, Uppercase, ToggleCase, Format, Surround }
enum Motion { Left, Right, Up, Down, WordStart{big: bool}, WordEnd{big}, LineStart, FirstNonBlank,
              LineEnd, FindChar{c, before, forward}, DocumentStart, DocumentEnd, Paragraph{forward},
              SearchNext{forward}, Matching, GotoLine(u32), /* markdown: */ NextHeading, PrevHeading, NextLink }
enum TextObject { Word{big}, Sentence, Paragraph, Quote(char), Bracket(char), Tag,
                  /* markdown: */ Link, CodeBlock, ListItem, Section }
struct Register(char);
```

- **Motion → rango** es una función pura `fn resolve(motion, &Buffer, Selection, count) -> Range`. Cientos de casos → test de tabla con `insta`.
- **Operator × rango → Transaction** es otra función pura.
- **Dot-repeat** (`.`) = guardar el último `Command` + texto insertado. **Macros** (`q`) = guardar la secuencia de `KeyEvent`s y re-alimentar el keymap.
- **Text objects markdown** (`il`/`al` link, `ih`/`ah` sección por heading, `ic` bloque de código) salen del árbol tree-sitter: diferenciador barato frente a Obsidian.

### Estado (M0a, 2026-10-15)

- `nota-vim` existe como máquina de estados pura: Normal, Insert y operador pendiente; motions `h j k l w W b B e E 0 ^ $ gg G` con counts; `d c y`, `dd cc yy`, `x`, `p P`, `u`, `ctrl-r`, `g- g+` (historial en árbol); text objects `iw aw iW aW i( a( i" a"`; registro sin nombre; multi-cursor en todas las operaciones.
- El keymap vive por ahora dentro de `nota-vim`. El default es una tabla tipada `(contexto, teclas, acción)`, infalible y validada por un test (sin secuencias ambiguas ni duplicadas); el parseo de strings, que sí puede fallar, llegará con la config del usuario. Se moverá a `nota-keymap` cuando haya más de un consumidor.
- **Referencia de comportamiento = `nvim --clean` 0.12.6 (última estable, 2026-10-09) con `noautoindent`.** Importa porque los defaults de nvim difieren de vim: `nostartofline` (`dd`, `G`, `gg` conservan la columna), `backspace=indent,eol,start`, `cpoptions=aABceFs_`. Las motions de palabra y los text objects portan la lógica de `textobject.c` (`fwd_word`, `current_word`, `current_block`, `current_quote`) para que los bordes coincidan.
- Cada caso de la tabla (`crates/nota-vim/tests/vim.rs`) corre en el motor y en nvim (`tests/support/oracle.lua`). El oráculo exige esa versión exacta y el CI la instala con checksum. Al pasar de 0.10.4 a 0.12.6 los 207 casos dieron lo mismo.

### Vim en toda la UI

El truco de Zed: **vim no es especial**; es un conjunto de bindings en contextos.

- Cada vista declara su contexto (`Editor`, `FileTree`, `Picker`, `CommandPalette`, `SearchBar`, `Backlinks`) y sus predicados (`mode == normal`, `editing`, `focused`).
- `FileTree`: `j/k` mover, `h/l` colapsar/expandir, `gg/G`, `/` filtrar, `a` crear, `r` renombrar, `dd` borrar (con confirmación), `o` abrir en split.
- `Picker`/paleta: el input es un `Editor` de una línea (contexto `Picker > Editor`) en insert por defecto; `Esc` pasa a normal y `j/k` mueve la lista.
- `:` abre una línea de comandos que parsea a las **mismas** acciones (`:w` → `buffer::save`, `:e nota` → `workspace::open`, `:vsp`, `:q`, `:%s/a/b/g`). Un solo registro de acciones = paleta, `:` y keymap comparten nombres. Los nombres de `nota` van en snake_case (`vim::next_word_start`), a diferencia del CamelCase de Zed.
- Leader: `"space f": "workspace::open_finder"` en el contexto `VimControl && !editing`; configurable por TOML.
- Ventanas: `ctrl-w h/j/k/l/v/s/q` como acciones de `pane::`.

Keymap del usuario en **TOML** (coherente con el resto de la config) que se fusiona sobre el default embebido:

```toml
[[keymap]]
context = "Editor && vim_mode == normal"
bindings = { "space f" = "workspace::open_finder", "g d" = "editor::follow_link" }
```

---

## 4. Indexado del vault

| Pieza | Crate | Versión | Notas |
|---|---|---|---|
| Recorrido | `ignore` | 0.4.33 | `WalkBuilder::build_parallel()`, respeta `.gitignore`; añadir filtro propio para `.obsidian/`, `.trash/`. |
| Vigilar cambios | `notify` + `notify-debouncer-full` | 8.2.0 / 0.7.0 | inotify en Linux. El debouncer junta rename-from/rename-to. Ojo con el límite `fs.inotify.max_user_watches` en vaults enormes **(sin verificar umbral)**. |
| Fuzzy finder | `nucleo` (o `nucleo-matcher`) | 0.5.0 / 0.3.1 | El de Helix. Matching en hilos de fondo, inyección incremental de ítems, snapshot sin bloquear la UI. Sin release desde 2024 pero estable. Alternativa: `frizbee` 0.13 (SIMD, el de blink.cmp). |
| Full-text | `tantivy` | 0.26.2 | Lucene en Rust; índice en disco, BM25, frases, fuzzy. Post-MVP: para "buscar en el vault" basta un grep paralelo (`grep-searcher`/`regex`) hasta ~10 k notas **(sin verificar umbral)**. |
| Persistencia de caché | `redb` 4.4.0 o `rusqlite` 0.40.2 | — | Ver abajo. |
| Grafo | `petgraph` 0.8.3 | — | Solo si hace falta la vista de grafo / algoritmos; para backlinks bastan dos `HashMap`. |

### Modelo de datos del índice

```rust
struct NoteId(u32);                      // interned, estable en la sesión
struct NoteMeta { path, mtime, size, hash: u64, title, aliases, tags, headings, block_ids }
struct LinkIndex {
    outgoing: HashMap<NoteId, Vec<Link>>,          // Link { target: LinkTarget, range, kind: Link|Embed }
    incoming: HashMap<NoteId, Vec<(NoteId, Range)>>, // backlinks resueltos
    unresolved: HashMap<SmolStr, Vec<NoteId>>,     // [[Nota que no existe]] — Obsidian los muestra
    by_name: HashMap<SmolStr /*lowercase stem o alias*/, SmallVec<[NoteId; 1]>>,
    tags: BTreeMap<SmolStr, Vec<NoteId>>,
}
```

- **Fuente de verdad = los `.md` en disco.** El índice es derivado y siempre reconstruible. Nada de base de datos propietaria (eso es exactamente lo que hace a Obsidian compatible con git, sync, etc.).
- **En memoria** para el MVP: 10 k notas × ~20 links ≈ unos MB. Rápido y simple.
- **Caché en disco** para arranque instantáneo (prioridad 1): `redb` (KV puro Rust, ACID, sin C) guardando `NoteMeta` + links serializados (`postcard`/`bincode`) por ruta. En el arranque: cargar caché → mostrar UI → en fondo comparar `mtime+size` (y hash si cambió) → reparsear solo lo distinto. `rusqlite` solo si se quieren consultas ad-hoc (estilo Dataview); sqlite cuesta toolchain C y es más difícil de razonar para agentes.
- **Reindex incremental**: evento de `notify` → reparse de ese archivo → diff de links salientes viejos vs nuevos → actualizar `incoming` de los afectados → emitir `IndexEvent::BacklinksChanged(Vec<NoteId>)` a la UI.
- **Renombrar** una nota = actualizar todos los `[[links]]` que apuntan a ella (Obsidian lo hace). Requiere rangos exactos → otra razón para guardar `range` en cada `Link`. Escribir con escrituras atómicas y una sola transacción de undo multi-archivo **(diseño propio, sin verificar en otros editores)**.

---

## 5. Hilos, seguridad ante caídas, config, plugins

### Modelo de concurrencia

- **Hilo de UI**: solo estado de vista + aplicar ediciones al buffer activo (las ediciones son baratas con rope).
- **Fondo**: parse tree-sitter si el documento es grande, indexado, fuzzy, búsqueda. Usar el executor del framework (GPUI tiene `background_executor`; iced tiene `Task`/subscriptions) **o** `rayon` para paralelismo de datos (indexado inicial).
- **Comunicación**: mensajes, no locks compartidos. `crossbeam-channel` 0.5.17 o `flume` 0.12. Estado leído por muchos y escrito poco (índice) → `arc-swap` 1.9 con snapshots inmutables: la UI lee un `Arc<IndexSnapshot>` sin bloquear nunca.
- **Tokio: no** para el núcleo. No hay red en el MVP; meter un runtime async complica el código que generan los agentes sin beneficio. Si GPUI, su executor ya cubre `async`.
- Cada versión de buffer lleva `version: u64`; los resultados de fondo traen la versión con la que se calcularon y se descartan si quedaron viejos.

### Crash safety

- **Escritura atómica**: `tempfile::NamedTempFile::new_in(misma_carpeta)` → escribir → `sync_all` → `persist(destino)` (rename atómico en el mismo filesystem). `atomicwrites` 0.4.4 hace lo mismo pero sin release desde 2024; `tempfile` (3.27) basta. *(Hecho en M0b: `nota_vault::write_atomic`. Además hace `fsync` del directorio tras el rename, conserva los permisos, escribe en el destino si la ruta es un enlace simbólico y borra el temporal si algo falla. No conserva el dueño si el archivo es de otro usuario: haría falta `chown` y privilegios.)*
- Preservar permisos y fin de línea / BOM del archivo original. Nunca reformatear lo que el usuario no tocó.
- **Autosave** con debounce (~1 s tras dejar de teclear, y al perder foco), como Obsidian. Además un *swap/journal* en `$XDG_STATE_HOME/nota/` con el contenido no guardado, para recuperar tras un crash.
- **Conflictos externos** (git pull, Syncthing): si `notify` avisa de cambio en un archivo con cambios locales sin guardar, no sobrescribir: mostrar diff (`similar` 3.2 o `imara-diff` 0.2) y elegir.
- Ignorar los eventos de `notify` que provoca nuestro propio guardado (comparar hash tras escribir).

### Config

- TOML con `toml` 1.1 + `serde`. Rutas XDG con `etcetera` 0.11 o `dirs` 7.0: `~/.config/nota/config.toml`, `keymap.toml`, `themes/`.
- Config **por vault** opcional en `<vault>/.nota/` (no tocar `.obsidian/`; leerlo de solo lectura, p. ej. para respetar `attachmentFolderPath`, más adelante).
- Defaults embebidos con `include_str!`; config del usuario se fusiona encima; recarga en caliente vía `notify`.
- Validar y reportar errores de config en la UI, nunca hacer panic.

### Plugins (post-MVP, solo nota)

Zed usa WASM components sobre `wasmtime` (49.0.3, oct-2026) con WIT para definir la API. Es el camino sano: sandbox, cualquier lenguaje, sin ABI de Rust inestable. Requisito para dejarlo abierto desde ya: que **todo** pase por acciones con nombre y por un `IndexSnapshot` consultable; eso será la API de plugins. Compatibilidad con plugins JS de Obsidian: fuera de alcance.

---

## 6. Layout del repo e idioms

### Workspace propuesto

```
nota/
├── Cargo.toml              # [workspace], [workspace.dependencies], [workspace.lints]
├── CLAUDE.md               # convenciones para agentes (ver §7)
├── crates/
│   ├── nota-buffer/        # Buffer (ropey), Edit, Transaction, History, Selections, newtypes de offset
│   ├── nota-syntax/        # tree-sitter-md + scanner Obsidian (wikilinks, tags, embeds, frontmatter)
│   ├── nota-vim/           # Mode, Motion, Operator, TextObject, VimState → Transaction. Depende solo de nota-buffer (+ nota-syntax para text objects md)
│   ├── nota-keymap/        # KeyEvent, trie, contextos y predicados, parseo TOML, registro de Actions
│   ├── nota-vault/         # leer y guardar notas (atómico); luego walk, watch, índice, links, backlinks, caché redb
│   ├── nota-search/        # nucleo (fuzzy) y luego tantivy
│   ├── nota-config/        # settings, rutas XDG, temas
│   ├── nota-ui/            # vistas sobre el framework elegido (editor, file tree, picker, paleta)
│   └── nota/               # binario: main, wiring, CLI (`nota <vault>`)
├── xtask/                  # tareas: generar fixtures, bench, release
└── tests/fixtures/vaults/  # vaults de ejemplo (incluir uno exportado de Obsidian real)
```

Regla de dependencias (verificable con `cargo tree` en CI): **solo `nota-ui` y `nota` dependen del framework de GUI**. Todo lo demás compila y se testea sin ventana. Eso hace que cambiar GPUI ↔ iced cueste una sola capa.

`Cargo.toml` raíz:

```toml
[workspace]
resolver = "3"
members = ["crates/*", "xtask"]

[workspace.package]
edition = "2024"
license = "GPL-3.0-or-later"   # o MIT/Apache-2.0: decidir; GPUI es Apache-2.0
rust-version = "1.90"          # fijar con rust-toolchain.toml (sin verificar la MSRV de GPUI)

[workspace.lints.rust]
unsafe_code = "forbid"
[workspace.lints.clippy]
pedantic = { level = "warn", priority = -1 }
unwrap_used = "deny"           # en crates de lib; permitir en tests
dbg_macro = "deny"
```

### Testing

| Tipo | Herramienta | Para qué |
|---|---|---|
| Snapshot | `insta` 1.49 | Motor vim: `"hello |world" + "dw" → "hello |"` como snapshot legible. Parser Obsidian: AST de links/tags por fixture. Resolución de links por vault de prueba. |
| Propiedades | `proptest` 1.12 | Invariantes: undo∘redo = id; cada número de secuencia del historial muestra siempre el mismo texto, se llegue por undo, redo, `g-` o `g+`; aplicar `Transaction` y su inversa deja el rope igual; offsets siempre en frontera de char; `Selections` siempre ordenadas y sin solaparse; reindex incremental == reindex desde cero. |
| Tabla | `rstest` 0.27 | Casos parametrizados de motions/text objects. |
| Diff legible | `pretty_assertions` 1.4 | |
| Runner | `cargo-nextest` 0.9.148 | Paralelo, rápido en CI. |
| Bench | `criterion` o `divan` **(sin verificar versiones)** | Indexar vault de 10 k notas, latencia por tecla. Guardar baseline. |
| UI | test del framework (GPUI `TestAppContext`; iced: lógica pura en `update`) | Mínimo; la lógica vive fuera de la UI. |

Formato de test de vim recomendado (copiado de cómo Zed testea su vim, con `|` como cursor **(estilo, sin verificar literalmente)**):

```rust
#[rstest]
#[case("hel|lo world", "dw", "hel|world")]
#[case("|foo [[bar]] baz", "f[dil", "foo [[|]] baz")]
fn normal_mode(#[case] before: &str, #[case] keys: &str, #[case] after: &str) { … }
```

### CI y calidad

- GitHub Actions / Forgejo Actions: `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo nextest run`, `cargo deny check` (licencias y advisories; `cargo-deny` 0.20), `cargo machete` (deps sin usar), `typos`.
- `rustfmt.toml` mínimo (`imports_granularity = "Crate"` requiere nightly; mejor no).
- Cache con `Swatinem/rust-cache` **(sin verificar versión)**.

### Empaquetado Linux

- **`cargo-dist`** 0.32.0 (may-2026): planea y construye releases en CI, tarballs, installer shell, Homebrew, MSI. **No genera AppImage ni Flatpak.**
- **AppImage**: `cargo-appimage` 2.4 (poco uso) o `linuxdeploy` en un job aparte. Las apps GPU (Vulkan/wgpu) dentro de AppImage dan guerra con drivers **(sin verificar para GPUI)**.
- **Flatpak**: manifest propio + `flatpak-cargo-generator` para vendorear deps; es el camino para Flathub. Permiso `--filesystem=home` o portal de documentos para abrir vaults.
- **AUR**: PKGBUILD a mano (`nota` desde fuente y `nota-bin` desde el tarball de cargo-dist). `cargo-aur` 1.7 existe pero está casi muerto (885 descargas recientes).
- `.deb`/`.rpm`: `cargo-deb` 3.8, `cargo-generate-rpm` 0.21.
- Archivo `.desktop`, icono y metainfo AppStream en `packaging/linux/` desde el primer release.

---

## 7. Vibe-coding Rust con agentes: lo que funciona

1. **Crates pequeños con frontera clara.** Un agente con `nota-vim` en contexto (pocos miles de líneas, sin GUI) acierta mucho más que con un crate de 30 k. Cada crate con su `README` de 10 líneas: qué hace, qué no hace, tipos públicos.
2. **Tipos fuertes como barandal.** Newtypes (`ByteOffset`, `NoteId`, `Row`), enums exhaustivos para `Motion`/`Action`, `#[non_exhaustive]` donde vaya a crecer. El compilador rechaza lo que el agente inventa; el ciclo `cargo check` es el mejor revisor.
3. **Tests como spec, escritos antes.** Para el motor vim, escribir (o pedir) primero la tabla de casos `antes / teclas / después` copiando el comportamiento real de nvim; luego "haz que pase". Se puede generar la verdad con `nvim --headless` desde un `xtask` y fijarla como snapshot: el oráculo es vim real, no el modelo. *(Hecho en M0a de otra forma: cada caso de la tabla de `nota-vim` corre también en nvim dentro de `cargo test`; ver §3, "Estado".)*
4. **Lógica pura, UI tonta.** Toda decisión en funciones `fn(&State, Input) -> Output`. La GUI es lo que peor genera un modelo y lo más difícil de testear; que tenga poca lógica.
5. **`CLAUDE.md` (o `AGENTS.md`) con reglas cortas y verificables**:
   - "Offsets internos en bytes; convertir solo en el render."
   - "Nada de `unwrap()` fuera de tests; errores con `thiserror` en libs y `anyhow` solo en el binario."
   - "Nunca bloquear el hilo de UI; I/O siempre en fondo."
   - "Solo `nota-ui` y `nota` importan el framework de GUI."
   - "Antes de terminar: `cargo fmt`, `cargo clippy -D warnings`, `cargo nextest run` en verde."
   - "No añadir dependencias sin verificar en crates.io que existen y están mantenidas."
6. **Desconfiar de APIs recordadas.** GPUI cambia rápido y su versión en crates.io (0.2.2, oct-2025) va detrás del monorepo de Zed; iced 0.14 rompió API respecto a 0.13. Dar al agente el `docs.rs` o el código fuente de la versión fijada (vendored en `docs/vendor/` o con `cargo doc`), no dejar que adivine.
7. **Rebanadas verticales chicas**: una tarea = un comportamiento observable + su test. "Implementa `ciw`" sí; "implementa vim" no.
8. **Worktrees separados** si hay varios agentes en paralelo (comparten `target/` y se pisan los type-checks).
9. **Benchmarks como guardas de rendimiento** (prioridad 1): un test que falle si indexar el vault fixture pasa de X ms evita regresiones silenciosas de código generado.

---

## 8. Hitos del MVP

### Estado (M0b, 2026-10-17)

- `nota-ui` (vista de editor sobre GPUI) y el binario `nota` existen; son los únicos que dependen de GPUI y el CI lo verifica con `cargo tree`. `gpui-kit` va sin features por defecto: solo `gpui-pre`, sin gpui-component ni su `Root` (04, riesgo 1).
- La vista pinta solo las líneas visibles, guarda el scroll en líneas (04, riesgo 6) y en Insert y en la línea de comandos el texto entra siempre por el input handler (04, riesgo 4). Colores y tamaños salen de `Theme`; la fuente es la primera monoespaciada instalada de una lista, porque GPUI entra en pánico con una familia que no existe.
- `nota-vim` tiene una línea de comandos mínima: `:w`, `:q`, `:q!`, `:wq`. El motor no hace I/O: devuelve el `Command` y la vista invoca la acción con nombre (`editor::save`, `editor::quit`, …).
- "Modificado" compara el estado del historial (`Buffer::history_seq`) con el guardado, más las ediciones de un Insert sin cerrar: como en vim, deshacer o `g-` hasta lo guardado quita el `[+]`, y llegar al mismo texto por otra edición no.
- Smoke sin GPU, bajo `xvfb-run` (lavapipe), con XTest tecleando: archivo nuevo → `i…<Esc>:wq` lo crea; con cambios, `:q` y el cierre pedido por el gestor de ventanas se niegan; `:w` conserva permisos 0640; `:q!` sale sin guardar; no quedan temporales. Primer frame en release: 631 ms con un archivo chico y 762 ms con 50 MB (el spike midió 710 ms aquí y 293 ms en la máquina de pruebas). El número que vale es el de la máquina de pruebas.
- Fuera de M0: autosave, journal de recuperación, conflictos con cambios externos, preservar el dueño del archivo.

| Hito | Contenido | Hecho cuando… |
|---|---|---|
| **M0 — Ventana + texto** ✅ (2026-10-17) | Workspace, CI, `nota-buffer` (Buffer, Edit, History), ventana con un buffer editable (sin modo), abrir/guardar un archivo con escritura atómica. | Abres `nota archivo.md`, editas, guardas; proptest de undo/redo en verde; arranque al primer frame ≈ 300 ms. El objetivo original de < 100 ms no era realista: el spike midió 244–297 ms en la máquina de pruebas y casi todo es GPUI/Vulkan levantando la ventana (cargar 50 MB son 71 ms, un archivo chico 2 ms). |
| **M1 — Vim normal/insert** | `nota-keymap` (trie + contextos), `nota-vim`: normal, insert, motions básicos (`hjkl wbe 0^$ gg G f t`), operadores `d c y` + `p`, `x`, `u`/`ctrl-r`, `.`, counts, text objects `iw aw i" i( ip`. Cursor bloque/línea por modo. | ~150 casos de tabla verificados contra nvim headless. |
| **M2 — Vault + finder** | `nota-vault`: abrir carpeta, walk con `ignore`, watcher, árbol de archivos con navegación vim, fuzzy finder (`nucleo`) en `space f`, paleta de comandos, `:e`, `:w`, `:q`, splits `ctrl-w`. | Abre un vault real de Obsidian (≥ 5 k notas) en < 1 s **(objetivo)**; crear/renombrar/borrar desde el árbol. |
| **M3 — Wikilinks + backlinks** | `nota-syntax` (scanner Obsidian + frontmatter), índice de links, resolución estilo Obsidian, `gd`/`enter` sigue link, autocompletado de `[[`, panel de backlinks (con vim), tags, links no resueltos, renombrar nota actualiza links. | Fixture de resolución en `insta`; reindex incremental == completo (proptest). |
| **M4 — Live preview** | tree-sitter-md incremental; ocultar marcadores fuera de la línea del cursor, headings con tamaño, negrita/cursiva, links como texto clicable, listas y checkboxes (`ctrl-enter`/`gx` alterna), código con resaltado, `==highlight==`, comentarios `%%`. Toggle source/live/lectura. | Escribir en una nota de 5 k líneas sin latencia perceptible (< 16 ms por tecla **objetivo**). |
| **M5 — Embeds y callouts** | `![[img]]` imágenes, `![[nota]]` transclusión (solo lectura), callouts, tablas renderizadas. | El vault de prueba se ve "como en Obsidian" en lo común. |
| **M6 — Búsqueda y pulido** | Búsqueda full-text (grep paralelo → `tantivy` si hace falta), visual mode completo, registros, macros, `:%s`, marks, keymap y tema del usuario en TOML, autosave + journal de recuperación, caché `redb` para arranque en caliente. | Primer release con cargo-dist + AUR. |
| Post-MVP | Multi-cursor, vista de grafo, daily notes/plantillas, math, export HTML/PDF, plugins WASM. | — |

---

## Crates verificados (crates.io, 2026-10-15)

`ropey 1.6.1` · `crop 0.4.3` · `lapce-xi-rope 0.4.0` · `zed-sum-tree 0.2.0` · `pulldown-cmark 0.13.4` · `comrak 0.56.0` · `markdown 1.0.0` · `tree-sitter 0.27.1` · `tree-sitter-md 0.5.3` · `tree-sitter-highlight 0.27.1` · `modalkit 0.0.31` · `editor-types 0.0.6` · `keybindings 0.0.4` · `ignore 0.4.33` · `notify 8.2.0` · `notify-debouncer-full 0.7.0` · `nucleo 0.5.0` · `nucleo-matcher 0.3.1` · `frizbee 0.13.0` · `tantivy 0.26.2` · `redb 4.4.0` · `rusqlite 0.40.2` · `petgraph 0.8.3` · `tempfile 3.27.0` · `toml 1.1.8` · `serde_norway 0.9.42` · `yaml-rust2 0.13.0` · `etcetera 0.11.0` · `dirs 7.0.0` · `crossbeam-channel 0.5.17` · `flume 0.12.0` · `rayon 1.12.0` · `arc-swap 1.9.2` · `smallvec 1.16.3` · `smol_str 0.3.6` · `unicode-segmentation 1.13.3` · `unicode-width 0.2.2` · `regex-automata 0.4.18` · `similar 3.2.0` · `imara-diff 0.2.0` · `wasmtime 49.0.3` · `insta 1.49.0` · `proptest 1.12.0` · `rstest 0.27.0` · `pretty_assertions 1.4.1` · `cargo-nextest 0.9.148` · `cargo-deny 0.20.2` · `cargo-machete 0.9.2` · `typos-cli 1.51.1` · `cargo-dist 0.32.0` · `cargo-deb 3.8.0` · `cargo-generate-rpm 0.21.0` · `cargo-appimage 2.4.0` · `cargo-aur 1.7.1` · `gpui 0.2.2` · `gpui-component 0.7.3` · `iced 0.14.0`.

Descartados o trampas: `vim-like` (no existe), `helix-core` (placeholder 0.0.0), `tree-sitter-markdown` (otro crate, 2021), `xi-rope` (2019, usar `lapce-xi-rope`), `serde_yaml` (deprecado), `zed-rope` (no publicado).
