# 05 — Tags

> Diseño del 2026-10-18. Decisión de producto: **los tags son el eje principal de organización de nota, por encima de las carpetas.** También están decididos cómo se escribe un tag (mayúsculas, acentos, cómo se muestra, §6), que **los tags son planos** y la jerarquía sale solo de los `parents` de las páginas de tag (§1, §7), y que todo se puede hacer desde la UI sin editar archivos a mano ([06](06-decisiones-de-fondo.md) §13). Lo que no venía de esas decisiones va marcado **(propuesta)** y queda pendiente de confirmar.
> Construye sobre [03](03-core-architecture.md): el scanner de `nota-syntax` (§2), el índice y el watcher de `nota-vault` (§4) y el modelo de acciones con nombre (§3). Entra en el roadmap como **M2.5** (03 §8).

## TL;DR

- Una carpeta obliga a elegir un solo lugar para cada nota; una nota casi siempre pertenece a varias cosas. Los tags no obligan a elegir. Las notas siguen siendo archivos markdown en carpetas, pero la UI y el modelo tratan a los tags como ciudadanos de primera clase.
- **Un solo sistema de tags**: los `#tag` del texto y la lista `tags:` del frontmatter son lo mismo. Viven en el archivo, que es la fuente de verdad; el índice de tags es caché regenerable.
- **Los tags son planos.** Un tag es solo un nombre: `#gpui`. La `/` es un carácter más del nombre (`#2024/10` es un tag) y no crea padres ni hijos.
- **Página de tag opcional**: `tags/<nombre>.md`, en una carpeta plana y configurable. Su frontmatter lleva `color`, `icon`, `aliases` y `parents`; su cuerpo dice de qué trata el tag. Un tag sin página funciona igual (§7).
- **La jerarquía sale solo de `parents`**, y es un grafo: `gpui` puede estar bajo `rust`, `graficos` y `ui` a la vez. `#rust` trae también las notas de `#gpui`, de forma transitiva. Los ciclos se avisan y no rompen nada.
- **Se usa desde la UI**: clic derecho sobre un tag (en el panel o en el texto) cambia color, ícono, alias y padres, renombra, fusiona y abre su página. nota crea la página sola; el archivo es el almacenamiento, no la interfaz (§3).
- **Panel lateral** con dos vistas, tags (en grafo por `parents`, con conteo) y carpetas, y una tecla para cambiar. Por defecto, tags **(propuesta)**.
- **Modo focus**: enfocar toda la app (finder, búsqueda, panel, backlinks) en una carpeta, un tag o una consulta de tags. `:focus #rust`, `:focus proyectos/`, o desde el panel.
- **Consultas**: `#rust #nota -#archivado`. Espacio es "y", `-` es "no", un tag incluye a sus alias y a sus descendientes por `parents`. Nada más.
- **Operaciones**: autocompletado de `#`, renombrar un tag en todo el vault (con su página y los `parents`/`aliases` que lo nombran, con vista previa y deshacer), fusionar dos tags.
- `#Rust`, `#rust` y `#RUST` son el mismo tag, y se muestra como lo escribe la mayoría; `#canción` y `#cancion` son distintos (§6).

---

## 1. De dónde sale un tag

Dos fuentes, un solo tag:

| Fuente | Ejemplo | Rango que se guarda |
|---|---|---|
| Texto | `Hoy probé #gpui en #nota` | Del `#` al último carácter del nombre |
| Frontmatter | `tags: [gpui, nota]` o `tags:` con lista en bloque | El del valor dentro del YAML (`Spanned<T>` de `serde-saphyr`, 03 §2) |

Al escribir se pone solo el tag (`#gpui`), sin ruta: que `gpui` esté bajo `rust` lo dice la página de `gpui` (§7), no cada nota.

Reglas del texto (las del scanner de `nota-syntax`, 03 §2):

- Empieza al inicio de línea o tras un espacio: `texto#algo` y el fragmento de una URL no son tags.
- Letras, números, `_`, `-` y `/`; fuera de ASCII, letras, marcas, números y símbolos (emoji incluidos). La puntuación lo corta: `#rust.` es `#rust`.
- **Decisión: puede ser solo números.** `#2024`, `#1` y `#2024/10` son tags, porque en nota los tags son el eje de organización y `#2024` sirve para agrupar un año; la regla contraria venía de Obsidian. Costo aceptado: en texto como "PR #12", `#12` se vuelve tag.
- `# Título` es un heading, no un tag: el espacio lo decide.

Reglas del frontmatter:

- Solo la clave en plural, `tags` (03 §2). Vale una lista o un solo texto; un `#` delante del valor se ignora (`tags: ["#rust"]` es `rust`) **(propuesta)**. Un texto con comas o espacios no se parte: `tags: "rust, nota"` no es un tag válido y se marca como problema recuperable **(propuesta)**; partirlo adivina la intención y luego renombrar tendría que reescribir dentro de un texto que el usuario escribió entero.
- Los `#` en otras claves del YAML no son tags.

