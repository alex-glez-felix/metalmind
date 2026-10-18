# 05 — Tags

> Diseño del 2026-10-18. Decisión de producto: **los tags son el eje principal de organización de nota, por encima de las carpetas.** Cómo se escribe un tag (mayúsculas, acentos, cómo se muestra) también está decidido (§6). Lo que no venía de esas decisiones va marcado **(propuesta)** y queda pendiente de confirmar.
> Construye sobre [03](03-core-architecture.md): el scanner de `nota-syntax` (§2), el índice y el watcher de `nota-vault` (§4) y el modelo de acciones con nombre (§3). Entra en el roadmap como **M2.5** (03 §8).

## TL;DR

- Una carpeta obliga a elegir un solo lugar para cada nota; una nota casi siempre pertenece a varias cosas. Los tags no obligan a elegir. Las notas siguen siendo archivos markdown en carpetas, pero la UI y el modelo tratan a los tags como ciudadanos de primera clase.
- **Un solo sistema de tags**: los `#tag` del texto (con `#padre/hijo` para jerarquía) y la lista `tags:` del frontmatter son lo mismo. Viven en el archivo, que es la fuente de verdad; el índice de tags es caché regenerable.
- **Panel lateral** con dos vistas, árbol de tags (jerárquico por `/`, con conteo) y árbol de carpetas, y una tecla para cambiar. Por defecto, tags **(propuesta)**.
- **Modo focus**: enfocar toda la app (finder, búsqueda, panel, backlinks) en una carpeta, un tag o una consulta de tags. `:focus #rust`, `:focus proyectos/`, o desde el panel.
- **Consultas**: `#rust #nota -#archivado`. Espacio es "y", `-` es "no", un tag padre incluye a sus hijos. Nada más.
- **Operaciones**: autocompletado de `#`, renombrar un tag en todo el vault (con vista previa y deshacer), fusionar dos tags.
- `#Rust`, `#rust` y `#RUST` son el mismo tag, y se muestra como lo escribe la mayoría; `#canción` y `#cancion` son distintos (§6).

---

## 1. De dónde sale un tag

Dos fuentes, un solo tag:

| Fuente | Ejemplo | Rango que se guarda |
|---|---|---|
| Texto | `Hoy probé #rust/async en #nota` | Del `#` al último carácter del nombre |
| Frontmatter | `tags: [rust/async, nota]` o `tags:` con lista en bloque | El del valor dentro del YAML (`Spanned<T>` de `serde-saphyr`, 03 §2) |

Reglas del texto (las del scanner de `nota-syntax`, 03 §2):

- Empieza al inicio de línea o tras un espacio: `texto#algo` y el fragmento de una URL no son tags.
- Letras, números, `_`, `-` y `/`; fuera de ASCII, letras, marcas, números y símbolos (emoji incluidos). La puntuación lo corta: `#rust.` es `#rust`.
- **Decisión: puede ser solo números.** `#2024`, `#1` y `#2024/10` son tags, porque en nota los tags son el eje de organización y `#2024` sirve para agrupar un año; la regla contraria venía de Obsidian. Costo aceptado: en texto como "PR #12", `#12` se vuelve tag.
- `# Título` es un heading, no un tag: el espacio lo decide.

Reglas del frontmatter:

- Solo la clave en plural, `tags` (03 §2). Vale una lista o un solo texto; un `#` delante del valor se ignora (`tags: ["#rust"]` es `rust`) **(propuesta)**. Un texto con comas o espacios no se parte: `tags: "rust, nota"` no es un tag válido y se marca como problema recuperable **(propuesta)**; partirlo adivina la intención y luego renombrar tendría que reescribir dentro de un texto que el usuario escribió entero.
- Los `#` en otras claves del YAML no son tags.

**Jerarquía.** `#a/b/c` es un tag que tiene como ancestros a `a` y `a/b`. La nota que lleva `#a/b/c` aparece bajo `a`, `a/b` y `a/b/c`. Un segmento vacío invalida el tag (`#a//b`); un `/` al final se descarta (`#a/` es `#a`) **(propuesta)**.

## 2. Modelo de datos

### Dónde vive cada pieza

