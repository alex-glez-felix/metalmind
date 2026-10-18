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

El **mismo** scanner de la sintaxis de nota (crate `nota-syntax`) debe usarse en ambos caminos para que el editor y el índice nunca discrepen sobre qué es un link.

### Sintaxis a cubrir (adoptada de Obsidian donde conviene)

| Sintaxis | Ejemplo | Fase |
|---|---|---|
| Wikilink, alias, heading, bloque | `[[Nota]]`, `[[Nota\|alias]]`, `[[Nota#Sección]]`, `[[Nota#^id]]` | M3 |
| Embed | `![[Nota]]`, `![[img.png\|300]]`, `![[Nota#Sección]]` | M4–M5 |
| Tags | `#tag`, `#a/b/c`, `tags:` en frontmatter | M2.5 ([05](05-tags.md)) |
| Frontmatter YAML | `tags` (M2.5); `aliases`, `cssclasses` | M3 |
| Callouts | `> [!note] Título` y plegables `> [!tip]-` | M5 |
| Highlight, comentarios | `==x==`, `%% x %%` | M4 |
| Math | `$x$`, `$$...$$` | post-MVP |
| Block IDs | `^abc123` al final de párrafo | M3 (índice) |

Resolución de wikilinks: por nombre de archivo sin extensión, case-insensitive, desempate por ruta más corta / misma carpeta; también matchea `aliases` del frontmatter. Documentarlo como tabla de casos en un test `insta` — es la spec.

Estado (2026-10-18): `nota-syntax` tiene el scanner a mano, todavía sin tree-sitter (entra en M4): wikilinks, embeds, tags, links markdown a archivos locales y frontmatter, ignorando bloques cercados, código inline y `%% %%`. La tabla de resolución está en `crates/nota-syntax/tests/resolve.rs`. La sintaxis es la de Obsidian salvo donde su regla es peor para nota (el README no pide compatibilidad):

- **Resolución**: nombre o final de ruta, sin mayúsculas, con o sin `.md`; gana la de menos carpetas. Si empatan, desempata la única escrita con las mismas mayúsculas (en Linux `Casa.md` y `casa.md` conviven) y si no, es `Ambiguous`: Obsidian elige una sin avisar y un link puede cambiar de nota sin que nadie lo note. Los destinos `./` y `../` parten de la carpeta de la nota de origen (`resolve_from`). La misma carpeta y los `aliases` como desempate quedan para el índice, que tiene el contexto.
- **Tags**: a diferencia de Obsidian, pueden ser solo números: `#2024` agrupa un año y en nota los tags son el eje de organización; el costo aceptado es que `PR #12` deja un tag `#12` (05 §1). Un tag empieza al inicio de línea o tras un espacio, así `texto#algo` y el fragmento de una URL no lo son. Ni solo separadores ni niveles vacíos: `#-` y `#a//b` no son tags y `#a/` es `#a` (05 §1).
- **Frontmatter**: solo los nombres en plural (`tags`, `aliases`, `cssclasses`); los singulares que Obsidian aún lee de antes de su 1.4 son claves cualquiera (un importador puede convertirlos, `docs/IDEAS.md`). Se lee como YAML 1.2 con `serde-saphyr` 2.0, el mismo crate que la config (sección siguiente), así que `tags: [no, on]` son los textos `no` y `on`. Un texto es un solo valor: `tags: "rust, nota"` no se parte y, como no es un tag válido, queda como problema (05 §1). Un YAML roto o un campo de otro tipo es un problema recuperable y el resto de la nota se escanea igual.

### Frontmatter YAML

El mismo crate lee la config (§5) y el frontmatter. Requisito: **YAML 1.2**, no 1.1. En 1.1, `NO`, `on`, `yes` y `off` son booleanos y `0755` es octal aunque el campo espere texto (el "problema de Noruega"); 1.2 (core schema) solo reconoce `true`/`false` y es superset de JSON, así que una config en JSON puro también vale.

Candidatos (versiones y dependencias en crates.io, 2026-10-18; versión de YAML según la documentación de cada uno):

| Crate | Versión | YAML | Serde | Notas |
|---|---|---|---|---|
| `serde_yaml` | 0.9.34+deprecated | 1.1 (libyaml) | Sí | Deprecado. |
| `serde_norway` | 0.9.42 | 1.1 | Sí | Fork de serde_yaml sobre `unsafe-libyaml-norway`, es decir, libyaml. Sin release desde dic-2024. |
| `serde_yaml_ng` | 0.10.0 | 1.1 | Sí | Otro fork sobre `unsafe-libyaml`. Sin release desde may-2024. |
| `serde_yml` | 0.0.13 | — | — | Él mismo se declara deprecado y sin mantenimiento; reenvía a `noyalib`. |
| `noyalib` | 0.0.58 | 1.2 | Sí | Nuevo (may-2026), API 0.0.x y un árbol de dependencias grande. |
| `yaml-rust2` | 0.13.0 | 1.2 | No | Parser y árbol propio; sin serde ni spans en los valores. |
| `saphyr` | 0.2.2 | 1.2 | No | Sucesor de yaml-rust2, con spans (`MarkedYaml`); sin serde. |
| **`serde-saphyr`** | **2.0.0** | **1.2** | Sí | Sobre `granit-parser` 2.0 (fork de saphyr-parser, YAML 1.2). Sin `unsafe`, sin panics ante YAML malformado (fuzzing), cubre el yaml-test-suite, `Spanned<T>` con línea y columna, errores con posición, límites contra alias bombs. |