**La `/` no es jerarquía.** `#2024/10` es un solo tag plano, distinto de `#2024` y sin relación con él; si `2024/10` debe aparecer bajo `2024`, su página lo dice con `parents` (§7). Se mantienen dos reglas del scanner, ahora solo léxicas: una `/` final se descarta como si fuera puntuación (`#proyecto/` es `#proyecto`) y dos `/` seguidas invalidan el tag (`#a//b`) **(propuesta)**. Las dos evitan que un fragmento de ruta escrito en el texto se vuelva tag.

## 2. Modelo de datos

### Dónde vive cada pieza

| Crate | Qué pone |
|---|---|
| `nota-syntax` | El scanner: de un `&str` saca las ocurrencias de tags (texto y frontmatter) con su rango en bytes, y de una página de tag sus campos (`color`, `icon`, `aliases`, `parents`) con sus rangos. Funciones puras, sin estado. |
| `nota-vault` | El `TagIndex`, el `TagGraph` de las páginas, la clave normalizada (`TagKey`), las consultas (`TagQuery`), el alcance de focus (`Scope`), la refactorización de renombrar/fusionar y la escritura de metadata desde la UI. Nada de esto toca la GUI **(propuesta)**. |
| `nota-search` | Sin cambios: el finder recibe la lista de candidatos ya filtrada por el `Scope`. Sigue sin depender de `nota-vault` (03 §4). |
| `nota-ui` | El panel de tags, el menú contextual y los selectores (§3), el indicador de focus y el autocompletado. Solo pinta y llama acciones. |

### Tipos

```rust
struct TagKey(SmolStr);              // normalizada (§6): NFC + case folding, sin el '#'
enum TagSource { Inline, Frontmatter }
struct TagOccurrence { key: TagKey, spelling: SmolStr, range: Range<usize>, source: TagSource }

struct TagIndex {
    by_tag:  HashMap<TagKey, TagEntry>,                       // clave tal como se escribió en las notas
    by_note: HashMap<NoteId, SmallVec<[TagOccurrence; 4]>>,
}
struct TagEntry {
    notes: BTreeSet<NoteId>,                                   // las que llevan este tag exacto
    spellings: SmallVec<[(SmolStr, u32); 1]>,                  // cómo se escribe y cuántas veces
}

struct TagGraph {                                              // sale solo de las páginas de tag (§7)
    pages:    HashMap<TagKey, TagPage>,
    alias_of: HashMap<TagKey, TagKey>,                         // alias → tag con página
    children: HashMap<TagKey, SmallVec<[TagKey; 2]>>,          // inverso de `parents`
    problems: Vec<TagProblem>,                                 // choques de alias, ciclos, padres sueltos
}
```

- `TagKey` es la identidad: agrupar, filtrar, consultar, focus y el panel usan solo la clave. `spelling` es lo que el usuario escribió y solo sirve para mostrar (§6).
- Este índice reemplaza al `tags: BTreeMap<SmolStr, Vec<NoteId>>` que 03 §4 dejó dentro de `LinkIndex`: tags y links se indexan del mismo scan pero se consultan por separado **(propuesta)**.
- `by_note` guarda los rangos para que renombrar no tenga que reescanear el vault (§5), igual que `Link` guarda `range` para renombrar notas (03 §4).
- **Índice y grafo separados (propuesta).** El `TagIndex` guarda lo que las notas escriben; el `TagGraph` dice qué significa. Resolver un alias o un padre se hace al consultar, así que cambiar una página no obliga a reescanear ninguna nota.
- **Descendientes**: los de `a` son el cierre de `children` desde `a`, con un conjunto de visitados para que un ciclo no dé vueltas (§7). Filtrar por `#a` es la unión de las notas de `a`, de sus alias y de cada descendiente (y de sus alias).
- **Conteo del panel**: número de notas **distintas** en `a` y sus descendientes, no la suma de los hijos: una nota con `#gpui` y `#tokio` cuenta una vez en `rust`; una nota bajo `gpui` que también está bajo `ui` cuenta una vez en cada uno **(propuesta)**. Se calcula al construir la vista del panel y se recalcula solo en los tags afectados por el cambio.

### Construcción e incremental