| Crate | Qué pone |
|---|---|
| `nota-syntax` | El scanner: de un `&str` saca las ocurrencias de tags (texto y frontmatter) con su rango en bytes. Función pura, sin estado. |
| `nota-vault` | El `TagIndex`, la clave normalizada (`TagKey`), las consultas (`TagQuery`) y el alcance de focus (`Scope`), y la refactorización de renombrar/fusionar. Nada de esto toca la GUI **(propuesta)**. |
| `nota-search` | Sin cambios: el finder recibe la lista de candidatos ya filtrada por el `Scope`. Sigue sin depender de `nota-vault` (03 §4). |
| `nota-ui` | El árbol de tags del panel, el indicador de focus y el autocompletado. Solo pinta y llama acciones. |

### Tipos

```rust
struct TagKey(SmolStr);              // normalizada (§6): NFC + case folding, sin el '#'
enum TagSource { Inline, Frontmatter }
struct TagOccurrence { key: TagKey, spelling: SmolStr, range: Range<usize>, source: TagSource }

struct TagIndex {
    by_tag:  BTreeMap<TagKey, TagEntry>,                      // ordenado: el subárbol de `a` es un rango
    by_note: HashMap<NoteId, SmallVec<[TagOccurrence; 4]>>,
}
struct TagEntry {
    notes: BTreeSet<NoteId>,                                   // las que llevan este tag exacto
    spellings: SmallVec<[(SmolStr, u32); 1]>,                  // cómo se escribe y cuántas veces
}
```

- `TagKey` es la identidad: agrupar, filtrar, consultar, focus y el árbol usan solo la clave. `spelling` es lo que el usuario escribió y solo sirve para mostrar (§6).

- Este índice reemplaza al `tags: BTreeMap<SmolStr, Vec<NoteId>>` que 03 §4 dejó dentro de `LinkIndex`: tags y links se indexan del mismo scan pero se consultan por separado **(propuesta)**.
- `by_note` guarda los rangos para que renombrar no tenga que reescanear el vault (§5), igual que `Link` guarda `range` para renombrar notas (03 §4).
- **Descendientes como rango.** Con un `BTreeMap` ordenado por bytes, los hijos de `a` son las claves en `"a/".."a0"` (`'0'` es el carácter siguiente a `'/'` en ASCII). Filtrar por `#a` es `notes(a) ∪ notes(rango de a/)`, sin recorrer todo el índice.
- **Conteo del árbol**: número de notas **distintas** en el subárbol, no la suma de los hijos: una nota con `#a/b` y `#a/c` cuenta una vez en `a` **(propuesta)**. Se calcula al construir la vista del árbol, una pasada por `by_note`, y se recalcula solo en las ramas que cambiaron.

### Construcción e incremental

- **En frío**: el mismo walk de `Vault::open` lista las notas; el escaneo de tags va en el pool de fondo, en paralelo por nota, y el índice se publica cuando termina. La UI abre antes: el panel muestra las carpetas y el árbol de tags aparece al llegar el índice **(propuesta)**.
- **Incremental**: el `TagIndex` se suscribe a los `VaultUpdate` del watcher (03 §4). `Created`/`Modified` → reescanear esa nota y diferenciar sus ocurrencias viejas y nuevas; `Removed` → quitar la nota; `Renamed` → solo cambia la ruta, el `NoteId` y sus tags siguen. Se publica igual que el índice de archivos (snapshot en `ArcSwap` + evento `IndexEvent::TagsChanged(Vec<TagKey>)`), para que la UI lea sin bloquear.
- **Buffers abiertos**: el índice refleja el disco, no el buffer sin guardar. El tag que escribes aparece en el árbol al guardar **(propuesta)**. Indexar cada tecla haría que el árbol baile mientras escribes `#ru…` y obligaría a tener dos verdades.
- **Invariante** (proptest, como el de archivos): índice incremental tras cualquier secuencia de crear, editar, mover y borrar == índice construido de cero sobre el resultado.

## 3. UI

### Panel lateral

Dos vistas en el mismo panel, árbol de tags y árbol de carpetas (el de M2). Una sola está visible a la vez.