Elegido **`serde-saphyr`**, sin las features por defecto (solo `deserialize`: no hace falta serializar porque nota nunca reescribe el YAML del usuario). Ojo: por defecto acepta `yes`/`on` en un campo `bool` (lo hace por compatibilidad con 1.1); `nota-config` activa `strict_booleans`, así que en un `bool` solo valen `true` y `false` y lo demás es un error de tipo con posición. En un campo de texto, `NO` o `on` son ese texto porque el tipo de destino manda. Las claves repetidas son error (default). Para el frontmatter sirve igual: `Spanned<T>` da la posición de cada valor para el editor, `render_options!{ line_offset }` corre la línea de un error al número de línea real de la nota, y un `deserialize_any` tolera lo que haya.

Plan para el frontmatter: extraer el bloque `---...---` a mano (rangos exactos para el editor), deserializar con `serde-saphyr` a un `Frontmatter { aliases, tags, raw }` tolerante, y **nunca reescribir** el YAML del usuario salvo petición explícita.

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
- **Dot-repeat** (`.`) = guardar la orden que empezó el último cambio, su count y lo tecleado en Insert (teclas, no el texto que quedó, como vim). **Macros** (`q`) = guardar la secuencia de `KeyEvent`s y re-alimentar el keymap.
- **Text objects markdown** (`il`/`al` link, `ih`/`ah` sección por heading, `ic` bloque de código) salen del árbol tree-sitter: diferenciador barato frente a Obsidian.

### Estado (M0a, 2026-10-15)