- **En frío**: el mismo walk de `Vault::open` lista las notas; el escaneo de tags va en el pool de fondo, en paralelo por nota, y el índice se publica cuando termina. Las páginas de tag se escanean en el mismo walk y el `TagGraph` se construye después, en una pasada **(propuesta)**. La UI abre antes: el panel muestra las carpetas y los tags aparecen al llegar el índice **(propuesta)**.
- **Incremental**: el `TagIndex` y el `TagGraph` se suscriben a los `VaultUpdate` del watcher (03 §4). En una nota: `Created`/`Modified` → reescanear esa nota y diferenciar sus ocurrencias viejas y nuevas; `Removed` → quitar la nota; `Renamed` → solo cambia la ruta, el `NoteId` y sus tags siguen. En una página: rehacer el `TagGraph` entero, que es barato porque solo tiene las páginas **(propuesta)**. Se publica igual que el índice de archivos (snapshot en `ArcSwap` + evento `IndexEvent::TagsChanged(Vec<TagKey>)`), para que la UI lea sin bloquear.
- **Buffers abiertos**: el índice refleja el disco, no el buffer sin guardar. El tag que escribes aparece en el panel al guardar **(propuesta)**. Indexar cada tecla haría que el panel baile mientras escribes `#ru…` y obligaría a tener dos verdades.
- **Invariante** (proptest, como el de archivos): índice y grafo incrementales tras cualquier secuencia de crear, editar, mover y borrar notas y páginas == índice y grafo construidos de cero sobre el resultado.

## 3. UI

### Una GUI, no una TUI

nota es una GUI con navegación estilo vim ([06](06-decisiones-de-fondo.md) §13). Para los tags, eso quiere decir:

- **Clic derecho sobre un tag**, en el panel o en el texto (un `#tag` del cuerpo o un valor de `tags:`), abre un menú con: color (selector de color), ícono (selector de emoji), alias, padres, renombrar (§5), fusionar (§5) y abrir su página (§7). Cada entrada es una acción con nombre (`tags::set_color`, `tags::set_icon`, `tags::edit_aliases`, `tags::edit_parents`, `tags::rename`, `tags::merge`, `tags::open_page`) **(propuesta para los nombres)**, y por eso también se llega desde la paleta, desde `:` y con atajos. Con el cursor sobre un tag, la misma lista sale con una tecla **(propuesta: `space T`)**.
- **nota crea la página sola** en cuanto le pones metadata desde la UI: elegir un color para `#rust` escribe `tags/rust.md` con `color:` en el frontmatter. El usuario nunca necesita crear la página a mano, aunque puede.
- **Escribir un `#tag-nuevo` no crea página.** `tags/` se llena solo con los tags que de verdad tienen algo que decir.
- **El archivo es el almacenamiento, no la interfaz.** Lo que se cambia desde la UI se escribe en el frontmatter de la página, tocando solo el campo editado (regla de `CLAUDE.md`), y editar ese frontmatter a mano da el mismo resultado.
- Quitar toda la metadata desde la UI no borra la página: puede tener cuerpo, y borrar un archivo del usuario es una decisión suya **(propuesta)**.
- Al crear un tag que no existía, un aviso no intrusivo junto al tag ofrece darle color o ícono, y se ignora sin hacer nada **(propuesta)**.

### Panel lateral

Dos vistas en el mismo panel, tags y árbol de carpetas (el de M2). Una sola está visible a la vez.

- **Por defecto: tags (propuesta).** Es el eje de organización; abrir la app en la vista de carpetas diría lo contrario. La vista elegida se recuerda por vault **(propuesta)**.
- Cambiar de vista: `T` dentro del panel (`panel::toggle_view`) **(propuesta)**; `space t` abre el panel en tags y `space e` en carpetas (`panel::show_tags`, `panel::show_folders`) **(propuesta)**. El encabezado del panel dice cuál es.
- **Raíces**: los tags sin `parents`. Bajo un tag, primero los tags que lo tienen como padre y luego sus notas, como una carpeta con subcarpetas y archivos **(propuesta)**. Un tag con tres padres aparece bajo los tres, y una nota con tres tags en tres lugares: es justamente lo que una carpeta no permite.
- Cada nodo muestra el nombre entero (`2024/10`, no `10`), su ícono y su color si los tiene, y el conteo.
- Al final, el **buzón**: las notas que no tienen ningún tag ([06](06-decisiones-de-fondo.md) §9). Sin él, una nota sin tags es invisible en la vista por defecto. Las páginas de tag nunca están en el buzón (§7).
- Cada nodo muestra la forma establecida del tag (§6): "Rust (10)" aunque una nota diga `#rust`.
- Orden: alfabético por la forma que se muestra, sin mayúsculas, como el de carpetas **(propuesta)**.
- Vim, igual que `FileTree` (03 §3): `j/k`, `h/l` colapsar/expandir, `gg/G`, `/` filtrar, `enter` abre la nota o la vista del tag (§7), `r` renombra el tag (§5), `F` enfoca (§3, focus) **(propuesta para `enter`, `r` y `F` aquí)**. El contexto es `TagTree`. Con el ratón: clic abre, clic en el triángulo expande, clic derecho abre el menú del tag.

### Modo focus

Inspirado en el "workspace view" de Inkdrop: enfocarse reduce **toda la app** a un subconjunto del vault.

**Qué es un alcance (`Scope`)**: una carpeta, una consulta de tags (§4) o las dos a la vez (`:focus proyectos/ #rust`), que se intersectan **(propuesta para la combinación carpeta + tags)**.

**Cómo se activa**:

| Desde | Cómo | Acción |
|---|---|---|
| `:` | `:focus #rust`, `:focus #rust #nota -#archivado`, `:focus proyectos/` | `workspace::focus` |
| Panel | `F` o el menú contextual sobre un tag o una carpeta | `workspace::focus` con ese nodo |
| Salir | `:unfocus`, `F` sobre el mismo nodo o clic en el indicador **(propuesta)** | `workspace::clear_focus` |

Una carpeta se distingue de un tag porque no empieza con `#`; el `/` final es opcional. `:focus` sin argumentos muestra el alcance actual en la línea de comandos **(propuesta)**. Autocompletado de tags y carpetas en `:focus`.

**Qué cambia dentro de focus**:

| Parte | Comportamiento |
|---|---|
| Finder (`space f`) | Solo candidatos del alcance. |
| Búsqueda full-text (M6) | Solo dentro del alcance. |
| Panel, tags | Solo los tags que aparecen en notas del alcance, con el conteo **dentro** del alcance **(propuesta)**: en focus `#rust`, `#async` dice cuántas notas de rust lo llevan. |
| Panel, carpetas | Solo carpetas con alguna nota del alcance; las vacías se esconden **(propuesta)**. |
| Backlinks (M3) | Solo los que vienen de notas del alcance, más una línea "N fuera de focus" que los muestra al abrirla **(propuesta)**. Esconderlos sin avisar haría creer que no existen. |
| Nota nueva | En focus de carpeta, se crea en esa carpeta. En focus de tags, nace con los tags incluidos (no los excluidos) en `tags:` del frontmatter **(propuesta)**; si no, desaparecería del focus en cuanto la creas. |
| Abrir una nota fuera del alcance | Se puede (`:e`, un link): focus filtra lo que se ofrece, no lo que se puede abrir **(propuesta)**. |

**Cómo se ve**: un indicador fijo en la barra de estado con el alcance escrito tal cual (`focus: #rust -#archivado`) y el encabezado del panel con el mismo texto, ambos con un token de tema propio (`ui.focus_indicator`). El finder lo repite en su prompt. Nunca debe haber duda de que estás viendo una parte del vault.

El focus dura la sesión; al reabrir, la app empieza sin focus **(propuesta)**. Recordarlo entre sesiones va a `IDEAS.md`.

## 4. Consultas

Sintaxis mínima, la misma en `space f`, en `:focus` y en el filtro del panel:

| Pieza | Significa |
|---|---|
| `#a` | La nota lleva `a`, un alias de `a` o un descendiente de `a` por `parents` (§7). |
| `#a #b` | Las dos (espacio = "y"). |
| `-#a` | No lleva `a`, ni sus alias, ni sus descendientes. |
| `carpeta/` | Está bajo esa carpeta (solo en `:focus`, §3). |
| cualquier otra palabra | En `space f`, texto para el fuzzy de rutas, como hoy. |

- Ejemplo: `#rust #nota -#archivado` = notas de rust **y** de nota que **no** están archivadas.
- En `space f` se mezclan: `#rust parser` filtra por `#rust` y busca `parser` con fuzzy entre lo que queda **(propuesta)**.
- El tag de la consulta se compara con la clave normalizada (§6): `#Rust` encuentra `#rust`. Un alias en la consulta vale por su tag: `#rs` es `#rust`.
- `#2024/10` es un tag como cualquier otro: no trae ni excluye nada de `#2024` salvo que su página lo ponga como padre.
- Un `-#a` solo, sin ningún `#` positivo, vale: "todo menos `a`".
- Las páginas de tag no son resultados, salvo que se pida (§7).
- **No hay "o"**, ni paréntesis, ni comodines **(propuesta)**. Cubren casos reales pero convierten esto en un lenguaje; si hacen falta, se ven junto con las vistas guardadas (`IDEAS.md`).
- Un tag que no existe en el vault no es error: da cero resultados y el prompt lo marca en el color de aviso del tema.
- En código, `TagQuery { include: Vec<TagKey>, exclude: Vec<TagKey> }` y `Scope { folder: Option<VaultPath>, tags: TagQuery }`, con `fn notes(&self, &TagIndex, &TagGraph, &VaultIndex) -> NoteSet`. El parseo es una función pura con tabla de casos en `insta`.

## 5. Operaciones

### Autocompletado de `#`

- En Insert, al escribir `#` donde puede empezar un tag (inicio de línea o tras espacio) seguido de un carácter de tag, se abre la lista de tags del vault, filtrada con `nucleo-matcher` y ordenada por número de notas **(propuesta)**. La lista muestra el ícono y el color de cada tag.
- Los alias también se buscan: escribir `#rs` ofrece "Rust (alias rs)" y acepta `#rust` **(propuesta)**.
- También dentro de la lista `tags:` del frontmatter y de `parents` en una página de tag.
- No aparece en bloques de código ni en código inline. `# ` con espacio cierra la lista: es un heading.
- La lista ofrece e inserta la **forma establecida** (§6), no la que escribiste a medias: escribir `#ru` y aceptar deja `#Rust` si así lo escribe el vault. Así no nacen variantes nuevas. `ctrl-n`/`ctrl-p` y `tab` como en el resto de pickers **(propuesta para las teclas)**; clic en una opción también la acepta.