- **Por defecto: árbol de tags (propuesta).** Es el eje de organización; abrir la app en la vista de carpetas diría lo contrario. La vista elegida se recuerda por vault **(propuesta)**.
- Cambiar de vista: `T` dentro del panel (`panel::toggle_view`) **(propuesta)**; `space t` abre el panel en tags y `space e` en carpetas (`panel::show_tags`, `panel::show_folders`) **(propuesta)**. El encabezado del panel dice cuál es.
- Cada nodo de tag muestra su segmento (`async`, no `rust/async`) y el conteo. Bajo un tag, primero sus tags hijos y luego sus notas, como una carpeta con subcarpetas y archivos **(propuesta)**. Una nota con tres tags aparece en tres lugares, y está bien: es justamente lo que una carpeta no permite.
- Al final, un nodo **Sin tags** con las notas que no tienen ninguno **(propuesta)**. Sin él, una nota sin tags es invisible en la vista por defecto.
- Cada nodo muestra la forma establecida de su segmento (§6): "Rust (10)" aunque una nota diga `#rust`.
- Orden: alfabético por la forma que se muestra, sin mayúsculas, como el de carpetas **(propuesta)**.
- Vim, igual que `FileTree` (03 §3): `j/k`, `h/l` colapsar/expandir, `gg/G`, `/` filtrar, `enter` abre la nota o expande el tag, `r` renombra el tag (§5), `F` enfoca (§3, focus) **(propuesta para `r` y `F` aquí)**. El contexto es `TagTree`.

### Modo focus

Inspirado en el "workspace view" de Inkdrop: enfocarse reduce **toda la app** a un subconjunto del vault.

**Qué es un alcance (`Scope`)**: una carpeta, una consulta de tags (§4) o las dos a la vez (`:focus proyectos/ #rust`), que se intersectan **(propuesta para la combinación carpeta + tags)**.

**Cómo se activa**:

| Desde | Cómo | Acción |
|---|---|---|
| `:` | `:focus #rust`, `:focus #rust #nota -#archivado`, `:focus proyectos/` | `workspace::focus` |
| Panel | `F` sobre un tag o una carpeta | `workspace::focus` con ese nodo |
| Salir | `:unfocus` o `F` sobre el mismo nodo **(propuesta)** | `workspace::clear_focus` |

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
| `#a` | La nota lleva `a` o un descendiente (`a/…`). |
| `#a #b` | Las dos (espacio = "y"). |
| `-#a` | No lleva `a` ni descendientes. |
| `carpeta/` | Está bajo esa carpeta (solo en `:focus`, §3). |
| cualquier otra palabra | En `space f`, texto para el fuzzy de rutas, como hoy. |

- Ejemplo: `#rust #nota -#archivado` = notas de rust **y** de nota que **no** están archivadas.
- En `space f` se mezclan: `#rust parser` filtra por `#rust` y busca `parser` con fuzzy entre lo que queda **(propuesta)**.
- El tag de la consulta se compara con la clave normalizada (§6): `#Rust` encuentra `#rust`.
- Un `-#a` solo, sin ningún `#` positivo, vale: "todo menos `a`".
- **No hay "o"**, ni paréntesis, ni comodines **(propuesta)**. Cubren casos reales pero convierten esto en un lenguaje; si hacen falta, se ven junto con las vistas guardadas (`IDEAS.md`).
- Un tag que no existe en el vault no es error: da cero resultados y el prompt lo marca en el color de aviso del tema.
- En código, `TagQuery { include: Vec<TagKey>, exclude: Vec<TagKey> }` y `Scope { folder: Option<VaultPath>, tags: TagQuery }`, con `fn notes(&self, &TagIndex, &VaultIndex) -> NoteSet`. El parseo es una función pura con tabla de casos en `insta`.

## 5. Operaciones

### Autocompletado de `#`

- En Insert, al escribir `#` donde puede empezar un tag (inicio de línea o tras espacio) seguido de un carácter de tag, se abre la lista de tags del vault, filtrada con `nucleo-matcher` y ordenada por número de notas **(propuesta)**. Con `/` sigue en el subárbol: `#rust/` ofrece los hijos de `rust`.
- También dentro de la lista `tags:` del frontmatter.
- No aparece en bloques de código ni en código inline. `# ` con espacio cierra la lista: es un heading.
- La lista ofrece e inserta la **forma establecida** (§6), no la que escribiste a medias: escribir `#ru` y aceptar deja `#Rust` si así lo escribe el vault. Así no nacen variantes nuevas. `ctrl-n`/`ctrl-p` y `tab` como en el resto de pickers **(propuesta para las teclas)**.