- `nota-vim` existe como máquina de estados pura: Normal, Insert, operador pendiente y Visual por carácter y por línea (`v`, `V`; ver abajo); motions `h j k l w W b B e E 0 ^ $ gg G` y `f F t T ; ,` con counts; `d c y`, `dd cc yy`, `gu gU g~` (con `guu gUU g~~`), `x`, `p P`, `u`, `ctrl-r`, `g- g+` (historial en árbol), `.`; text objects `iw aw iW aW i( a( i" a" ip ap`; multi-cursor en todas las operaciones.
- Búsqueda (2026-10-18): `/ ? n N * # g* g#`, también como motion de operador (`d/foo<CR>`, `dn`, que `.` repite), con historial por flechas, y `:s`/`:%s` con rangos, flags `g i I e`, `&` y `\1` en el reemplazo, en un solo paso de undo. El dialecto de vim se traduce a `regex` línea por línea (`ignorecase` + `smartcase` por default); lo que no tiene equivalente se rechaza con un error en vez de buscar otra cosa (lista en el README del crate). El patrón activo queda en `Vim::search_highlight` para que la UI lo resalte.
- Registros (`"a`–`"z`, `"A`–`"Z`, `"0`, `"1`–`"9`, `"-`, `"_`, `".`, `"/`, `"+`, `"*`), macros (`q`, `@`, `@@`, con count) y marks (`m`, `'`, `` ` ``, `''`, ` `` `). El motor no hace I/O: `"+` y `"*` los provee quien lo aloja con el trait `Clipboard`, que se pasa en cada llamada porque GPUI solo da el portapapeles con el contexto de la app. Una macro va en una cola, como el typebuf de vim, se corta cuando algo falla como un `beep_flush` y se deshace con un solo `u` (grupos de undo de `nota-buffer`). Las marks son `LineAnchor`s de `nota-buffer`, que ajusta cualquier edición, incluso el undo. Pendiente: los registros son de cada `Vim` y las marks no tienen `A`–`Z`; cuando haya buffers múltiples, los registros deben compartirse entre editores y las marks globales vivir fuera del buffer.
- Visual (2026-10-18): `v`/`V` entran, cambian de tipo o salen; `o`, `gv`, `<Esc>`; las motions, counts y búsquedas (`/ ? n N`) extienden la selección y los text objects la seleccionan o la hacen crecer, portando las ramas de Visual de `textobject.c`. Operadores `d x c y p P r ~ u U`, también con registro (`"ay`); `.` repite sobre la misma cantidad de texto (`redo_VIsual` de vim) y `[count]v` reusa el tamaño de la última. La selección es `Selection { anchor, head }` con los dos extremos incluidos, y el cursor puede quedar sobre el fin de línea, como con `selection=inclusive`; la UI todavía la pinta como una del ratón, sin el último carácter. Por bloque (`ctrl-v`) queda fuera. Límites en el README del crate.
- El keymap vive por ahora dentro de `nota-vim`. El default es una tabla tipada `(contexto, teclas, acción)`, infalible y validada por un test (sin secuencias ambiguas ni duplicadas); el parseo de strings, que sí puede fallar, llegará con la config del usuario. Se moverá a `nota-keymap` cuando haya más de un consumidor.
- **Referencia de comportamiento = `nvim --clean` 0.12.6 (última estable, 2026-10-09) con `noautoindent`.** Importa porque los defaults de nvim difieren de vim: `nostartofline` (`dd`, `G`, `gg` conservan la columna), `backspace=indent,eol,start`, `cpoptions=aABceFs_`. Las motions de palabra y los text objects portan la lógica de `textobject.c` (`fwd_word`, `current_word`, `current_block`, `current_quote`) para que los bordes coincidan.
- Cada caso de la tabla (`crates/nota-vim/tests/vim.rs`) corre en el motor y en nvim (`tests/support/oracle.lua`). El oráculo exige esa versión exacta y el CI la instala con checksum. Al pasar de 0.10.4 a 0.12.6 los 207 casos dieron lo mismo. Los de Visual (`tests/visual.rs`) comparan además el otro extremo de la selección (`getpos("v")`).

### Vim en toda la UI

El truco de Zed: **vim no es especial**; es un conjunto de bindings en contextos.

- Cada vista declara su contexto (`Editor`, `FileTree`, `Picker`, `CommandPalette`, `SearchBar`, `Backlinks`) y sus predicados (`mode == normal`, `editing`, `focused`).
- `FileTree`: `j/k` mover, `h/l` colapsar/expandir, `gg/G`, `/` filtrar, `a` crear, `r` renombrar, `dd` borrar (con confirmación), `o` abrir en split.
- `Picker`/paleta: el input es un `Editor` de una línea (contexto `Picker > Editor`) en insert por defecto; `Esc` pasa a normal y `j/k` mueve la lista.
- `:` abre una línea de comandos que parsea a las **mismas** acciones (`:w` → `editor::save`, `:e nota` → `editor::edit`, `:vs` → `pane::split_right`, `:q` → `editor::quit`; `:%s/a/b/g` y `:noh` los corre el propio motor). Un solo registro de acciones = paleta, `:` y keymap comparten nombres. Los nombres de `nota` van en snake_case (`vim::next_word_start`), a diferencia del CamelCase de Zed.
- Leader: `"space f": "workspace::open_finder"` en el contexto `VimControl && !editing`; configurable por YAML.
- Ventanas: `ctrl-w h/j/k/l/v/s/w/o` como acciones de `pane::` (`ctrl-w q` es `editor::quit`, igual que `:q`). Viven en el keymap de `nota-vim`, en Normal, y el motor las devuelve como `Command` para que las ejecute la vista: así `ctrl-w` comparte prefijo, count y cancelación con el resto de las teclas (`d ctrl-w v` cancela el `d`, como en vim) en vez de competir con el keymap de GPUI.

Keymap del usuario en **YAML** (coherente con el resto de la config) que se fusiona sobre el default embebido:

```yaml
keymap:
  - context: Editor && vim_mode == normal
    bindings:
      space f: workspace::open_finder
      g d: editor::follow_link
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
}
```

Los tags tienen su propio `TagIndex`, alimentado del mismo scan y del mismo watcher: ver [05 §2](05-tags.md#2-modelo-de-datos).

- **Fuente de verdad = los `.md` en disco.** El índice es derivado y siempre reconstruible. Nada de base de datos propietaria (eso es exactamente lo que hace a Obsidian compatible con git, sync, etc.).
- **En memoria** para el MVP: 10 k notas × ~20 links ≈ unos MB. Rápido y simple.
- **Caché en disco** para arranque instantáneo (prioridad 1): `redb` (KV puro Rust, ACID, sin C) guardando `NoteMeta` + links serializados (`postcard`/`bincode`) por ruta. En el arranque: cargar caché → mostrar UI → en fondo comparar `mtime+size` (y hash si cambió) → reparsear solo lo distinto. `rusqlite` solo si se quieren consultas ad-hoc (estilo Dataview); sqlite cuesta toolchain C y es más difícil de razonar para agentes.
- **Reindex incremental**: evento de `notify` → reparse de ese archivo → diff de links salientes viejos vs nuevos → actualizar `incoming` de los afectados → emitir `IndexEvent::BacklinksChanged(Vec<NoteId>)` a la UI.
- **Renombrar** una nota = actualizar todos los `[[links]]` que apuntan a ella (Obsidian lo hace). Requiere rangos exactos → otra razón para guardar `range` en cada `Link`. Escribir con escrituras atómicas y una sola transacción de undo multi-archivo **(diseño propio, sin verificar en otros editores)**.

### Estado (M2, 2026-10-17): índice de archivos

- `Vault::open(raíz)` canonicaliza la raíz y la recorre con `ignore` en paralelo. El resultado es un `VaultIndex`: cada ruta (`VaultPath`, relativa, con `/`) es carpeta, nota (`.md`, sin distinguir mayúsculas) o adjunto, y cada carpeta guarda sus hijos ya ordenados como el "A a Z" de Obsidian (carpetas primero, nombre sin mayúsculas). La UI recorre el árbol con `children(carpeta)` sin ordenar nada.
- Reglas del walk, iguales en cualquier máquina: `.gitignore` e `.ignore` del vault aunque no sea un repo git (`require_git(false)`) y `.git/info/exclude`; no los de carpetas superiores a la raíz ni el `excludesFile` global de git, que harían que el mismo vault se vea distinto según quién lo abra.
- Los ocultos (nombre con punto) no se muestran nunca, y eso cubre `.obsidian/`, `.git/` y `.trash/`. Va como filtro propio y no con `hidden(true)` a secas, porque en `ignore` un `!.obsidian/` en `.gitignore` le gana a `hidden`. `VaultPath` tampoco admite componentes ocultos, `..` ni `.`: una ruta del vault nunca sale de la raíz.
- Los enlaces simbólicos no se siguen ni se listan (la raíz sí puede serlo): seguirlos podría sacar al vault de su raíz o duplicar carpetas. Los nombres que no son UTF-8 se saltan con un aviso en el log.

### Estado (M2, 2026-10-17): watcher

- `Vault::open` empieza a vigilar **antes** del walk (`notify-debouncer-full`, 100 ms, recursivo): lo que cambie mientras se indexa llega después como evento y no se pierde nada entre el walk y el watcher.
- Un hilo propio recibe los lotes del debouncer y no interpreta cada evento: junta las rutas que tocó el lote, las relee del disco con el **mismo** walk de `ignore` (desde la raíz, para que apliquen los `.gitignore` de por medio, pero entrando solo en los ancestros de esas rutas) y saca los cambios de comparar antes y después. Así el índice incremental es por construcción el de un walk completo, lleguen los eventos como lleguen; un proptest lo comprueba con secuencias al azar de crear, borrar y mover, y otro con el watcher de verdad.
- Los cambios se publican como `VaultUpdate { changes, snapshot }` por un `mpsc` y el índice queda en un `ArcSwap<VaultIndex>`: la UI lee `Vault::snapshot()` sin bloquear nunca, y quien quiera reaccionar a cada cambio lee el canal. `snapshot` va dentro de cada update para que cambios e índice nunca se desfasen. Cada lote clona el índice entero; con 5 k entradas es barato, y si con vaults grandes deja de serlo, el paso es un árbol persistente.
- `VaultChange` es `Created`, `Removed`, `Renamed` (si es carpeta, cubre todo su contenido) o `Modified`. Un rename sale como `Renamed` cuando el debouncer empareja origen y destino (en Linux, por el cookie de inotify). Un **guardado atómico** (temporal + rename encima) sale como `Modified` del destino y nunca como borrar y crear, sea el temporal oculto (`write_atomic`) o visible al lado (`nota.md.tmp`). Una ráfaga llega en pocos lotes: en el test, 330 operaciones en menos de 33. Dentro de un lote, los cambios van en orden de aplicación: primero los `Removed` (en rutas de antes), luego `Renamed`, `Created` y `Modified`; así, si se renombra una carpeta y se borra algo de dentro en el mismo lote, quien reaplica los cambios llega al mismo índice.
- Abrir, leer o cambiar permisos no cuenta. Tocar un `.gitignore` o `.ignore` relee su carpeta. Si `notify` avisa que pudo perder eventos (`Flag::Rescan`, desborde de la cola de inotify) o da un error sin ruta, se relee todo el vault.
- Al soltar el `Vault`, se para el debouncer y se espera al hilo: no queda nada escribiendo en segundo plano.

### Estado (M2, 2026-10-17): búsqueda difusa

- Vive en `nota-search`, como dice §6, y no depende de `nota-vault`: recibe las rutas como `&str` y devuelve índices. Usa **`nucleo-matcher`** y no `nucleo`: el finder necesita una función pura (query → resultados ordenados con posiciones), y la inyección incremental y el hilo de fondo de `nucleo` solo hacen falta si buscar en todo el vault deja de caber en un frame. Con 5 k rutas cabe de sobra (abajo).
- Config de rutas de `nucleo` (`match_paths`), mayúsculas y acentos "smart" (cuentan solo si el query los lleva). Orden: puntaje, luego ruta más corta, luego orden de entrada; el query vacío devuelve todo tal cual, para que la UI ponga primero las recientes.
- `search(query, candidatos, limit)`: puntúa todo, se queda con los `limit` mejores (`select_nth_unstable`) y solo a esos les calcula posiciones, que es lo caro. Con la lista completa, `tema nota` sobre 5 100 rutas tardaba 8.7 ms; con una página de 50, 4.2 ms.
- Las posiciones salen en bytes: `nucleo` cuenta en grafemas (o en bytes si cada grafema empieza con ASCII) y se convierten en el borde, con `unicode-segmentation`, igual que el resto del núcleo.

### Estado (M2, 2026-10-17): operaciones de archivo

- `Vault::create_note`, `create_folder`, `rename` (renombrar y mover son lo mismo) y `move_to_trash`. No tocan el índice: el watcher ve el cambio en disco y lo publica como cualquier otro, así que hay un solo camino para que el índice cambie.
- **Borrar va a `.trash/` del vault, no a la papelera del sistema** (crate `trash`). Es lo que hace Obsidian con su opción "papelera de Obsidian", así que un vault compartido con Obsidian ve las mismas cosas borradas; queda dentro del vault y viaja con git o Syncthing; es un `rename` atómico dentro del mismo filesystem, sin depender de que el escritorio implemente la spec de freedesktop ni de que la papelera del sistema esté en otro disco (con un vault en un USB, mover a `~/.local/share/Trash` sería copiar); y es determinista en los tests. `.trash/` es plana como la de Obsidian: si el nombre ya está, se prueba `nota 1.md`, `nota 2.md`, … **(el sufijo es decisión propia, sin verificar contra Obsidian)**.
- Nada pisa nada: renombrar usa `renameat2(RENAME_NOREPLACE)` vía `rustix` (ya venía por `tempfile`), porque `rename(2)` reemplaza en silencio un archivo o una carpeta vacía, y mirar antes si existe deja una carrera. Crear usa `O_EXCL`. Un filesystem sin `RENAME_NOREPLACE` (algunos FUSE o NFS viejos) da error en vez de arriesgarse a pisar.
- Nada sale de la raíz: `VaultPath` no admite `..` ni rutas absolutas, y antes de operar se revisa con `lstat` que ninguna carpeta entre la raíz y la ruta sea un enlace simbólico; tampoco la papelera. Entre esa revisión y la operación queda una ventana si alguien cambia una carpeta por un enlace a propósito; cerrarla del todo pide `openat2(RESOLVE_BENEATH)` y no vale la pena mientras quien escribe en el vault sea su dueño.

### Medido (M2, 2026-10-17)

Benchmarks con `divan` 0.1.21, en release, **medido en la máquina de CI** (x86_64, 4 hilos, Linux, SSD, caché de disco caliente), no en la máquina de pruebas:

| Qué | Bench | Mediana |
|---|---|---|
| Abrir e indexar 5 000 notas + 100 adjuntos en 110 carpetas anidadas (walk + watcher puesto) | `cargo bench -p nota-vault` | **57 ms** (objetivo de M2: < 1 s) |
| Búsqueda difusa sobre esas 5 100 rutas, página de 50: `n` / `tema nota` / `a3t7n42` / `zzz` | `cargo bench -p nota-search` | 0.56 / 4.2 / 0.61 / 0.41 ms |

Falta el número con la caché de disco fría y en un vault real de Obsidian; el de la máquina de pruebas es el que vale para el hito.

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

- YAML 1.2 con `serde-saphyr` 2.0 + `serde` (por qué este crate: §2, Frontmatter YAML). Rutas XDG con `dirs` 7.0 (elegido sobre `etcetera` 0.11: no pide autor ni dominio para las rutas de macOS y Windows): `~/.config/nota/config.yaml`, `keymap.yaml`, `themes/`. Antes fue TOML; se cambió a YAML (2026-10-18) para que quien prefiera JSON lo pueda escribir tal cual.
- Config **por vault** opcional en `<vault>/.nota/` (no tocar `.obsidian/`; leerlo de solo lectura, p. ej. para respetar `attachmentFolderPath`, más adelante).
- Defaults embebidos con `include_str!`; config del usuario se fusiona encima; recarga en caliente vía `notify`.
- Validar y reportar errores de config en la UI, nunca hacer panic.

#### Estado (`nota-config`, 2026-10-18)

- Crate `nota-config`, sin GPUI. Rutas con `dirs` 7.0: en Linux la especificación XDG se resuelve a mano (`Paths::from_xdg`, con el entorno inyectado para probarla; una `XDG_*` relativa se ignora, como pide la especificación); en macOS y Windows, lo que `dirs` da para cada plataforma. `Paths::under(raíz)` lo pone todo en una carpeta, para tests.
- `config.yaml` (sin alias `config.yml`): tema, fuente (familia, tamaño), `scrolloff`, `tabstop`, `expandtab`, `ignorecase`, `smartcase` y vault por defecto; todo con default. `Settings::parse(texto)` es pura y sirve para la recarga: devuelve settings más diagnósticos y nunca falla. Un YAML roto (sintaxis o indentación), un tipo equivocado (también `on`/`yes` en un booleano) o un valor fuera de rango cambian el archivo entero por los defaults, con el error en archivo:línea:columna. Una clave desconocida es solo un aviso (vía `serde_ignored`, con su posición: una segunda pasada, solo si hubo claves ignoradas, ubica cada clave): se descartó `deny_unknown_fields` porque un typo costaría toda la config.
- Temas: paleta con nombres más grupos semánticos `ui`, `cursor` (por modo) y `syntax` (para M4). Un color semántico es un hex, un nombre de la paleta o `{ color, alpha }`; el alpha va aparte para que `0.5` sea exacto y el tema oscuro embebido dé los mismos `Hsla` que los colores fijos de antes (hay un test). Embebidos: `nota-oscuro` (default y respaldo) y `nota-claro`. Los del usuario, en `<config>/themes/*.yaml`, reemplazan a uno embebido del mismo nombre (y pueden extenderlo: un `extends` a su propio nombre va al embebido) y pueden usar `extends`; la paleta se fusiona primero, así que una referencia heredada usa la paleta final. Cadenas circulares, temas inexistentes, nombres de paleta desconocidos y colores faltantes son errores con posición; el tema que falla se cambia por `nota-oscuro`.
- El binario lee la config al arrancar y reporta los diagnósticos por el log; `nota-ui` toma colores y fuente del tema. Pendiente: mostrar los diagnósticos en la UI, el watcher de `notify` para la recarga en caliente, `keymap.yaml` y la config por vault.

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
│   ├── nota-buffer/        # Buffer (ropey), Edit, Transaction, History (con grupos de undo), Selections, LineAnchor (marks), newtypes de offset
│   ├── nota-syntax/        # tree-sitter-md + scanner de la sintaxis de nota (wikilinks, tags, embeds, frontmatter)
│   ├── nota-vim/           # Mode (Normal, Insert, Visual v/V, operador pendiente, `:` `/` `?`), Motion, Operator, TextObject, registros, macros, marks, VimState → Transaction; búsqueda `/ ? n N * #` y `:s` sobre `regex`. `"+`/`"*` vía el trait Clipboard que da la UI. Depende de nota-buffer y regex (+ nota-syntax para text objects md)
│   ├── nota-keymap/        # KeyEvent, trie, contextos y predicados, parseo YAML, registro de Actions
│   ├── nota-vault/         # hecho (M2): abrir un vault, índice de archivos, watcher, operaciones de archivo, guardado atómico; luego links, backlinks, caché redb
│   ├── nota-search/        # hecho (M2): fuzzy de rutas con nucleo-matcher; luego tantivy
│   ├── nota-config/        # settings, rutas XDG, temas
│   ├── nota-ui/            # vistas sobre el framework elegido (editor, file tree, picker, paleta)
│   └── nota/               # binario: main, wiring, CLI (`nota <vault>`; subcomandos sin GUI, ver 06 §7)
├── xtask/                  # tareas: generar fixtures, bench, release
└── tests/fixtures/vaults/  # vaults de ejemplo (incluir uno exportado de Obsidian real)
```

Estado (2026-10-18): existen `nota-buffer`, `nota-config`, `nota-syntax` (scanner de la sintaxis de nota y resolución de nombres, sin tree-sitter todavía), `nota-vim`, `nota-vault`, `nota-ui` y `nota`.

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
| Snapshot | `insta` 1.49 | Motor vim: `"hello |world" + "dw" → "hello |"` como snapshot legible. Scanner de sintaxis: links/tags por fixture. Resolución de links por vault de prueba. |
| Propiedades | `proptest` 1.12 | Invariantes: undo∘redo = id; cada número de secuencia del historial muestra siempre el mismo texto, se llegue por undo, redo, `g-` o `g+`; aplicar `Transaction` y su inversa deja el rope igual; offsets siempre en frontera de char; `Selections` siempre ordenadas y sin solaparse; reindex incremental == reindex desde cero. |
| Tabla | `rstest` 0.27 | Casos parametrizados de motions/text objects. |
| Diff legible | `pretty_assertions` 1.4 | |
| Runner | `cargo-nextest` 0.9.148 | Paralelo, rápido en CI. |
| Bench | `divan` 0.1.21 (elegido en M2: menos dependencias que `criterion` 0.8.2 y sin harness propio que configurar) | Indexar vault de 10 k notas, latencia por tecla. Guardar baseline. |
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

- **`cargo-dist`** (hoy `dist`, 0.33.0 en sep-2026): planea y construye releases en CI, tarballs, installer shell, Homebrew, MSI. **No genera AppImage ni Flatpak.** En uso desde 2026-10-17: Linux x86_64 y aarch64, tags `v*`, sin installer shell (ver `docs/release.md`).
- **AppImage**: `cargo-appimage` 2.4 (poco uso) o `linuxdeploy` en un job aparte. Las apps GPU (Vulkan/wgpu) dentro de AppImage dan guerra con drivers **(sin verificar para GPUI)**.
- **Flatpak**: manifest propio + `flatpak-cargo-generator` para vendorear deps; es el camino para Flathub. Permiso `--filesystem=home` o portal de documentos para abrir vaults.
- **AUR**: PKGBUILD a mano en `packaging/aur/`: `nota-git` desde la rama main y `nota-bin` desde el tarball de dist. El nombre `nota` queda para un paquete desde el tarball de fuente de un release. `cargo-aur` 1.7 existe pero está casi muerto (885 descargas recientes).
- `.deb`/`.rpm`: `cargo-deb` 3.8, `cargo-generate-rpm` 0.21.
- Archivo `.desktop`, icono y metainfo AppStream en `packaging/linux/`; van en cada tarball y el CI los valida.

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
- La vista pinta solo las líneas visibles, guarda el scroll en líneas (04, riesgo 6) y en Insert y en la línea de comandos el texto entra siempre por el input handler (04, riesgo 4). Colores y tamaños salen de `Theme`, cuyos colores vienen del tema de `nota-config` (§5, "Config"); la fuente es la de la config si está instalada y si no la primera monoespaciada instalada de una lista, porque GPUI entra en pánico con una familia que no existe.
- `nota-vim` tiene una línea de comandos mínima: `:w`, `:q`, `:q!`, `:wq`. El motor no hace I/O: devuelve el `Command` y la vista invoca la acción con nombre (`editor::save`, `editor::quit`, …).
- "Modificado" compara el estado del historial (`Buffer::history_seq`) con el guardado, más las ediciones de un Insert sin cerrar: como en vim, deshacer o `g-` hasta lo guardado quita el `[+]`, y llegar al mismo texto por otra edición no.
- Smoke sin GPU, bajo `xvfb-run` (lavapipe), con XTest tecleando: archivo nuevo → `i…<Esc>:wq` lo crea; con cambios, `:q` y el cierre pedido por el gestor de ventanas se niegan; `:w` conserva permisos 0640; `:q!` sale sin guardar; no quedan temporales. Primer frame en release: 631 ms con un archivo chico y 762 ms con 50 MB (el spike midió 710 ms aquí y 293 ms en la máquina de pruebas). El número que vale es el de la máquina de pruebas.
- Fuera de M0: autosave, journal de recuperación, conflictos con cambios externos, preservar el dueño del archivo.

### Estado (M2, buffers y splits, 2026-10-18)

- `nota-ui` separa un modelo sin GPUI (`workspace/`) de las vistas. El `Workspace` tiene el registro de documentos (cada uno con su buffer, su historial y su estado de guardado; un archivo se abre una sola vez aunque se escriba con otra ruta), el árbol de paneles con los cursores de cada panel y **un solo motor vim para todos**, como en vim, donde modo, registros, `.` y `;` son globales: lo copiado en un buffer se pega en otro. Cambiar de panel termina lo que estuviera a medias (Insert, operador, `:`) como un `ctrl-c`. Todo eso se prueba sin ventana; la vista solo crea un `Editor` por hoja del árbol y mueve el foco de GPUI. El borde entre paneles, el cursor y la barra de un panel sin foco son colores del tema (`ui.pane_border`, `cursor.unfocused`, `ui.status_unfocused_foreground`).
- Dos paneles pueden mostrar el mismo buffer con cursores independientes. `nota-buffer` anota cada reemplazo hecho (también los de undo y redo, `Buffer::take_splices`) y los cursores de los otros paneles los siguen, como hace vim con `mark_adjust`.
- `:e ruta` abre en el panel activo; una ruta relativa parte del directorio de trabajo, como en vim. `:e` sin ruta relee el archivo y `:e!` descarta los cambios. Como vim sin `hidden`, un documento vive mientras algún panel lo muestre: `:e` y `:q` se niegan si se perderían cambios, salvo que otro panel siga mostrando ese buffer. Cerrar la ventana (`:q` en el último panel o el gestor de ventanas) mira todos los buffers (`E37`, o `E162` si los cambios están en otro). `ctrl-w o` deja los paneles que se llevarían cambios (`E445`).
- Splits: `ctrl-w v`/`:vs` (el nuevo a la derecha), `ctrl-w s`/`:sp` (abajo), `ctrl-w h/j/k/l` (el vecino de ese lado que comparte más borde), `ctrl-w w` (siguiente, en el orden de vim), `ctrl-w q`/`:q`, `ctrl-w o`/`:on`. El espacio se reparte en partes iguales; no hay redimensionar. Un proptest del árbol comprueba que, haga lo que se haga, queda normalizado, cubre la ventana y el foco está en un panel que existe.
- Diferencias con vim: el panel nuevo de un split va a la derecha o abajo con el foco (`splitright`/`splitbelow`); `:e` de un archivo ya abierto toma el cursor de un panel que lo muestra; tras `:e` el historial de undo empieza de nuevo; no se expanden `%`, `#` ni `~` y los espacios de la ruta no se escapan; el count de `ctrl-w` se ignora; las marks son de cada buffer, también `''` (en vim es de cada ventana), y se pierden al cerrar el buffer o releerlo con `:e!`.
- El smoke sin GPU ya es un script, `scripts/smoke-x11.sh` (Xvfb + lavapipe + xdotool): los casos de M0 más `:e` (se niega con cambios, `:e!` descarta) y `ctrl-w v` (`:q` cierra de a un panel; el último no cierra si otro buffer tiene cambios). El cierre pedido por el gestor de ventanas no está en el script: sin gestor de ventanas en Xvfb no hay quien lo pida.

| Hito | Contenido | Hecho cuando… |
|---|---|---|
| **M0 — Ventana + texto** ✅ (2026-10-17) | Workspace, CI, `nota-buffer` (Buffer, Edit, History), ventana con un buffer editable (sin modo), abrir/guardar un archivo con escritura atómica. | Abres `nota archivo.md`, editas, guardas; proptest de undo/redo en verde; arranque al primer frame ≈ 300 ms. El objetivo original de < 100 ms no era realista: el spike midió 244–297 ms en la máquina de pruebas y casi todo es GPUI/Vulkan levantando la ventana (cargar 50 MB son 71 ms, un archivo chico 2 ms). |
| **M1 — Vim normal/insert** ✅ (2026-10-17) | `nota-keymap` (trie + contextos), `nota-vim`: normal, insert, motions básicos (`hjkl wbe 0^$ gg G f t`), operadores `d c y` + `p`, `x`, `u`/`ctrl-r`, `.`, counts, text objects `iw aw i" i( ip`. Cursor bloque/línea por modo. | ~150 casos de tabla verificados contra nvim headless. Cerrado con 488 casos, cada uno corrido también en nvim. El keymap (tabla tipada con contextos) sigue dentro de `nota-vim` hasta que haya un segundo consumidor (§3, "Estado"). |
| **M2 — Vault + finder** (en curso) | `nota-vault`: abrir carpeta, walk con `ignore`, watcher, árbol de archivos con navegación vim, fuzzy finder (`nucleo`) en `space f`, paleta de comandos, `:e`, `:w`, `:q`, splits `ctrl-w`. Hecho: varios buffers, `:e`/`:e!`, splits `ctrl-w` y `:q`/`:wq` por panel (ver "Estado (M2…)"). | Abre una carpeta real de notas markdown (≥ 5 k notas, por ejemplo un vault de Obsidian) en < 1 s **(objetivo)**; crear/renombrar/borrar desde el árbol. Núcleo hecho (2026-10-17, §4 "Estado"): índice, watcher, búsqueda y operaciones, sin interfaz; 5 k notas sintéticas abren en 57 ms, medido en la máquina de CI. Falta el árbol, `space f` y la paleta en la UI. |
| **M2.5 — Tags** | Los tags son el eje de organización ([05](05-tags.md)). `nota-syntax` saca los tags del texto (`#a/b`) y de `tags:` del frontmatter, sin contar los de código (✅ 2026-10-18, salvo el código indentado y el rango de cada tag del frontmatter); `TagIndex` en `nota-vault`, incremental sobre el watcher; árbol de tags en el panel (jerárquico por `/`, con conteo) que alterna con el de carpetas; `space f` filtrable por tag (`#rust -#archivado`); modo focus por carpeta o tags (`:focus`, desde el panel, indicador en la barra de estado); autocompletado de `#`; renombrar y fusionar tags en todo el vault con vista previa y deshacer. Junto con M2.5 o justo después **(propuesta)**: la CLI (`nota add`, `nota search`, `nota tags`, con `--json`) y el buzón, la vista de notas sin tags ([06](06-decisiones-de-fondo.md) §7 y §9). | Con el vault sintético de 5 k notas de M2: índice de tags en < 200 ms al abrir y `space f` filtrado por tag en < 16 ms **(objetivos)**; reindex incremental == completo (proptest); casos borde del scanner y de las consultas en `insta`; renombrar `#a` → `#b` (con hijos y frontmatter) y deshacer deja cada archivo byte a byte igual. |
| **M3 — Wikilinks + backlinks** | `nota-syntax` (scanner de wikilinks y links + frontmatter) ✅ (2026-10-18), índice de links, resolución de nombres (la función pura ✅; falta el índice), `gd`/`enter` sigue link, autocompletado de `[[`, panel de backlinks (con vim), links no resueltos, renombrar nota actualiza links. | Fixture de resolución en `insta`; reindex incremental == completo (proptest). |
| **M4 — Live preview** | tree-sitter-md incremental; ocultar marcadores fuera de la línea del cursor, headings con tamaño, negrita/cursiva, links como texto clicable, listas y checkboxes (`ctrl-enter`/`gx` alterna), código con resaltado, `==highlight==`, comentarios `%%`. Toggle source/live/lectura. | Escribir en una nota de 5 k líneas sin latencia perceptible (< 16 ms por tecla **objetivo**). |
| **M5 — Embeds y callouts** | `![[img]]` imágenes, `![[nota]]` transclusión (solo lectura), callouts, tablas renderizadas. | El vault de prueba se ve "como en Obsidian" en lo común. |
| **M6 — Búsqueda y pulido** | Búsqueda full-text (grep paralelo → `tantivy` si hace falta), visual mode completo (✅ `v` y `V` en el motor, 2026-10-18; falta pintar la selección y `ctrl-v`), registros ✅, macros ✅, `:%s`, marks ✅ (falta `A`–`Z`), keymap y tema del usuario en YAML, autosave + journal de recuperación, caché `redb` para arranque en caliente. Historial de versiones por nota (`:history`, [06](06-decisiones-de-fondo.md) §3) **(propuesta)**. | Primer release con cargo-dist + AUR. Listo para el tag (2026-10-17): dist, `nota --version`, `.desktop` + metainfo, PKGBUILD de `nota-bin` y `nota-git`, CHANGELOG y `docs/release.md`. Falta publicar. |
| Post-MVP | Multi-cursor, vista de grafo, daily notes/plantillas, math, export HTML/PDF, plugins WASM. | — |

---

## Crates verificados (crates.io, 2026-10-15; los de M2 de nuevo el 2026-10-17)

`ropey 1.6.1` · `crop 0.4.3` · `lapce-xi-rope 0.4.0` · `zed-sum-tree 0.2.0` · `pulldown-cmark 0.13.4` · `comrak 0.56.0` · `markdown 1.0.0` · `tree-sitter 0.27.1` · `tree-sitter-md 0.5.3` · `tree-sitter-highlight 0.27.1` · `modalkit 0.0.31` · `editor-types 0.0.6` · `keybindings 0.0.4` · `ignore 0.4.33` · `notify 8.2.0` (la 9.0 sigue en rc) · `notify-debouncer-full 0.7.0` (la 0.8 en rc) · `nucleo 0.5.0` · `nucleo-matcher 0.3.1` · `frizbee 0.13.0` · `tantivy 0.26.2` · `redb 4.4.0` · `rusqlite 0.40.2` · `petgraph 0.8.3` · `tempfile 3.27.0` · `toml 1.1.8` · `serde_norway 0.9.42` · `yaml-rust2 0.13.0` · `serde-saphyr 2.0.0` (2026-10-18) · `granit-parser 2.0.0` · `saphyr 0.2.2` · `serde_yaml_ng 0.10.0` · `noyalib 0.0.58` · `etcetera 0.11.0` · `dirs 7.0.0` · `crossbeam-channel 0.5.17` · `flume 0.12.0` · `rayon 1.12.0` · `arc-swap 1.9.2` · `rustix 1.1.5` · `divan 0.1.21` · `criterion 0.8.2` · `trash 5.2.9` · `smallvec 1.16.3` · `smol_str 0.3.6` · `unicode-segmentation 1.13.3` · `unicode-width 0.2.2` · `unicode-normalization 0.1.26` (2026-10-18) · `caseless 0.2.2` (2026-10-18) · `regex 1.13.1` (2026-10-18) · `regex-automata 0.4.18` · `similar 3.2.0` · `imara-diff 0.2.0` · `wasmtime 49.0.3` · `insta 1.49.0` · `proptest 1.12.0` · `rstest 0.27.0` · `pretty_assertions 1.4.1` · `cargo-nextest 0.9.148` · `cargo-deny 0.20.2` · `cargo-machete 0.9.2` · `typos-cli 1.51.1` · `cargo-dist 0.32.0` · `cargo-deb 3.8.0` · `cargo-generate-rpm 0.21.0` · `cargo-appimage 2.4.0` · `cargo-aur 1.7.1` · `gpui 0.2.2` · `gpui-component 0.7.3` · `iced 0.14.0`.

Descartados o trampas: `vim-like` (no existe), `helix-core` (placeholder 0.0.0), `tree-sitter-markdown` (otro crate, 2021), `xi-rope` (2019, usar `lapce-xi-rope`), `serde_yaml` (deprecado), `serde_norway` y `serde_yaml_ng` (libyaml: YAML 1.1), `serde_yml` (deprecado, reenvía a `noyalib`), `zed-rope` (no publicado).