### Renombrar un tag en todo el vault

Es una refactorización: reescribe archivos del usuario. Por eso va en tres pasos, nunca en uno.

1. **Pedir**: `:tagrename #viejo #nuevo` (`tags::rename`), `r` sobre el tag en el panel o "Renombrar" en el menú contextual **(propuesta para el nombre del comando)**.
2. **Vista previa**: una lista, como un picker, con cada cambio (archivo, línea, antes → después) y el total de archivos y ocurrencias. Entran las ocurrencias en notas, el renombre de la página `tags/viejo.md` → `tags/nuevo.md` y cada `parents` o `aliases` de otra página que nombra al tag. Se navega con `j/k` o el ratón, `enter` abre esa línea, `x` quita ese cambio **(propuesta)**. `:w` o el botón "Aplicar" aplica; `Esc`, `:q` o "Cancelar" cancela sin tocar nada **(propuesta)**.
3. **Aplicar**: cada archivo se reescribe solo en los rangos de `by_note` (y en los de las páginas), con escritura atómica; la página se renombra con un rename atómico. Se valida antes que el archivo en disco no cambió desde el escaneo (mismo `mtime`+tamaño y hash); si cambió, se reescanea ese archivo y se avisa en vez de escribir a ciegas.

Detalles:

- **Sin descendientes por nombre**: renombrar `#a` a `#b` no toca `#a/x`, que es otro tag. Los hijos de `a` siguen bajo `b` porque sus `parents` se actualizan en el mismo cambio.
- **Alias**: las ocurrencias escritas con un alias (`#rs`) no se reescriben y el alias se queda en la página renombrada: renombrar cambia el nombre del tag, no sus alias **(propuesta)**.
- **Frontmatter**: se reemplaza solo el texto del valor en su rango, sin re-serializar el YAML; el resto del bloque queda byte a byte igual (regla de `CLAUDE.md`: nunca reescribir el YAML del usuario salvo el campo editado). Vale para `tags:` en notas y para `parents`/`aliases` en páginas.
- **Mayúsculas**: renombrar reescribe **todas las variantes** del tag (`#Rust`, `#rust`, `#RUST`) a la forma nueva. Renombrar `#Rust` a `#rust` es válido aunque la clave sea la misma: es la manera de unificar cómo se escribe un tag. Es la única vez que nota toca la forma escrita; mostrar la forma establecida nunca reescribe archivos.
- **Nombre nuevo sin página posible**: si el tag tiene página y el nombre nuevo no puede ser nombre de archivo (`#2024/10`, §7), la vista previa lo marca y no se aplica **(propuesta)**.
- **Buffers abiertos con cambios sin guardar**: el cambio se aplica al buffer como una transacción (entra en su undo) y el archivo queda modificado sin guardar; no se escribe el disco por debajo del buffer **(propuesta)**. Buffers abiertos sin cambios: se escribe el disco y el buffer se recarga.
- **Deshacer**: la operación entera, renombre de la página incluido, es una sola transacción de vault, con el contenido previo de cada archivo tocado guardado en memoria durante la sesión. `:tagundo` (`tags::undo_rename`) o el botón "Deshacer" del aviso que queda al aplicar la revierte **(propuesta para el nombre)**. Si un archivo cambió después del renombre, ese archivo no se revierte y se lista; los demás sí. Es el mismo motor que necesita M3 para "renombrar nota actualiza links" (03 §4), y se diseña una vez para los dos **(propuesta)**.
- **Fallo a medias**: cada archivo es atómico, el conjunto no. Si una escritura falla, se para, se informa qué quedó escrito y se ofrece deshacer lo aplicado.

### Fusionar dos tags

- `:tagmerge #a #b` (`tags::merge`) o "Fusionar con…" en el menú contextual **(propuesta para el nombre)**: todas las ocurrencias de `#a` pasan a `#b`. Es renombrar hacia un tag que ya existe, con el mismo flujo de vista previa y deshacer.
- `:tagrename` hacia un tag existente avisa en la vista previa ("`#b` ya existe: se fusionan") en vez de negarse.
- Si los dos tienen página, la vista previa lo marca como bloqueo y no aplica hasta que quede una; juntar dos frontmatter y dos cuerpos es decisión del usuario **(propuesta)**. Si solo `a` tiene página, pasa a ser la de `b`.
- Si una nota termina con el mismo tag dos veces en la lista `tags:`, el duplicado se quita de esa lista; el resto del frontmatter no se toca **(propuesta)**. En el texto, dos `#b` en el cuerpo se dejan: son dos menciones, no un dato repetido.