### Renombrar un tag en todo el vault

Es una refactorización: reescribe archivos del usuario. Por eso va en tres pasos, nunca en uno.

1. **Pedir**: `:tagrename #viejo #nuevo` (`tags::rename`) o `r` sobre el tag en el panel **(propuesta para el nombre del comando)**.
2. **Vista previa**: una lista, como un picker, con cada ocurrencia (archivo, línea, antes → después) y el total de archivos y ocurrencias. Se navega con `j/k`, `enter` abre esa línea, `x` quita esa ocurrencia del cambio **(propuesta)**. `:w` aplica; `Esc` o `:q` cancela sin tocar nada **(propuesta)**.
3. **Aplicar**: cada archivo se reescribe solo en los rangos de `by_note`, con escritura atómica. Se valida antes que el archivo en disco no cambió desde el escaneo (mismo `mtime`+tamaño y hash); si cambió, se reescanea ese archivo y se avisa en vez de escribir a ciegas.

Detalles:

- **Descendientes**: renombrar `#a` a `#b` renombra también `#a/x` a `#b/x` **(propuesta)**. Mover una rama del árbol es justamente para lo que sirve la jerarquía.
- **Frontmatter**: se reemplaza solo el texto del valor en su rango, sin re-serializar el YAML; el resto del bloque queda byte a byte igual (regla de `CLAUDE.md`: nunca reescribir el YAML del usuario salvo el campo editado).
- **Mayúsculas**: renombrar reescribe **todas las variantes** del tag (`#Rust`, `#rust`, `#RUST`) a la forma nueva. Renombrar `#Rust` a `#rust` es válido aunque la clave sea la misma: es la manera de unificar cómo se escribe un tag. Es la única vez que nota toca la forma escrita; mostrar la forma establecida nunca reescribe archivos.
- **Buffers abiertos con cambios sin guardar**: el cambio se aplica al buffer como una transacción (entra en su undo) y el archivo queda modificado sin guardar; no se escribe el disco por debajo del buffer **(propuesta)**. Buffers abiertos sin cambios: se escribe el disco y el buffer se recarga.
- **Deshacer**: la operación entera es una sola transacción de vault, con el contenido previo de cada archivo tocado guardado en memoria durante la sesión. `:tagundo` (`tags::undo_rename`) la revierte **(propuesta para el nombre)**. Si un archivo cambió después del renombre, ese archivo no se revierte y se lista; los demás sí. Es el mismo motor que necesita M3 para "renombrar nota actualiza links" (03 §4), y se diseña una vez para los dos **(propuesta)**.
- **Fallo a medias**: cada archivo es atómico, el conjunto no. Si una escritura falla, se para, se informa qué quedó escrito y se ofrece deshacer lo aplicado.

### Fusionar dos tags

- `:tagmerge #a #b` (`tags::merge`) **(propuesta para el nombre)**: todas las ocurrencias de `#a` pasan a `#b`. Es renombrar hacia un tag que ya existe, con el mismo flujo de vista previa y deshacer.
- `:tagrename` hacia un tag existente avisa en la vista previa ("`#b` ya existe: se fusionan") en vez de negarse.
- Si una nota termina con el mismo tag dos veces en la lista `tags:`, el duplicado se quita de esa lista; el resto del frontmatter no se toca **(propuesta)**. En el texto, dos `#b` en el cuerpo se dejan: son dos menciones, no un dato repetido.

### Padre incluye a los hijos

Filtrar, enfocar o consultar por `#a` incluye `#a/…` (§4). En el panel, el nodo de `a` cuenta las notas del subárbol. No hay forma de pedir "solo `a` exacto, sin hijos" **(propuesta)**; si hace falta, va a `IDEAS.md`.

## 6. Casos borde