### Un tag incluye a sus descendientes

Filtrar, enfocar o consultar por `#a` incluye a los tags que tienen `a` entre sus `parents`, y a los de ellos, de forma transitiva (§4, §7). En el panel, el nodo de `a` cuenta las notas de todos ellos. No hay forma de pedir "solo `a` exacto, sin descendientes" **(propuesta)**; si hace falta, va a `IDEAS.md`.

## 6. Casos borde

| Caso | Decisión | Por qué |
|---|---|---|
| `#Rust`, `#rust` y `#RUST` | **El mismo tag** para agrupar, filtrar, consultar, focus y el panel | Nadie quiere dos tags que solo se distinguen por la mayúscula inicial de una frase. La clave usa el *case folding* por defecto de Unicode (`CaseFolding.txt`, no solo ASCII y no `str::to_lowercase`), con `caseless` 0.2.2 (crates.io, 2026-10-18): así `#STRASSE` y `#straße` también son el mismo. Es el folding sin locale, el mismo en cualquier máquina; la `ı` turca sin punto no se une a `i`. |
| Cómo se muestra | **La forma más usada** | El panel, el finder y los chips enseñan la variante que más ocurrencias tiene en el vault: con 9 `#Rust` y 1 `#rust`, "Rust (10)". Empate: la primera en orden de bytes (`Rust` antes que `rust`), así no cambia según el orden del walk. El nombre de la página cuenta como una ocurrencia más, así un tag con página y sin notas se muestra como su archivo **(propuesta)**. Se recalcula con el índice. **Nunca se reescribe un archivo por esto**; solo renombrar (§5) cambia lo escrito. |
| `#2024/10` y `#2024` | **Dos tags planos, sin relación** | La `/` no crea jerarquía (§1). Si `2024/10` va bajo `2024`, lo dice su página con `parents`. |
| `#canción` y `#cancion` | **Distintos** | Los acentos sí importan: en español cambian la palabra (`#año` y `#ano`). Quitarlos uniría cosas distintas sin que nadie se entere. |
| `#canción` con `ó` compuesta o descompuesta (U+00F3 vs `o` + U+0301) | **El mismo tag** | Solo se normaliza lo invisible: se ven idénticos y llegan distintos según el teclado o el sistema de origen del archivo. La clave es NFC (`unicode-normalization` 0.1.26, crates.io, 2026-10-18), luego folding, luego NFC otra vez, porque el folding puede dejar una cadena que ya no está en NFC. El archivo no se toca: solo la clave. |
| `#tag` en un bloque cercado, en código indentado o en `` `código inline` `` | **No cuenta** | Es código, no una nota sobre el tema: `#include`, `#[derive]` y comentarios de shell llenarían el índice. Lo resuelve el scanner (03 §2), que ya ignora código. |
| `#tag` dentro de `%% comentario %%` | **No cuenta** | El scanner ya los ignora (03 §2); un comentario no es contenido. |
| `#tag` en una URL o en `[[Nota#Sección]]` | No es tag | No va tras espacio o inicio de línea. |
| `#2024` | **Sí es tag** | Agrupa un año, y en nota los tags son el eje de organización (§1). Costo aceptado: `PR #12` deja un tag `#12`. |
| Mismo tag en el texto y en `tags:` | La nota cuenta una vez | El índice es por nota; las dos ocurrencias se guardan para renombrar. |
| Frontmatter roto | Los tags del cuerpo cuentan igual | El scanner trata el YAML roto como problema recuperable (03 §2). |
| Tag muy largo o grafo muy profundo | Sin límite | Un límite es un caso especial más; si un vault real lo necesita, se mide primero. |

Los casos de las páginas de tag están en §7.

## 7. Páginas de tag

Inspirado en TagStudio, donde los tags tienen metadata y "tags de tags". Un tag puede tener una página: un archivo markdown que guarda su metadata y explica de qué trata. **Es opcional**: un tag sin página funciona igual que siempre. Con página, el tag es también un lugar: al abrir `#rust` se ve su descripción arriba de sus notas.

### Dónde y cómo se llama

- `tags/<nombre>.md`, en una carpeta **plana**: sin subcarpetas. La carpeta es un ajuste del vault, como la de adjuntos ([06](06-decisiones-de-fondo.md) §6), con `tags/` por defecto **(propuesta para la clave `tags_folder` en `<vault>/.nota/config.yaml`)**.
- La página se identifica por el **nombre del archivo** sin `.md`, con las mismas reglas de identidad de §6: sin distinguir mayúsculas, con acentos y en NFC. `tags/Rust.md` es la página de `#rust`.
- Los archivos en subcarpetas de `tags/` (`tags/viejo/x.md`) no son páginas: son notas normales **(propuesta)**.
- Normalmente no se crea a mano: nota la crea en cuanto le pones metadata desde la UI (§3). Crearla a mano da lo mismo.

### Formato

```markdown
---
color: naranja
icon: 🦀
aliases: [rs]
parents: [lenguajes, programacion]
---
Notas sobre Rust: el lenguaje, sus crates y lo que voy aprendiendo.
Lo de GPUI vive en su propio tag, #gpui, que tiene a #rust como padre.
```

| Campo | Qué es |
|---|---|
| `color` | El color del tag en el panel, en los chips y en el `#tag` del texto. Un nombre de la paleta del tema (`rojo`, `naranja`, `azul`…, tokens `tag.color.*`) o un hex entre comillas (`"#d97706"`) **(propuesta para el formato)**. Con nombres, el color se adapta a temas claros y oscuros; el hex queda para quien lo quiera exacto. Sin comillas, `#d97706` es un comentario de YAML, y por eso la UI siempre escribe nombres o hex entre comillas. |
| `icon` | Un emoji. Se muestra delante del nombre en el panel, en el autocompletado y en la vista del tag. |
| `aliases` | Otros nombres del mismo tag: con `aliases: [rs]`, `#rs` cuenta como `#rust` para filtrar, consultar, contar y en el panel. El texto no se toca: la nota sigue diciendo `#rs`. |
| `parents` | Los padres del tag, **varios**. De aquí y solo de aquí sale la jerarquía. |

- `aliases` y `parents` aceptan una lista o un solo texto, y un `#` delante se ignora, igual que `tags:` (§1) **(propuesta)**.
- Un `parents` puede nombrar un alias: `parents: [rs]` es `parents: [rust]` **(propuesta)**.
- En una página, `aliases` son alias del tag, no los alias de nota que M3 usa para resolver `[[links]]` **(propuesta)**.
- Las claves desconocidas se ignoran y se conservan, como en cualquier frontmatter.
- **Ícono (propuesta)**: solo emoji en M2.5. Nombres de ícono (`icon: lucide:cpu`) piden empaquetar un set de íconos, que el tema los pinte y un selector que los busque; van a `IDEAS.md`. Un `icon` que no es un solo emoji se ignora y se marca como problema.

### Padres: un grafo, no un árbol

- `gpui` puede tener como padres a `rust`, `graficos` y `ui` a la vez, y aparece bajo los tres en el panel.
- Los padres sirven para **filtrar y buscar**: `#rust` trae también las notas de `#gpui`, y las de los hijos de `gpui`, de forma transitiva (§4, §5).
- **Ciclos**: `a` con padre `b` y `b` con padre `a` (o cualquier vuelta más larga) se detectan al construir el `TagGraph`, se avisan y no rompen nada. Los descendientes se calculan con un conjunto de visitados, así que `#a` trae `b` y `#b` trae `a`, sin vueltas infinitas. En el panel, un tag que ya está en el camino desde la raíz se muestra con una marca de ciclo y no se expande otra vez; si ningún tag del ciclo tiene un padre fuera de él, todos salen como raíces para que no desaparezcan del panel **(propuesta)**. El aviso sale en el panel y en las páginas del ciclo, con el camino escrito (`a → b → a`).
- Una página que se pone a sí misma como padre: se ignora ese padre y se avisa **(propuesta)**.

### La vista del tag

Abrir `#rust` (desde el panel, con `tags::open` o `ctrl`+clic sobre un `#rust` del texto) muestra la vista del tag: el nombre con su ícono y su color, sus alias y sus padres, el cuerpo de la página y debajo la lista de sus notas **(propuesta para los atajos)**. El cuerpo es la página misma, abierta como buffer: se edita como cualquier nota **(propuesta)**. Si el tag no tiene página, la vista muestra solo la lista y el botón "Agregar descripción", que crea la página (§3).

### Qué no es una página

- Las páginas **no salen en el buzón** aunque no tengan tags ([06](06-decisiones-de-fondo.md) §9).
- **No cuentan como notas normales** en búsquedas ni en conteos, salvo que se pida: `:focus tags/`, la vista de carpetas o una opción del finder y de la búsqueda full-text las muestran **(propuesta para cómo se pide)**.
- Los `#tag` del cuerpo de una página no suman al conteo de esos tags, pero sí se reescriben al renombrar **(propuesta)**.
- Un `[[tags/rust]]` es un link a un archivo como cualquier otro (M3) **(propuesta)**.

### Casos borde