| Caso | Decisión | Por qué |
|---|---|---|
| `#Rust`, `#rust` y `#RUST` | **El mismo tag** para agrupar, filtrar, consultar, focus y el árbol | Nadie quiere dos tags que solo se distinguen por la mayúscula inicial de una frase. La clave usa el *case folding* por defecto de Unicode (`CaseFolding.txt`, no solo ASCII y no `str::to_lowercase`), con `caseless` 0.2.2 (crates.io, 2026-10-18): así `#STRASSE` y `#straße` también son el mismo. Es el folding sin locale, el mismo en cualquier máquina; la `ı` turca sin punto no se une a `i`. |
| Cómo se muestra | **La forma más usada** | El panel, el finder y los chips enseñan la variante que más ocurrencias tiene en el vault: con 9 `#Rust` y 1 `#rust`, "Rust (10)". Empate: la primera en orden de bytes (`Rust` antes que `rust`), así no cambia según el orden del walk. Se recalcula con el índice. **Nunca se reescribe un archivo por esto**; solo renombrar (§5) cambia lo escrito. |
| Jerarquía con mayúsculas | `#Rust/GPUI` y `#rust/gpui` son **el mismo nodo** | La clave se normaliza entera, segmentos incluidos. La forma de cada segmento se elige por separado y cuenta también las ocurrencias de los descendientes: si nadie escribe `#rust` solo pero 5 notas dicen `#Rust/GPUI`, el nodo padre se muestra "Rust". |
| `#canción` y `#cancion` | **Distintos** | Los acentos sí importan: en español cambian la palabra (`#año` y `#ano`). Quitarlos uniría cosas distintas sin que nadie se entere. |
| `#canción` con `ó` compuesta o descompuesta (U+00F3 vs `o` + U+0301) | **El mismo tag** | Solo se normaliza lo invisible: se ven idénticos y llegan distintos según el teclado o el sistema de origen del archivo. La clave es NFC (`unicode-normalization` 0.1.26, crates.io, 2026-10-18), luego folding, luego NFC otra vez, porque el folding puede dejar una cadena que ya no está en NFC. El archivo no se toca: solo la clave. |
| `#tag` en un bloque cercado, en código indentado o en `` `código inline` `` | **No cuenta** | Es código, no una nota sobre el tema: `#include`, `#[derive]` y comentarios de shell llenarían el índice. Lo resuelve el scanner (03 §2), que ya ignora código. |
| `#tag` dentro de `%% comentario %%` | **No cuenta** | El scanner ya los ignora (03 §2); un comentario no es contenido. |
| `#tag` en una URL o en `[[Nota#Sección]]` | No es tag | No va tras espacio o inicio de línea. |
| `#2024` | **Sí es tag** | Agrupa un año, y en nota los tags son el eje de organización (§1). Costo aceptado: `PR #12` deja un tag `#12`. |
| Mismo tag en el texto y en `tags:` | La nota cuenta una vez | El índice es por nota; las dos ocurrencias se guardan para renombrar. |
| Frontmatter roto | Los tags del cuerpo cuentan igual | El scanner trata el YAML roto como problema recuperable (03 §2). |
| Tag muy largo o muy profundo | Sin límite | Un límite es un caso especial más; si un vault real lo necesita, se mide primero. |

## 7. Fuera de M2.5

Van a [`docs/IDEAS.md`](../IDEAS.md#tags):

- **Vistas guardadas**: una consulta con nombre, como las carpetas inteligentes.
- "O" en las consultas, paréntesis, comodines y "solo este tag sin hijos".
- Recordar el focus entre sesiones.
- Color o ícono por tag.
- Una nota que describe un tag (abrir `#rust` muestra su nota).
- Alias de tags (`#rs` → `#rust`).
- Sugerir tags a partir del contenido.
- Que el índice siga al buffer sin guardar (§2).

No se hacen: tags inferidos de la carpeta (la carpeta y el tag son ejes distintos, y mezclarlos traería de vuelta la jerarquía única) **(propuesta)**.

## 8. Hecho cuando…

El criterio de M2.5 en 03 §8. Desglosado:

- Fixture de casos borde del scanner y del parseo de consultas en `insta` (§1, §4, §6).
- Proptest: índice incremental == completo (§2).
- Test de renombrar `#a` → `#b` con hijos, frontmatter y texto: cada archivo cambia solo en los rangos esperados; deshacer deja todos los archivos byte a byte iguales al original.
- Con el vault sintético de 5 k notas de M2: índice de tags completo en el arranque en < 200 ms **(objetivo)**, medido con `divan` como el de archivos; filtrar `space f` por un tag en < 16 ms **(objetivo)**.