| Caso | Decisión | Por qué |
|---|---|---|
| Página sin frontmatter | **Vale.** Define el tag por su nombre de archivo, sin metadata; el cuerpo es su descripción. | Escribir de qué trata un tag no debería exigir YAML. |
| Página con frontmatter roto | El tag existe y conserva su descripción; la metadata se ignora y queda como problema recuperable **(propuesta)** | Igual que en una nota (03 §2): un YAML roto no tumba lo demás. |
| Un alias que choca con el nombre de otro tag con página (`rust.md` dice `aliases: [rs]` y existe `tags/rs.md`) | **Gana la página**: `#rs` es el tag `rs`; el alias se ignora y se avisa en las dos páginas **(propuesta)** | Una página es una declaración explícita de que el tag existe por sí mismo. Si el alias ganara, la página de `rs` quedaría muerta sin que nadie la haya borrado. |
| Un alias que nombra un tag usado en notas pero sin página (`#rs` en 20 notas) | **El alias absorbe al tag**: esas 20 notas cuentan como `#rust` | Es justo para lo que sirve un alias. |
| Un alias que choca con otro alias (`rust.md` y `rescript.md` dicen `aliases: [rs]`) | **Ninguno aplica**: `#rs` queda como tag propio y se avisa en las dos páginas **(propuesta)** | Elegir uno adivinaría; dejarlo suelto es visible y no mete notas de rescript en rust. |
| Un alias igual al nombre de su propia página | Se ignora sin avisar **(propuesta)** | No cambia nada. |
| Un padre que no existe como tag ni como página (`parents: [graficos]` y nadie escribe `#graficos`) | **Vale**: `graficos` aparece en el panel como nodo que agrupa a sus hijos, con una marca de "solo existe como padre" en el color de aviso del tema **(propuesta)** | Es una forma legítima de crear una categoría sin página. La marca deja ver un typo (`parents: [rsut]`) sin bloquear a quien lo hizo a propósito. |
| Ciclo de padres | Se detecta, se avisa y no rompe nada | Ver "Padres: un grafo, no un árbol". |
| Tag con `/` (`#2024/10`) | **No puede tener página** en M2.5: `/` no cabe en un nombre de archivo y la carpeta es plana. La UI no ofrece metadata para esos tags y lo explica **(propuesta)** | Codificar la `/` en el nombre (`2024∕10.md` o `2024%2F10.md`) se puede ver después; ninguna opción se lee bien a mano. |
| Archivo cuyo nombre no es un tag válido (`tags/mi tag.md`, `tags/a.b.md`) | No es página; es una nota normal y se lista como problema **(propuesta)** | No existe un `#tag` que lo nombre. |
| Dos archivos con la misma clave (`tags/Rust.md` y `tags/rust.md`, o `ó` compuesta y descompuesta) | Vale el primero en orden de bytes y se avisa en los dos **(propuesta)** | Mismo desempate que la forma escrita (§6): estable, sin depender del orden del walk. |
| Mover o renombrar `tags/rust.md` fuera de nota | El watcher lo ve como cambio de página: `rust` se queda sin página y aparece la del nombre nuevo. Las notas no se reescriben **(propuesta)** | Inferir un renombre de tag desde un cambio de archivo reescribiría notas sin vista previa. El renombre de verdad es `tags::rename`. |
| **Renombrar un tag con página** | Se renombra la página y se actualizan los `parents` y `aliases` de las otras páginas que lo nombran, en la misma vista previa y el mismo deshacer que las notas | Ver §5. |

## 8. Fuera de M2.5

Van a [`docs/IDEAS.md`](../IDEAS.md#tags):

- **Vistas guardadas**: una consulta con nombre, como las carpetas inteligentes.
- "O" en las consultas, paréntesis, comodines y "solo este tag sin descendientes".
- Recordar el focus entre sesiones.
- Íconos por nombre, además de emoji (§7).
- Páginas para tags con `/` (§7).
- Sugerir tags a partir del contenido.
- Que el índice siga al buffer sin guardar (§2).

No se hacen: tags inferidos de la carpeta (la carpeta y el tag son ejes distintos, y mezclarlos traería de vuelta la jerarquía única) **(propuesta)**.

## 9. Hecho cuando…

El criterio de M2.5 en 03 §8. Desglosado:

- Fixture de casos borde del scanner, de las páginas de tag y del parseo de consultas en `insta` (§1, §4, §6, §7).
- Tabla de casos del `TagGraph`: alias que chocan con un tag con página o con otro alias, padre suelto, ciclo, varios padres; para cada uno, los descendientes de cada tag, los avisos y el panel resultante (§7).
- Proptest: índice y grafo incrementales == completos (§2).
- Test de renombrar `#a` → `#b` con página, con otras páginas que tienen `a` en `parents` y `aliases`, con frontmatter y texto: cada archivo cambia solo en los rangos esperados y la página cambia de nombre; deshacer deja todos los archivos byte a byte iguales al original y la página en su nombre viejo.
- Test de poner color e ícono desde una acción a un tag sin página: crea `tags/<nombre>.md` con solo esos campos; repetirlo sobre una página con frontmatter escrito a mano cambia solo ese campo.
- Las páginas no salen en el buzón, ni en `space f`, ni en los conteos, salvo que se pidan.
- Con el vault sintético de 5 k notas de M2 y 200 páginas de tag: índice y grafo completos en el arranque en < 200 ms **(objetivo)**, medido con `divan` como el de archivos; filtrar `space f` por un tag con descendientes en < 16 ms **(objetivo)**.
