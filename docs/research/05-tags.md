# 05 — Tags

> Diseño del 2026-10-18. Decisión de producto: **los tags son el eje principal de organización de Metalmind, por encima de las carpetas.** También están decididos cómo se escribe un tag (mayúsculas, acentos, cómo se muestra, §6), que **los tags son planos** y la jerarquía sale solo de los `parents` de las páginas de tag (§1, §7), y que todo se puede hacer desde la UI sin editar archivos a mano ([06](06-decisiones-de-fondo.md) §13). Los detalles que quedaban abiertos se cerraron con las respuestas del autor en la PR #33 y, en lo menor (nombres de comandos, teclas, casos borde), con la recomendación de este documento: todo lo de aquí es decisión.
> Construye sobre [03](03-core-architecture.md): el scanner de `metalmind-syntax` (§2), el índice y el watcher de `metalmind-vault` (§4) y el modelo de acciones con nombre (§3). Entra en el roadmap como **M2.5** (03 §8).

## TL;DR

- Una carpeta obliga a elegir un solo lugar para cada nota; una nota casi siempre pertenece a varias cosas. Los tags no obligan a elegir. Las notas siguen siendo archivos markdown en carpetas, pero la UI y el modelo tratan a los tags como ciudadanos de primera clase.
- **Un solo sistema de tags**: los `#tag` del texto y la lista `tags:` del frontmatter son lo mismo. Viven en el archivo, que es la fuente de verdad; el índice de tags es caché regenerable.
- **Los tags son planos.** Un tag es solo un nombre: `#gpui`. Un nombre solo usa caracteres que valen en un nombre de archivo, así que todo tag puede tener página. La `/` no forma parte del tag: `#2024/10` es `#2024` seguido de texto.
- **Página de tag opcional**: `tags/<nombre>.md`, en una carpeta plana y configurable. Su frontmatter lleva `color`, `icon` (emoji o `set:nombre`, con Tabler incluido), `aliases` y `parents`; su cuerpo dice de qué trata el tag. Un tag sin página funciona igual (§7).
- **La jerarquía sale solo de `parents`**, y es un grafo: `gpui` puede estar bajo `rust`, `graficos` y `ui` a la vez. `#rust` trae también las notas de `#gpui`, de forma transitiva. Los ciclos se avisan y no rompen nada.
- **Se usa desde la UI**: clic derecho sobre un tag (en el panel o en el texto) cambia color, ícono, alias y padres, renombra, fusiona y abre su página. Metalmind crea la página sola; el archivo es el almacenamiento, no la interfaz (§3).
- **Panel lateral** con dos vistas, tags (en grafo por `parents`, con conteo) y carpetas, y una tecla para cambiar. Abre en tags; `T` cambia a carpetas.
- **Modo focus**: enfocar toda la app (finder, búsqueda, panel, backlinks) en una carpeta, un tag o una consulta de tags. `:focus #rust`, `:focus proyectos/`, o desde el panel.
- **Consultas**: `#rust #nota -#archivado`. Espacio es "y", `-` es "no", un tag incluye a sus alias y a sus descendientes por `parents`. La forma `clave:valor` queda reservada para filtros futuros (§4).
- **Operaciones**: autocompletado de `#`, renombrar un tag o un alias en todo el vault (con su página y los `parents`/`aliases` que lo nombran, con vista previa y deshacer), quitar un tag de todo el vault con el mismo flujo, pasar un alias al nombre principal, fusionar dos tags.
- **Solo los tags tienen alias.** Se declaran en la página del tag; una nota normal no tiene `aliases`.
- `#Rust`, `#rust` y `#RUST` son el mismo tag, y se muestra como lo escribe la mayoría; `#canción` y `#cancion` son distintos (§6).

---

## 1. De dónde sale un tag

Dos fuentes, un solo tag:

| Fuente | Ejemplo | Rango que se guarda |
|---|---|---|
| Texto | `Hoy probé #gpui en #nota` | Del `#` al último carácter del nombre |
| Frontmatter | `tags: [gpui, nota]` o `tags:` con lista en bloque | El del valor dentro del YAML (`Spanned<T>` de `serde-saphyr`, 03 §2) |

Al escribir se pone solo el tag (`#gpui`), sin ruta: que `gpui` esté bajo `rust` lo dice la página de `gpui` (§7), no cada nota.

Reglas del texto (las del scanner de `metalmind-syntax`, 03 §2):

- Empieza al inicio de línea o tras un espacio: `texto#algo` y el fragmento de una URL no son tags.
- Letras, números, `_` y `-`; fuera de ASCII, letras, marcas, números y símbolos (emoji incluidos). La puntuación lo corta: `#rust.` es `#rust`.
- **Decisión: puede ser solo números.** `#2024`, `#1` y `#2024-10` son tags, porque en Metalmind los tags son el eje de organización y `#2024` sirve para agrupar un año; la regla contraria venía de Obsidian. Costo aceptado: en texto como "PR #12", `#12` se vuelve tag.
- `# Título` es un heading, no un tag: el espacio lo decide.
- **Un tag nunca lleva espacios**, ni en el texto, ni en `tags:`, ni con una sintaxis de cierre como el `#tag con espacios#` de Bear. Decidido el 2026-10-24 ([07 §5](07-features-otras-apps.md#5-preguntas-y-respuestas), pregunta 2): ninguna otra herramienta la lee, y la gramática no se cambia después de publicarla.

Reglas del frontmatter:

- Solo la clave en plural, `tags` (03 §2). Vale una lista o un solo texto; un `#` delante del valor se ignora (`tags: ["#rust"]` es `rust`). Un texto con comas o espacios no se parte: `tags: "rust, nota"` no es un tag válido y se marca como problema recuperable; partirlo adivina la intención y luego renombrar tendría que reescribir dentro de un texto que el usuario escribió entero.
- Los `#` en otras claves del YAML no son tags.

**Un nombre de tag solo usa caracteres que valen en un nombre de archivo.** Así todo tag puede tener su página `tags/<nombre>.md` (§7) sin casos especiales ni codificar nada en el nombre. Las reglas:

- **La `/` no forma parte del tag**: lo corta, como la puntuación. `#2024/10` es el tag `#2024` seguido del texto `/10`; `#proyecto/` es `#proyecto`; `#/a` no es tag. Para un mes, `#2024-10`. Tampoco hay jerarquía por `/`: que un tag esté bajo otro lo dicen los `parents` de su página (§7).
- **También cortan el tag** `\ : * ? " < > |`. En Linux solo la `/` (y el NUL) no caben en un nombre de archivo, pero un vault se sincroniza (Syncthing, git, una nube) con Windows, macOS y Android, y ahí esos caracteres fallan o se reemplazan sin avisar. Un tag cuyo archivo no puede existir en otra máquina del mismo vault es un tag a medias. En la práctica ya estaban fuera, porque en ASCII un tag solo admite letras, números, `_` y `-`; el único cambio frente a la PR #30 es la `/`.
- **Longitud**: un nombre de más de 250 bytes en UTF-8 no es tag, porque `<nombre>.md` tiene que caber en los 255 bytes de ext4 y APFS (NTFS cuenta 255 unidades UTF-16, que nunca son más que los bytes UTF-8). No es un límite que alguien vaya a tocar escribiendo; existe para que la regla "todo tag puede tener página" sea verdad siempre.
- **Nombres reservados de Windows** (`con`, `nul`, `aux`, `prn`, `com1`…`com9`, `lpt1`…`lpt9`): son tags normales. Si el usuario crea su página desde la UI, Metalmind avisa que ese archivo no se sincroniza con Windows y la crea igual. Prohibirlos quitaría tags reales (`#con`) por una máquina que quizá no existe.
- En `tags:` del frontmatter un valor no se corta: `tags: [2024/10]` no es un tag válido y queda como problema recuperable, igual que `"rust, nota"`.
- Los nombres de las páginas se escriben en NFC (`#canción` → `tags/canción.md`). macOS puede devolverlos en NFD, pero la clave del tag es la misma (§6).

## 2. Modelo de datos

### Dónde vive cada pieza

| Crate | Qué pone |
|---|---|
| `metalmind-syntax` | El scanner: de un `&str` saca las ocurrencias de tags (texto y frontmatter) con su rango en bytes, y de una página de tag sus campos (`color`, `icon`, `aliases`, `parents`) con sus rangos. Funciones puras, sin estado. |
| `metalmind-vault` | El `TagIndex`, el `TagGraph` de las páginas, la clave normalizada (`TagKey`), las consultas (`TagQuery`), el alcance de focus (`Scope`), la refactorización de renombrar/fusionar y la escritura de metadata desde la UI. Nada de esto toca la GUI. |
| `metalmind-search` | Sin cambios: el finder recibe la lista de candidatos ya filtrada por el `Scope` y no sabe de `metalmind-vault`. La búsqueda de texto completo sí depende de `metalmind-vault` para recorrer el disco con sus reglas, y recibe el alcance como un conjunto de rutas (03 §4). |
| `metalmind-ui` | El panel de tags, el menú contextual y los selectores (§3), el indicador de focus y el autocompletado. Solo pinta y llama acciones. |

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
- Este índice reemplaza al `tags: BTreeMap<SmolStr, Vec<NoteId>>` que 03 §4 dejó dentro de `LinkIndex`: tags y links se indexan del mismo scan pero se consultan por separado.
- `by_note` guarda los rangos para que renombrar no tenga que reescanear el vault (§5), igual que `Link` guarda `range` para renombrar notas (03 §4).
- **Índice y grafo separados.** El `TagIndex` guarda lo que las notas escriben; el `TagGraph` dice qué significa. Resolver un alias o un padre se hace al consultar, así que cambiar una página no obliga a reescanear ninguna nota.
- **Descendientes**: los de `a` son el cierre de `children` desde `a`, con un conjunto de visitados para que un ciclo no dé vueltas (§7). Filtrar por `#a` es la unión de las notas de `a`, de sus alias y de cada descendiente (y de sus alias).
- **Conteo del panel**: número de notas **distintas** en `a` y sus descendientes, no la suma de los hijos: una nota con `#gpui` y `#tokio` cuenta una vez en `rust`; una nota bajo `gpui` que también está bajo `ui` cuenta una vez en cada uno. Se calcula al construir la vista del panel y se recalcula solo en los tags afectados por el cambio.

### Construcción e incremental

- **En frío**: el mismo walk de `Vault::open` lista las notas; el escaneo de tags va en el pool de fondo, en paralelo por nota, y el índice se publica cuando termina. Las páginas de tag se escanean en el mismo walk y el `TagGraph` se construye después, en una pasada. La UI abre antes: el panel muestra las carpetas y los tags aparecen al llegar el índice.
- **Incremental**: el `TagIndex` y el `TagGraph` se suscriben a los `VaultUpdate` del watcher (03 §4). En una nota: `Created`/`Modified` → reescanear esa nota y diferenciar sus ocurrencias viejas y nuevas; `Removed` → quitar la nota; `Renamed` → solo cambia la ruta, el `NoteId` y sus tags siguen. En una página: rehacer el `TagGraph` entero, que es barato porque solo tiene las páginas. Se publica igual que el índice de archivos (snapshot en `ArcSwap` + evento `IndexEvent::TagsChanged(Vec<TagKey>)`), para que la UI lea sin bloquear.
- **Buffers abiertos**: el índice refleja el disco, no el buffer sin guardar. El tag que escribes aparece en el panel al guardar. Indexar cada tecla haría que el panel baile mientras escribes `#ru…` y obligaría a tener dos verdades.
- **Invariante** (proptest, como el de archivos): índice y grafo incrementales tras cualquier secuencia de crear, editar, mover y borrar notas y páginas == índice y grafo construidos de cero sobre el resultado.

### Estado (M2.5, 2026-10-18): núcleo, sin UI

En `metalmind-vault`, módulo `tags`. Lo que se implementó distinto de lo de arriba, y por qué:

- **La nota se identifica por su `VaultPath`**, no por un `NoteId`: el vault todavía no interna rutas, y la ruta ya es la identidad ([06](06-decisiones-de-fondo.md) §1). Cuando llegue el `NoteId` de M3, el cambio queda dentro del módulo.
- **En frío**: `Vault::open` escanea los tags en hilos propios (`std::thread::scope`, uno por núcleo, sin crate de pool) y devuelve el vault con el índice ya armado. La UI no espera porque abre el vault fuera del hilo de la ventana, como ya hace falta para el walk. Medido en el [presupuesto de rendimiento](../performance.md).
- **Publicación**: no hay `IndexEvent` aparte; cada `VaultUpdate` del watcher trae el `Tags` nuevo (índice y grafo juntos, en el mismo `ArcSwap` que `Vault::tags()`) y `tag_changes`, las claves cuyas notas, formas o página pueden haber cambiado. Así cambios de archivos y de tags nunca se desfasan.
- **Incremental**: `Created`/`Modified` reescanean la nota; `Removed` la quita; `Renamed` mueve sus ocurrencias sin releerla, salvo que entre o salga de la carpeta de tags. Si cambió alguna página, el grafo se rehace entero. El proptest de arriba corre sobre el mismo `refresh` del watcher.
- **Una nota que no se puede leer** (permisos, no es UTF-8) cuenta como nota sin tags, con un aviso en el log, hasta que vuelva a cambiar.
- **El frontmatter entero** (2026-10-24, 03 §4): el `TagIndex` guarda de cada nota y página cada clave escalar de primer nivel con su valor (`TagIndex::frontmatter`), del mismo scan y al día con el watcher; un renombre lo mueve sin releer.
- **Publicar sin copiar** (2026-10-25, la parte de tags de [#60](https://github.com/alex-glez-felix/metalmind/issues/60)): el `TagIndex` va en mapas persistentes (`imbl`), así que clonar `Tags` para publicar un lote o para `with_unsaved` comparte lo que no cambió y cada cambio copia solo la nota y los tags que toca. Las páginas y el grafo van en un `Arc`: son pocas y cambiar una ya rehace el grafo entero. Recorrer un mapa persistente cuesta más que uno de `std`, así que lo que no necesita todo el vault no lo recorre: un renombre mira solo las notas del tag y las páginas, y una exclusión `-#tag` borra sin armar otro conjunto.
- **La carpeta de tags** llega por `VaultOptions::tags_folder` (`tags` por defecto); leerla de `tags_folder` en `<vault>/.metalmind/config.yaml` queda para cuando exista la config del vault.
- **Ícono y color**: un `icon` es un emoji (símbolos con sus modificadores) o `set:nombre`; un `color` es un nombre (letras, números, `-`, `_`) o un hex de 3 o 6 dígitos. Si el nombre está en la paleta del tema lo decide la UI. Un valor que no es ninguno es un `TagProblem::InvalidValue` y el resto de la página vale.
- **Planes**: `plan_rename`, `plan_merge`, `plan_use_main_name` y `plan_page_change` devuelven un `TagPlan`: sus `Changes` (por archivo, las ediciones en orden, cada una con su línea y el texto que espera encontrar, que `FileEdits::apply` comprueba; los archivos nuevos; el renombre de la página) y los avisos (`Merges`, `RenamesAlias`, `AliasAbsorbs`, `NotPortable`, y los bloqueos `BothHavePages` y `AliasTaken`). Fusionar escribe la forma establecida del destino. Un padre que tras fusionar sería la página misma se quita, igual que un valor repetido en `tags:` o en `parents`. El renombre de un alias también reescribe los `parents` que lo nombran, para que sigan apuntando al mismo tag. La vista previa es de la UI: lista los `Changes` y, con `x`, quita una edición antes de aplicar.
- **Aplicar y deshacer** van en el núcleo, y el motor es el de M3 para renombrar notas (`RenamePlan::changes()`): `Vault::apply(&Changes)` lee cada archivo y comprueba que cada edición encuentre su texto antes de escribir nada, comprueba que los destinos estén libres, y luego edita (cada archivo de forma atómica y sin seguir enlaces, comparando otra vez justo antes), crea sin pisar y renombra con `RENAME_NOREPLACE`. Devuelve un `Applied` con el texto de antes y el de después de cada archivo; `Vault::undo` devuelve cada uno a su texto de antes, borra lo creado (y las carpetas que se crearon para eso, si quedaron vacías) y deshace los renombres, salvo lo que cambió después de aplicar, que queda en el reporte. Si algo falla a medias, el error trae el `Applied` de lo hecho, para deshacerlo. No compara `mtime`, tamaño ni hash: cada edición lleva el texto que reemplaza, así un cambio en otra parte del archivo no la invalida y un `touch` tampoco. Para eso el índice guarda la línea de cada ocurrencia y el texto de lo que se borra al quitarla de una lista.
- **Buffers sin guardar**: `Tags::with_unsaved` da los tags como si esos buffers estuvieran guardados. Un plan armado sobre eso trae, para esos archivos, ediciones sobre el texto del buffer; la UI las aplica al buffer como una transacción (entran en su undo) y pasa el resto a `Vault::apply`.
- **Metadata desde una acción** (`tags::set_color`, `tags::set_icon`, `tags::edit_aliases`, `tags::edit_parents`, `tags::set_template`): `plan_page_change` reemplaza el campo entero, `clave: valor` con las líneas de su valor, y deja el resto del frontmatter byte a byte; un campo nuevo va antes del `---` de cierre, y una página sin frontmatter recibe uno antes de su cuerpo, tras el BOM y con sus finales de línea. Las listas se escriben entre corchetes y los valores van entre comillas solo si YAML los leería distinto (`"2024"`, `"#d97706"`). Si el YAML está roto o no es un mapa en bloque (`{color: rojo}`), el plan se niega y la página hay que arreglarla a mano: reescribirla perdería lo del usuario. Si el tag no tiene página, crea `tags/<forma establecida>.md` con solo ese campo; un alias cambia la página de su tag. `plan_create_page` crea la página vacía ("Agregar descripción") y `page_path` dice dónde está o dónde estaría.
- **Focus**: `Scope::from_words` lee las palabras del argumento de `:focus` (una carpeta y una consulta), ya separadas por la línea de comandos, y `Scope::filter` da un `ScopeFilter` con `contains(ruta)`, que el finder, la búsqueda, el panel de carpetas y los backlinks usan para filtrar. Una página de tag solo está en el alcance con `:focus tags/`. `new_note_tags` da los tags de una nota nueva en ese focus.
- **Panel**: `Tags::tree(&filtro)` arma la vista de tags dentro del alcance: raíces, hijos en el orden del panel, conteo, forma establecida, color, ícono y las marcas de página y de solo padre; en focus quedan solo los tags con notas del alcance. El conteo es una pasada por las notas: cada una suma a sus tags y a sus ancestros, así cuenta notas distintas. No se recalcula solo en los tags afectados, como decía arriba: armarlo entero cuesta 1.2 ms con 5 k notas y 28 ms con 50 k ([presupuesto](../performance.md)), menos que llevar la cuenta de qué afecta a qué. Las notas de un tag las da `tagged_notes` al expandirlo, no el árbol, porque ordenarlas todas costaba más que el resto. El ciclo se marca en la UI, que conoce el camino desde la raíz. El buzón es `TagTree::inbox`.
- **Autocompletado**: `Scan::tag_at(byte)` de `metalmind-syntax` dice si el cursor está sobre un tag (en el cuerpo, en `tags:` o en `aliases`/`parents`) y qué rango reemplazar; sirve también para el menú del tag sobre el texto y para `ctrl`+clic. `Tags::completions()` da los candidatos, de más notas a menos, con su forma establecida, sus alias, color e ícono, y `TagCompletion::text(lugar)` el texto a insertar (en el YAML, entre comillas si hace falta). El filtrado difuso con `nucleo-matcher` es de `metalmind-search`.
- **Finder** (2026-10-25): `space f` lee lo escrito como `TagQuery::parse`: los tags (`#rust -#archivado`, `-#` para el buzón) dejan las notas que cumplen la consulta y el fuzzy busca el resto de las palabras entre ellas. Un tag que no existe o no es válido se pinta con `ui.picker_warning`: uno inválido no deja ninguna nota, y uno que no existe no deja ninguna si se incluye y no quita ninguna si se excluye, como en `TagQuery`. El aviso sale de `TagQuery::words`, la misma lectura que la consulta. El picker lo recibe como un `Narrowing`, que no sabe de tags; la consulta se resuelve en el fondo y se reusa mientras solo cambia la parte difusa.
- **Panel, vista de tags** (2026-10-25): `TagOutline` (en `metalmind-ui`, sin GPUI) aplana el `TagTree` en filas: cada tag con su forma establecida, su ícono emoji y su conteo; al abrirlo, primero sus hijos y luego sus notas (`tagged_notes`); al final, "Sin tags", el buzón. Una fila se identifica por su camino desde la raíz, así un tag con varios padres se abre en cada lugar por separado, y uno que ya está en el camino se marca como ciclo y no se abre otra vez. Las dos vistas comparten el rasgo `Outline` (moverse, abrir, cerrar, qué archivo abre la fila), así `j/k`, `h/l`, `gg/G`, `enter`, el clic y el doble clic van a la vista que se ve; crear, borrar y filtrar siguen siendo de la de carpetas, y renombrar (`r`) y mover (`m`) sobre un tag lo renombran o lo fusionan (ver abajo). El clic derecho en una nota abre el menú de abrir en un split. Cada vista tiene su scroll, y cambiar de vista cierra lo que se estaba preguntando o el menú, que eran de la otra. El árbol de tags se arma solo cuando la vista se ve: al abrir con carpetas no se arma, y un lote del watcher la rehace solo si se ve o al mostrarla, porque armarlo cuesta lo que el vault (28 ms con 50 k notas). Faltan: el color de la página (la paleta del tema no llega a la UI todavía), los íconos de un set (#41), `/` en esta vista, y abrir el panel en tags por defecto, que cambia los tests de `gui_smoke` que empiezan en el árbol.
- **Lista de notas** (2026-10-26, [#164](https://github.com/alex-glez-felix/metalmind/issues/164), paso 1 de la dirección D): una columna plegable (`space l`, el botón `≡` de la barra o `q` desde ella) a la izquierda de la nota, con las notas de una fuente: el buzón, todas, un tag con sus descendientes o una carpeta con sus subcarpetas, que se elige con `space v` o con un clic en la cabecera. El buzón y el tag son la misma `TagQuery` del finder (`-#` y `#tag`). Cada fila lleva el título (el primer `# ` o el nombre del archivo), un extracto, sus tags en su forma establecida, la carpeta y la fecha (`mtime`, "modificada" de §4), de la más reciente a la más vieja; título y extracto salen de `Summary` (`metalmind-vault`) y se leen en el fondo al abrir y con cada lote del watcher. `space f` o `/` filtran la lista en su lugar con la consulta del finder (`#rust -#archivado parser`, el mismo `TagFilter` y el mismo picker sin GPUI) y `esc` quita el filtro. `t` agrega tags al frontmatter de la nota elegida (`with_tags` de las plantillas, tocando solo ese campo) como un cambio de tags: no escribe bajo un buffer con cambios y `:tagundo` lo deshace. Se recuerda por vault en `ui.yaml` (si se abrió o se plegó, la fuente y el filtro); sin elección guardada arranca abierta solo en una ventana de al menos `layout.list_open_min_width` (1600 px), y eso no se guarda. Una carpeta renombrada sigue siendo la fuente y la fila elegida sigue a su nota renombrada. `ctrl-w h`/`ctrl-w l` van del panel lateral a la lista y a la nota. Faltan las vistas guardadas como fuente, que esperan a que se decida dónde se guardan (06 §10).
- **Autocompletado de `#` en el editor** (2026-10-25): en Insert, `Workspace::typed_tag` dice qué tag se está escribiendo con el cursor al final de su nombre: en el cuerpo con `LineStyle::tag_at` (el estilo incremental de la línea, que ya deja fuera el código), en el frontmatter con `Scan::tag_at` sobre el frontmatter solo; así no se escanea la nota en cada tecla. `aliases` y `parents` solo cuentan en una página de tag, y en el YAML solo un escalar sin comillas. `TagCompletions` filtra los candidatos de `Tags::completions` (que se arman de nuevo solo con otros tags) con `nucleo`, por nombre y alias, sin distinguir mayúsculas: primero los que empiezan con lo escrito y, dentro de cada grupo, los de más notas. Aceptar teclea la forma establecida (`TagCompletion::text`) en lugar de lo escrito, dentro de la misma pasada de Insert: un solo paso de undo, y `.` lo repite. Como el completado de nvim, si la forma empieza con lo escrito (`ru` → `rust`) solo teclea el resto; si no (`ru` → `Rust`), borra lo escrito y teclea la forma entera. Un lote del vault con la lista abierta (un autoguardado, otra nota que cambia) la vuelve a filtrar: el tag que eligió el usuario sigue elegido si todavía coincide; si no eligió, queda la mejor coincidencia. Atajos con la lista abierta (contexto `Editor && TagCompletion`): `tab` acepta (`editor::accept_tag_completion`), `ctrl-n`/`down` y `ctrl-p`/`up` eligen; `enter` sigue siendo un salto de línea y `esc` sale de Insert. Un clic en una fila la acepta, y la elegida muestra su tecla. La lista va bajo el tag o encima si abajo no cabe (si no cabe entera en ninguno, del lado con más lugar y con las filas que caben dentro del texto del panel), con los colores y medidas del menú y `layout.completion_rows` filas; no pasa del ancho de la ventana (se recorta el alias antes que el nombre, nunca el conteo ni la tecla) y sus filas solo se corren cuando la elegida sale de las que se ven.
- **Renombrar y fusionar desde la UI** (2026-10-26): `:tagrename #viejo #nuevo` y `:tagmerge #a #b` (`tags::rename`, `tags::merge`), o en la vista de tags `r` y `m` sobre un tag, que también están en su menú contextual ("Renombrar", "Fusionar con…") y preguntan el otro nombre en el panel. La ventana arma el plan con los tags de la sesión (`TagRequest::plan`) y muestra la vista previa (`TagChangePreview`, sin GPUI: una fila por edición, archivo nuevo o renombre, en el orden del plan; quitar una fila la saca de `changes()`; `blocked()` dice por qué no se aplica: un aviso que bloquea, archivos con cambios sin guardar o ningún cambio). Cada fila muestra solo las líneas que cambian, y bajo la lista va el cambio elegido entero, en `layout.tag_change_detail_lines` líneas fijas: en un campo de la página el valor nuevo es lo que se confirma. Aplicar llama a `Vault::apply` con lo que quedó; los buffers sin cambios se releen como cualquier cambio en disco y los documentos abiertos de la página renombrada la siguen (`Workspace::moved`, sin esperar al watcher). Los documentos abiertos sin cambios de lo que se escribió se releen enseguida, sin esperar al watcher. Lo aplicado queda en memoria para `:tagundo`, también si se paró a medias (si no se cambió nada, queda el registro anterior); `:tagundo` no deshace mientras algo de lo que tocó tenga cambios sin guardar, por la misma razón que aplicar. La vista previa vuelve a mirar los buffers sin guardar cuando cambia el workspace, así un autoguardado la desbloquea; el destino de un renombre de página también cuenta. Las teclas de la vista previa son acciones con nombre (`tag_change::*`, contexto `TagChange`).
- **El resto del menú del tag** (2026-10-26): "Nueva nota con este tag" (`a`, 06 §11), "Abrir su página" (`P`, `tags::open_page`, `:tagpage #tag`; si no existe, se crea vacía con su carpeta, como "Agregar descripción" de §7), y "Alias…" (`e`), "Padres…" (`p`), "Ícono…" (`i`) y "Plantilla…" (`t`), que preguntan en el panel el campo con lo que tiene ahora (`TagField::current`) y van por la misma vista previa, aplicar y `:tagundo` que renombrar (`TagRequest::Page` con `Tags::plan_page_change`). En `:` son `:tagaliases #tag [alias…]`, `:tagparents #tag [padre…]`, `:tagicon #tag [ícono]` y `:tagtemplate #tag [plantilla]` (`tags::edit_aliases`, `tags::edit_parents`, `tags::set_icon`, `tags::set_template`); sin valor, quitan el campo. Los nombres se separan con coma o espacio y el `#` es opcional; el ícono es un emoji o `set:nombre`, escrito a mano hasta que haya selector. Agregar un campo se ancla en el `---` de cierre, y el frontmatter de una página sin él, en su primera línea: un plan armado antes de otro cambio a la página (el watcher todavía no trajo el lote) falla como cualquier edición vieja en vez de escribir en otro lugar. Una página vacía no tiene dónde anclar ([#148](https://github.com/alex-glez-felix/metalmind/issues/148)).
- **Focus en la UI** (2026-10-26): el alcance de la sesión vive en la `VaultSession` de la ventana, que lo resuelve contra el índice y los tags de cada lote del watcher en un `InScope` (los archivos del alcance y las carpetas que los contienen); el finder lista solo esas notas, la búsqueda de texto busca solo en ellas (también en los buffers sin guardar, y abierta vuelve a buscar si un lote cambia el alcance), la vista de carpetas esconde lo demás (su filtro busca solo dentro) y la de tags arma su árbol y sus notas con el `ScopeFilter`. `:focus` lo lee el `Workspace` con `Focus::parse` (un error no cambia nada; sin argumento dice el de ahora) y `:unfocus`, `F` sobre el mismo nodo (`panel::toggle_focus`, también en el menú de una carpeta o un tag como "Enfocar"/"Salir del focus") o un clic en el indicador lo quitan. Si un lote del watcher renombra o mueve la carpeta del focus, el focus la sigue (`Focus::follow`) y el indicador cambia con ella. El indicador sale en la barra de estado de cada panel, bajo el encabezado del panel lateral y antes de lo escrito en el finder y la búsqueda, con `ui.focus_indicator`; si no cabe, su tooltip lo muestra entero. Una nota nueva en focus (el picker de `:template` y `space n`, "Nueva nota con este tag" o `a` en el panel) nace, si el focus tiene carpeta, dentro de ella, y si tiene tags, con los incluidos y la plantilla que pida alguno; con solo tags, la carpeta es la de siempre (lo escrito en la pregunta del nombre, o la elegida en el panel). Una plantilla elegida no le pone el tag que la pide si el focus lo excluye (o es `-#`). `a` en el panel nunca crea fuera de la carpeta del focus y lee las plantillas solo si hay tags del focus (`new_note::create_in_panel`). En el finder y la búsqueda, un focus largo se recorta a `layout.picker_detail_share` de la fila, para no tapar lo que se escribe. Una carpeta con espacios se escribe con `\ ` o entre comillas, y `:focus` autocompleta tags y carpetas (ver "Modo focus", 2026-10-26).
- Falta: el color (la paleta del tema no llega a la UI todavía) y los selectores de color e ícono, el menú sobre un tag del texto y `space T`, la vista del tag, los buffers sin guardar al renombrar ([#141](https://github.com/alex-glez-felix/metalmind/issues/141)) y leer `tags_folder` de la config del vault.

## 3. UI

### Una GUI, no una TUI

Metalmind es una GUI con navegación estilo vim ([06](06-decisiones-de-fondo.md) §13). Para los tags, eso quiere decir:

- **Clic derecho sobre un tag**, en el panel o en el texto (un `#tag` del cuerpo o un valor de `tags:`), abre un menú con: color (selector de color), ícono (selector de emoji e íconos), alias, padres, renombrar (§5), fusionar (§5) y abrir su página (§7). Cada entrada es una acción con nombre (`tags::set_color`, `tags::set_icon`, `tags::edit_aliases`, `tags::edit_parents`, `tags::rename`, `tags::merge`, `tags::open_page`), y por eso también se llega desde la paleta, desde `:` y con atajos. Con el cursor sobre un tag, la misma lista sale con `space T`.
- **Metalmind crea la página sola** en cuanto le pones metadata desde la UI: elegir un color para `#rust` escribe `tags/rust.md` con `color:` en el frontmatter. El usuario nunca necesita crear la página a mano, aunque puede.
- **Escribir un `#tag-nuevo` no crea página.** `tags/` se llena solo con los tags que de verdad tienen algo que decir.
- **El archivo es el almacenamiento, no la interfaz.** Lo que se cambia desde la UI se escribe en el frontmatter de la página, tocando solo el campo editado (regla de `CLAUDE.md`), y editar ese frontmatter a mano da el mismo resultado.
- Quitar toda la metadata desde la UI no borra la página: puede tener cuerpo, y borrar un archivo del usuario es una decisión suya.
- Al crear un tag que no existía, un aviso no intrusivo junto al tag ofrece darle color o ícono, y se ignora sin hacer nada.

### Panel lateral

Dos vistas en el mismo panel, tags y árbol de carpetas (el de M2). Una sola está visible a la vez. Desde el 2026-10-26 hay una tercera, los backlinks de la nota activa ([03](03-core-architecture.md) §4), y `T` pasa por las tres.

- **Por defecto: tags.** Es el eje de organización; abrir la app en la vista de carpetas diría lo contrario. La vista elegida se recuerda por vault. Hasta que llegue el índice de tags (M2.5), el panel abre en carpetas.
- Cambiar de vista: `T` dentro del panel (`panel::toggle_view`); `space t` abre el panel en tags y `space e` en carpetas, con el foco, y si el panel ya muestra esa vista con el foco, lo esconden (`panel::toggle_tags`, `panel::toggle_folders`); si la muestra sin el foco, se lo dan, como `ToggleFocus` de Zed (2026-10-18: antes la escondían, y `j` y `enter` tras `space e` iban al editor). Lo mismo con los dos botones de la barra a la izquierda del panel. El encabezado del panel dice cuál es.
- **Raíces**: los tags sin `parents`. Bajo un tag, primero los tags que lo tienen como padre y luego sus notas, como una carpeta con subcarpetas y archivos. Un tag con tres padres aparece bajo los tres, y una nota con tres tags en tres lugares: es justamente lo que una carpeta no permite.
- Cada nodo muestra el nombre del tag, su ícono y su color si los tiene, y el conteo.
- Al final, el **buzón**: las notas que no tienen ningún tag ([06](06-decisiones-de-fondo.md) §9). Sin él, una nota sin tags es invisible en la vista por defecto. Las páginas de tag nunca están en el buzón (§7).
- Cada nodo muestra la forma establecida del tag (§6): "Rust (10)" aunque una nota diga `#rust`.
- Orden: alfabético por la forma que se muestra, sin mayúsculas, como el de carpetas.
- Vim, igual que `FileTree` (03 §3): `j/k`, `h/l` colapsar/expandir, `gg/G`, `/` filtrar, `enter` abre la nota o la vista del tag (§7), `r` renombra el tag (§5), `F` enfoca (§3, focus). El contexto es `TagTree`. Con el ratón: clic abre, clic en el triángulo expande, clic derecho abre el menú del tag.

### Modo focus

Inspirado en el "workspace view" de Inkdrop: enfocarse reduce **toda la app** a un subconjunto del vault.

**Qué es un alcance (`Scope`)**: una carpeta, una consulta de tags (§4) o las dos a la vez (`:focus proyectos/ #rust`), que se intersectan.

**Cómo se activa**:

| Desde | Cómo | Acción |
|---|---|---|
| `:` | `:focus #rust`, `:focus #rust #nota -#archivado`, `:focus proyectos/` | `workspace::focus` |
| Panel | `F` o el menú contextual sobre un tag o una carpeta | `workspace::focus` con ese nodo |
| Salir | `:unfocus`, `F` sobre el mismo nodo o clic en el indicador | `workspace::clear_focus` |

Una carpeta se distingue de un tag porque no empieza con `#`; el `/` final es opcional. `:focus` sin argumentos muestra el alcance actual en la línea de comandos. Autocompletado de tags y carpetas en `:focus`.

**Una carpeta con espacios** (2026-10-26, [#157](https://github.com/alex-glez-felix/metalmind/issues/157)): el espacio se escapa con `\`, como en los argumentos de vim (`:focus Mis\ notas/ #rust`); también se acepta entre comillas dobles (`:focus "Mis notas"/`), que cuesta lo mismo. `\` escapa cualquier carácter, así que `\\` es una barra y `\"` una comilla, y lo escapado o entre comillas al principio de la palabra no es un tag: `\#ideas/` es la carpeta `#ideas`. El indicador y el autocompletado escriben siempre con `\` (`focus: Mis\ notas/`), aunque se haya escrito con comillas, para que lo que se ve se pueda volver a escribir tal cual. Se descartó leer como carpeta todo lo que no es de tags: es más ambiguo (`:focus a b/` sería una sola carpeta `a b` sin avisar) y no sirve para `:e`. La regla de las palabras vive en `metalmind-vim` (`argument_words`, `escape_argument`), que es de la línea de comandos y no del vault; `Scope::from_words` recibe las palabras ya separadas.

**Autocompletado del argumento** (2026-10-26, [#156](https://github.com/alex-glez-felix/metalmind/issues/156)): tras `:focus `, la palabra que se escribe abre una lista bajo lo escrito, en el pop-up de la línea de `:` (desde [#193](https://github.com/alex-glez-felix/metalmind/issues/193); antes iba sobre la barra de estado): si empieza con `#` o `-#`, los tags con sus alias (los candidatos de `Tags::completions`, con el mismo filtro que `#` en una nota, y aceptar deja la forma establecida con su signo); si no, las carpetas del vault por su ruta, que se escriben escapadas y con `/`. Las teclas son las de la lista del editor: `tab` acepta (y la fila elegida lo muestra), `ctrl-n`/`ctrl-p` y las flechas eligen, y un clic acepta. Con la lista abierta, las flechas eligen en ella en vez de recorrer el historial; sin nada escrito en la palabra no hay lista, así que `:focus ` + flecha arriba sigue siendo el historial. `enter` corre lo escrito, como en vim, sin aceptar. La infraestructura es de cualquier comando: `Vim::typed_argument` dice qué comando y qué palabra se escriben, `Vim::complete_argument` la reemplaza, y el editor elige la lista por la acción (hoy solo `workspace::focus`; `:e`, [#45](https://github.com/alex-glez-felix/metalmind/issues/45), suma la suya ahí).

**Qué cambia dentro de focus**:

| Parte | Comportamiento |
|---|---|
| Finder (`space f`) | Solo candidatos del alcance. |
| Búsqueda full-text (M6) | Solo dentro del alcance. |
| Panel, tags | Solo los tags que aparecen en notas del alcance, con el conteo **dentro** del alcance: en focus `#rust`, `#async` dice cuántas notas de rust lo llevan. |
| Panel, carpetas | Solo carpetas con alguna nota del alcance; las vacías se esconden. |
| Backlinks (M3) | Solo los que vienen de notas del alcance, más una línea "N fuera de focus" que los muestra al abrirla. Esconderlos sin avisar haría creer que no existen. |
| Nota nueva | En focus de carpeta, se crea en esa carpeta. En focus de tags, nace con los tags incluidos (no los excluidos) en `tags:` del frontmatter; si no, desaparecería del focus en cuanto la creas. |
| Abrir una nota fuera del alcance | Se puede (`:e`, un link): focus filtra lo que se ofrece, no lo que se puede abrir. |

**Cómo se ve**: un indicador fijo en la barra de estado con el alcance escrito tal cual (`focus: #rust -#archivado`) y el encabezado del panel con el mismo texto, ambos con un token de tema propio (`ui.focus_indicator`). El finder lo repite en su prompt. Nunca debe haber duda de que estás viendo una parte del vault.

El focus dura la sesión; al reabrir, la app empieza sin focus. Recordarlo entre sesiones va a `IDEAS.md`.

## 4. Consultas

Sintaxis mínima, la misma en `space f`, en `:focus` y en el filtro del panel:

| Pieza | Significa |
|---|---|
| `#a` | La nota lleva `a`, un alias de `a` o un descendiente de `a` por `parents` (§7). |
| `#a #b` | Las dos (espacio = "y"). |
| `-#a` | No lleva `a`, ni sus alias, ni sus descendientes. |
| `-#` | No lleva ningún tag: el buzón ([06](06-decisiones-de-fondo.md) §9). Se lee "no `#` cualquiera"; las páginas de tag no salen. |
| `carpeta/` | Está bajo esa carpeta (solo en `:focus`, §3). |
| cualquier otra palabra | En `space f`, texto para el fuzzy de rutas, como hoy. |

- Ejemplo: `#rust #nota -#archivado` = notas de rust **y** de nota que **no** están archivadas.
- En `space f` se mezclan: `#rust parser` filtra por `#rust` y busca `parser` con fuzzy entre lo que queda.
- El tag de la consulta se compara con la clave normalizada (§6): `#Rust` encuentra `#rust`. Un alias en la consulta vale por su tag: `#rs` es `#rust`.
- Un `-#a` solo, sin ningún `#` positivo, vale: "todo menos `a`".
- Las páginas de tag no son resultados, salvo que se pida (§7).
- **No hay "o"**, ni paréntesis, ni comodines. Cubren casos reales pero convierten esto en un lenguaje; si hacen falta, se ven junto con las vistas guardadas (`IDEAS.md`).
- Un tag que no existe en el vault no es error: incluido (`#a`) da cero resultados y excluido (`-#a`) no quita ninguno. El prompt lo marca en el color de aviso del tema.
- **La forma `clave:valor` está reservada** (decidido el 2026-10-24, [07 §5](07-features-otras-apps.md#5-preguntas-y-respuestas), pregunta 4): `modified:7d`, `has:adjunto` y cualquier otra palabra con `:` son filtros, no texto para el fuzzy. Hoy no hay ninguno implementado, así que una palabra así es un error de consulta que el prompt marca; cuando llegue cada filtro, ninguna vista guardada ([06](06-decisiones-de-fondo.md) §10) ni salida `--json` cambia de significado. "Sin tags" ya existe como `-#`.
- **Fechas** (decidido el 2026-10-24, [07 §5](07-features-otras-apps.md#5-preguntas-y-respuestas), pregunta 3): "modificada" es el `mtime` del archivo; "creada" sale solo del campo `created:` del frontmatter (que el índice guarda entero, 03 §4), y una nota sin él no entra en un filtro por creación. Metalmind nunca escribe `created:` por su cuenta: sería un dato en cada nota, como el ID que 06 §1 descartó.
- En código, `TagQuery { include: Vec<TagKey>, exclude: Vec<TagKey>, untagged: bool }` y `Scope { folder: Option<VaultPath>, tags: TagQuery }`, con `fn notes(&self, &TagIndex, &TagGraph, &VaultIndex) -> NoteSet`. El parseo es una función pura con tabla de casos en `insta`.

## 5. Operaciones

### Autocompletado de `#`

- En Insert, al escribir `#` donde puede empezar un tag (inicio de línea o tras espacio) seguido de un carácter de tag, se abre la lista de tags del vault, filtrada con `nucleo-matcher` y ordenada por número de notas. La lista muestra el ícono y el color de cada tag.
- Los alias también se buscan: escribir `#rs` ofrece "Rust (alias rs)" y acepta `#rust`.
- También dentro de la lista `tags:` del frontmatter y de `parents` en una página de tag.
- No aparece en bloques de código ni en código inline. `# ` con espacio cierra la lista: es un heading.
- La lista ofrece e inserta la **forma establecida** (§6), no la que escribiste a medias: escribir `#ru` y aceptar deja `#Rust` si así lo escribe el vault. Así no nacen variantes nuevas. `ctrl-n`/`ctrl-p` y `tab` como en el resto de pickers; clic en una opción también la acepta.

### Renombrar un tag en todo el vault

Es una refactorización: reescribe archivos del usuario. Por eso va en tres pasos, nunca en uno.

1. **Pedir**: `:tagrename #viejo #nuevo` (`tags::rename`), `r` sobre el tag en el panel o "Renombrar" en el menú contextual.
2. **Vista previa**: una lista, como un picker, con cada cambio (archivo, línea, antes → después) y el total de archivos y ocurrencias. Entran las ocurrencias en notas, el renombre de la página `tags/viejo.md` → `tags/nuevo.md` y cada `parents` o `aliases` de otra página que nombra al tag. Se navega con `j/k` (o `ctrl-n/p` y las flechas); `x` o un clic quita ese cambio, y otra vez lo devuelve. `enter` o el botón "Aplicar" aplica; `esc`, `q` o "Cancelar" cancela sin tocar nada. Cambiado el 2026-10-26: era `:w` para aplicar y `enter` para abrir la línea, pero la vista previa no es un buffer y no tiene línea de comandos; abrir la línea de un cambio queda en [#142](https://github.com/alex-glez-felix/metalmind/issues/142).
3. **Aplicar**: cada archivo se reescribe solo en los rangos de `by_note` (y en los de las páginas), con escritura atómica; la página se renombra con un rename atómico. Se valida antes que el archivo en disco no cambió desde el escaneo (mismo `mtime`+tamaño y hash); si cambió, se reescanea ese archivo y se avisa en vez de escribir a ciegas.

Detalles:

- **Hijos**: los tags con `a` en `parents` siguen bajo `b`, porque esos `parents` se actualizan en el mismo cambio.
- **Alias**: las ocurrencias escritas con un alias (`#rs`) no se reescriben y el alias se queda en la página renombrada: renombrar cambia el nombre del tag, no sus alias. Para eso están las dos operaciones de abajo.
- **Frontmatter**: se reemplaza solo el texto del valor en su rango, sin re-serializar el YAML; el resto del bloque queda byte a byte igual (regla de `CLAUDE.md`: nunca reescribir el YAML del usuario salvo el campo editado). Vale para `tags:` en notas y para `parents`/`aliases` en páginas.
- **Mayúsculas**: renombrar reescribe **todas las variantes** del tag (`#Rust`, `#rust`, `#RUST`) a la forma nueva. Renombrar `#Rust` a `#rust` es válido aunque la clave sea la misma: es la manera de unificar cómo se escribe un tag. Es la única vez que Metalmind toca la forma escrita; mostrar la forma establecida nunca reescribe archivos.
- **Nombre nuevo inválido**: el nombre nuevo pasa por la misma validación del scanner (§1); con `/` o con cualquier carácter que corta un tag, la vista previa no se abre y se dice por qué.
- **Buffers abiertos con cambios sin guardar**: el cambio se aplica al buffer como una transacción (entra en su undo) y el archivo queda modificado sin guardar; no se escribe el disco por debajo del buffer. Buffers abiertos sin cambios: se escribe el disco y el buffer se recarga. Mientras no esté ([#141](https://github.com/alex-glez-felix/metalmind/issues/141)), la vista previa marca esos archivos y no aplica hasta que se guarden o se quiten sus filas: nunca se escribe por debajo de un buffer.
- **Deshacer**: la operación entera, renombre de la página incluido, es una sola transacción de vault, con el contenido previo de cada archivo tocado guardado en memoria durante la sesión. `:tagundo` (`tags::undo`, también en la paleta) la revierte; el botón "Deshacer" en el aviso que queda al aplicar está en [#142](https://github.com/alex-glez-felix/metalmind/issues/142). Deshace el último cambio de tags, sea renombrar o fusionar. Si un archivo cambió después del renombre, ese archivo no se revierte y se lista; los demás sí. Es el mismo motor que necesita M3 para "renombrar nota actualiza links" (03 §4), y se diseña una vez para los dos.
- **Fallo a medias**: cada archivo es atómico, el conjunto no. Si una escritura falla, se para, se informa qué quedó escrito y se ofrece deshacer lo aplicado.

### Quitar un tag de todo el vault

`:tagremove #a` (`tags::remove`) o "Quitar de todas las notas" en el menú contextual borra el tag de cada nota que lo lleva, sin borrar ninguna nota. Es renombrar hacia "nada", con el mismo flujo de tres pasos, la misma vista previa y el mismo deshacer. Se quitan todas las variantes de mayúsculas y las ocurrencias escritas con un alias, porque un alias vale por su tag (§4). En `tags:` se borra el valor con el rango de borrado que guarda el scanner (03 §2); en el texto, el `#nombre`, y la vista previa enseña cómo queda cada línea. La página `tags/a.md` y los `parents` que la nombran no se tocan: borrarlos es otra acción. Decidido el 2026-10-24 para M2.5 ([07 §5](07-features-otras-apps.md#5-preguntas-y-respuestas), pregunta 8).

### Aviso al crear un tag nuevo

Escribir un tag que el vault todavía no tiene (ninguna nota, página ni alias lo usa) lo marca en el color de aviso del tema, con un tooltip que dice que es nuevo. No impide nada: evita `#rsut` cuando ya existe `#rust`, junto con el autocompletado. Viene encendido y se apaga en los ajustes. Decidido el 2026-10-24 ([07 §5](07-features-otras-apps.md#5-preguntas-y-respuestas), pregunta 9); no entra en M2.5.

### Renombrar un alias

`:tagrename #rs #rst` sobre un alias, o "Renombrar" sobre el alias en el menú del tag, renombra el alias: cambia el valor en `aliases` de la página y **reescribe las notas que usan `#rs`** a `#rst`. Es el mismo flujo de tres pasos, la misma vista previa y el mismo deshacer que renombrar un tag. La vista previa dice que es un alias de `#rust`.

### Pasar al nombre principal

`:tagmain #rs` (`tags::use_main_name`), o "Pasar al nombre principal" sobre un alias en el menú del tag, reescribe todas las ocurrencias de `#rs` a `#rust`, para dejar de usar el alias. Mismo flujo con vista previa y deshacer. El alias se queda en la página: así un `#rs` escrito más adelante sigue contando como `#rust`. Quitarlo es otra acción del menú (`tags::edit_aliases`).

### Fusionar dos tags

- `:tagmerge #a #b` (`tags::merge`) o "Fusionar con…" en el menú contextual: todas las ocurrencias de `#a` pasan a `#b`. Es renombrar hacia un tag que ya existe, con el mismo flujo de vista previa y deshacer.
- `:tagrename` hacia un tag existente avisa en la vista previa ("`#b` ya existe: se fusionan") en vez de negarse.
- Si los dos tienen página, la vista previa lo marca como bloqueo y no aplica hasta que quede una; juntar dos frontmatter y dos cuerpos es decisión del usuario. Si solo `a` tiene página, pasa a ser la de `b`.
- Si una nota termina con el mismo tag dos veces en la lista `tags:`, el duplicado se quita de esa lista; el resto del frontmatter no se toca. En el texto, dos `#b` en el cuerpo se dejan: son dos menciones, no un dato repetido.

### Un tag incluye a sus descendientes

Filtrar, enfocar o consultar por `#a` incluye a los tags que tienen `a` entre sus `parents`, y a los de ellos, de forma transitiva (§4, §7). En el panel, el nodo de `a` cuenta las notas de todos ellos. No hay forma de pedir "solo `a` exacto, sin descendientes"; si hace falta, va a `IDEAS.md`.

## 6. Casos borde

| Caso | Decisión | Por qué |
|---|---|---|
| `#Rust`, `#rust` y `#RUST` | **El mismo tag** para agrupar, filtrar, consultar, focus y el panel | Nadie quiere dos tags que solo se distinguen por la mayúscula inicial de una frase. La clave usa el *case folding* por defecto de Unicode (`CaseFolding.txt`, no solo ASCII y no `str::to_lowercase`), con `caseless` 0.2.2 (crates.io, 2026-10-18): así `#STRASSE` y `#straße` también son el mismo. Es el folding sin locale, el mismo en cualquier máquina; la `ı` turca sin punto no se une a `i`. |
| Cómo se muestra | **La forma más usada** | El panel, el finder y los chips enseñan la variante que más ocurrencias tiene en el vault: con 9 `#Rust` y 1 `#rust`, "Rust (10)". Empate: la primera en orden de bytes (`Rust` antes que `rust`), así no cambia según el orden del walk. El nombre de la página cuenta como una ocurrencia más, así un tag con página y sin notas se muestra como su archivo. Se recalcula con el índice. **Nunca se reescribe un archivo por esto**; solo renombrar (§5) cambia lo escrito. |
| `#2024/10` | **Es `#2024`** seguido de `/10` | La `/` no cabe en un nombre de archivo y no forma parte del tag (§1). Para un mes, `#2024-10`; para que quede bajo `#2024`, su página lo dice con `parents`. |
| `#canción` y `#cancion` | **Distintos** | Los acentos sí importan: en español cambian la palabra (`#año` y `#ano`). Quitarlos uniría cosas distintas sin que nadie se entere. |
| `#canción` con `ó` compuesta o descompuesta (U+00F3 vs `o` + U+0301) | **El mismo tag** | Solo se normaliza lo invisible: se ven idénticos y llegan distintos según el teclado o el sistema de origen del archivo. La clave es NFC (`unicode-normalization` 0.1.26, crates.io, 2026-10-18), luego folding, luego NFC otra vez, porque el folding puede dejar una cadena que ya no está en NFC. El archivo no se toca: solo la clave. |
| `#tag` en un bloque cercado, en código indentado o en `` `código inline` `` | **No cuenta** | Es código, no una nota sobre el tema: `#include`, `#[derive]` y comentarios de shell llenarían el índice. Lo resuelve el scanner (03 §2), que ya ignora código. |
| `#tag` dentro de `%% comentario %%` | **No cuenta** | El scanner ya los ignora (03 §2); un comentario no es contenido. |
| `#tag` en una URL o en `[[Nota#Sección]]` | No es tag | No va tras espacio o inicio de línea. |
| `#2024` | **Sí es tag** | Agrupa un año, y en Metalmind los tags son el eje de organización (§1). Costo aceptado: `PR #12` deja un tag `#12`. |
| Mismo tag en el texto y en `tags:` | La nota cuenta una vez | El índice es por nota; las dos ocurrencias se guardan para renombrar. |
| Frontmatter roto | Los tags del cuerpo cuentan igual | El scanner trata el YAML roto como problema recuperable (03 §2). |
| Tag de más de 250 bytes | No es tag | Su página no cabría en un nombre de archivo (§1). |
| Grafo muy profundo | Sin límite | Un límite es un caso especial más; si un vault real lo necesita, se mide primero. |

Los casos de las páginas de tag están en §7.

## 7. Páginas de tag

Inspirado en TagStudio, donde los tags tienen metadata y "tags de tags". Un tag puede tener una página: un archivo markdown que guarda su metadata y explica de qué trata. **Es opcional**: un tag sin página funciona igual que siempre. Con página, el tag es también un lugar: al abrir `#rust` se ve su descripción arriba de sus notas.

### Dónde y cómo se llama

- `tags/<nombre>.md`, en una carpeta **plana**: sin subcarpetas. La carpeta es un ajuste del vault, como la de adjuntos ([06](06-decisiones-de-fondo.md) §6), con `tags/` por defecto, en la clave `tags_folder` de `<vault>/.metalmind/config.yaml`.
- La página se identifica por el **nombre del archivo** sin `.md`, con las mismas reglas de identidad de §6: sin distinguir mayúsculas, con acentos y en NFC. `tags/Rust.md` es la página de `#rust`.
- Los archivos en subcarpetas de `tags/` (`tags/viejo/x.md`) no son páginas: son notas normales.
- Normalmente no se crea a mano: Metalmind la crea en cuanto le pones metadata desde la UI (§3). Crearla a mano da lo mismo.

### Formato

```markdown
---
color: naranja
icon: tabler:brand-rust
aliases: [rs]
parents: [lenguajes, programacion]
---
Notas sobre Rust: el lenguaje, sus crates y lo que voy aprendiendo.
Lo de GPUI vive en su propio tag, #gpui, que tiene a #rust como padre.
```

| Campo | Qué es |
|---|---|
| `color` | El color del tag en el panel, en los chips y en el `#tag` del texto. Un nombre de la paleta del tema (`rojo`, `naranja`, `azul`…, tokens `tag.color.*`) o un hex entre comillas (`"#d97706"`). Con nombres, el color se adapta a temas claros y oscuros; el hex queda para quien lo quiera exacto. Sin comillas, `#d97706` es un comentario de YAML, y por eso la UI siempre escribe nombres o hex entre comillas. |
| `icon` | Un emoji (`🦀`) o un ícono con prefijo de set (`tabler:book`). Se muestra delante del nombre en el panel, en el autocompletado y en la vista del tag. |
| `aliases` | Otros nombres del mismo tag: con `aliases: [rs]`, `#rs` cuenta como `#rust` para filtrar, consultar, contar y en el panel. El texto no se toca: la nota sigue diciendo `#rs`. |
| `parents` | Los padres del tag, **varios**. De aquí y solo de aquí sale la jerarquía. |
| `template` | El nombre de una plantilla ([06](06-decisiones-de-fondo.md) §11): una nota nueva con este tag (o con uno de sus alias), o creada en focus de él, nace con ella. Un texto que es una ruta del vault sin `.md`; una lista es un problema del frontmatter y un nombre como `../x`, un valor inválido de la página. No se hereda de los `parents`. |

- `aliases` y `parents` aceptan una lista o un solo texto, y un `#` delante se ignora, igual que `tags:` (§1).
- Un `parents` puede nombrar un alias: `parents: [rs]` es `parents: [rust]`.
- **Solo los tags tienen alias.** Los alias se declaran solo en la página del tag. En una nota normal, `aliases:` es una clave más del YAML: no resuelve `[[links]]` (M3) ni entra en búsquedas.
- Las claves desconocidas se ignoran y se conservan, como en cualquier frontmatter.
- **Ícono: emoji o ícono con prefijo de set.** `icon` acepta:
  - Un **emoji** (`icon: 🦀`): trae sus propios colores y se pinta con la fuente de emoji del sistema.
  - Un **ícono con prefijo de set**, `<set>:<nombre>` al estilo de [Iconify](https://iconify.design) (`icon: tabler:book`, `icon: tabler:book-filled`, `icon: ph:books`). El prefijo deja lugar a varios sets sin que un nombre de ícono se confunda con un emoji.
  - **Set incluido por defecto: [Tabler](https://tabler.io/icons)**, que viene con Metalmind. Lucide, del que salen los íconos de Zed, se descartó como set por defecto por limitado: tiene 2159 íconos, contra los más de 6000 de Tabler.
  - **Sets extra del usuario**: SVGs en `<config>/icons/<set>/*.svg` (`Paths::config` de `metalmind-config`), con el nombre del archivo como nombre del ícono: `~/.config/metalmind/icons/ph/books.svg` es `ph:books`. Así se agregan Phosphor (MIT), Material Symbols (Apache-2.0) o Lucide (ISC), todos compatibles con GPL-3. Metalmind no los descarga ni los trae; la carpeta es del usuario. Un set del usuario no puede llamarse `tabler`: esa carpeta se ignora y se avisa.
  - **Color**: los íconos de línea y los rellenos se pintan en el `color` del tag (sin color, en el color de texto del tema) y quedan parejos con la UI. Un SVG multicolor de un set del usuario se pinta en un solo color (ver GPUI, abajo).
  - El **selector de ícono** de la UI (§3) busca a la vez en los emoji y en todos los sets cargados, con `nucleo-matcher`, y muestra el set de cada resultado.
  - Un `icon` que no es ni un emoji ni un `<set>:<nombre>` válido es un problema y el tag se muestra sin ícono. Un set que no está en esta máquina (la página dice `ph:books` y nadie dejó Phosphor en la config) no es un problema de la página: el tag se muestra sin ícono, con un aviso en el selector, y el campo no se toca. La config es por máquina y el vault se sincroniza; la página no puede quedar mal por eso.
- **Lo que implica Tabler (datos de 2026-10-18):**
  - **Licencia**: MIT (`@tabler/icons` en npm y el repo `tabler/tabler-icons`). Es compatible con GPL-3; basta con incluir el aviso de copyright en el binario y en los paquetes (`packaging/`). `cargo-deny` no lo ve porque no es un crate.
  - **Cuánto**: `@tabler/icons` 3.49.0 trae 5184 íconos de línea y 1054 rellenos. El paquete de npm ocupa 11,3 MB sin comprimir, pero solo 3,65 MB son SVG (2,90 MB de línea y 0,75 MB rellenos); el resto son otros formatos.
  - **Peso comprimido**: los 6238 SVG concatenados quedan en ~365 KB con gzip -9 y ~290 KB con zstd -19. Para Metalmind, ~300–400 KB más en el binario, frente a unos 3,7 MB embebidos sin comprimir.
  - **Embebido o como recurso**: recomendación, embebido y comprimido con gzip. `flate2` (1.1.10) ya está en el `Cargo.lock` como dependencia transitiva, así que no agrega un crate al árbol, y un solo binario sigue funcionando sin instalar nada (CLI, AppImage, tarball). Como recurso aparte (`/usr/share/metalmind/icons/tabler.tar.gz`), el binario no crece, pero cada paquete tiene que instalarlo y Metalmind tiene que buscarlo. El set se descomprime la primera vez que hace falta (abrir el selector o pintar un ícono) y queda en memoria; solo se rasteriza lo que se ve en pantalla.
  - **GPUI ya pinta SVG**: `svg()` de `gpui-pre` (0.3.9, con `resvg`/`usvg` 0.46 en el `Cargo.lock`) lee el SVG desde un `AssetSource` o desde bytes, y lo pinta como **máscara de un color** con el `text_color` del elemento. Eso encaja con Tabler, cuyos íconos de línea usan `stroke="currentColor"` y los rellenos `fill="currentColor"`, y es justamente lo que pinta un ícono en el color del tag. No hace falta ningún crate nuevo. Un SVG multicolor de un set del usuario pierde sus colores; eso se documenta, no se arregla con un segundo camino de pintado.

### Padres: un grafo, no un árbol

- `gpui` puede tener como padres a `rust`, `graficos` y `ui` a la vez, y aparece bajo los tres en el panel.
- Los padres sirven para **filtrar y buscar**: `#rust` trae también las notas de `#gpui`, y las de los hijos de `gpui`, de forma transitiva (§4, §5).
- **Ciclos**: `a` con padre `b` y `b` con padre `a` (o cualquier vuelta más larga) se detectan al construir el `TagGraph`, se avisan y no rompen nada. Los descendientes se calculan con un conjunto de visitados, así que `#a` trae `b` y `#b` trae `a`, sin vueltas infinitas. En el panel, un tag que ya está en el camino desde la raíz se muestra con una marca de ciclo y no se expande otra vez; si ningún tag del ciclo tiene un padre fuera de él, todos salen como raíces para que no desaparezcan del panel. El aviso sale en el panel y en las páginas del ciclo, con el camino escrito (`a → b → a`).
- Una página que se pone a sí misma como padre: se ignora ese padre y se avisa.

### La vista del tag

Abrir `#rust` (desde el panel, con `tags::open` o `ctrl`+clic sobre un `#rust` del texto) muestra la vista del tag: el nombre con su ícono y su color, sus alias y sus padres, el cuerpo de la página y debajo la lista de sus notas. El cuerpo es la página misma, abierta como buffer: se edita como cualquier nota. Si el tag no tiene página, la vista muestra solo la lista y el botón "Agregar descripción", que crea la página (§3).

### Qué no es una página

- Las páginas **no salen en el buzón** aunque no tengan tags ([06](06-decisiones-de-fondo.md) §9).
- **No cuentan como notas normales** en búsquedas ni en conteos, salvo que se pida: `:focus tags/`, la vista de carpetas o una opción del finder y de la búsqueda full-text las muestran.
- Los `#tag` del cuerpo de una página no suman al conteo de esos tags, pero sí se reescriben al renombrar.
- Un `[[tags/rust]]` es un link a un archivo como cualquier otro (M3).

### Casos borde

| Caso | Decisión | Por qué |
|---|---|---|
| Página sin frontmatter | **Vale.** Define el tag por su nombre de archivo, sin metadata; el cuerpo es su descripción. | Escribir de qué trata un tag no debería exigir YAML. |
| Página con frontmatter roto | El tag existe y conserva su descripción; la metadata se ignora y queda como problema recuperable | Igual que en una nota (03 §2): un YAML roto no tumba lo demás. |
| Un alias igual al nombre de otro tag existente, es decir, con página propia (`rust.md` dice `aliases: [rs]` y existe `tags/rs.md`) | **El alias no aplica**: `#rs` es el tag `rs`, y se avisa en la página que declara el alias | Una página es una declaración explícita de que el tag existe por sí mismo. Si el alias ganara, la página de `rs` quedaría muerta sin que nadie la haya borrado. |
| Un alias que se usa en notas pero no tiene página (`#rs` en 20 notas) | **El alias aplica**: esas 20 notas cuentan como `#rust` | No es otro tag existente: es el uso del alias, justo para lo que sirve. Si contara como tag existente, un alias dejaría de funcionar en cuanto alguien lo escribiera. |
| Un alias que choca con otro alias (`rust.md` y `rescript.md` dicen `aliases: [rs]`) | **Ninguno aplica**: `#rs` queda como tag propio y se avisa en las dos páginas | Elegir uno adivinaría; dejarlo suelto es visible y no mete notas de rescript en rust. |
| Un alias igual al nombre de su propia página | Se ignora sin avisar | No cambia nada. |
| Un padre que no existe como tag ni como página (`parents: [graficos]` y nadie escribe `#graficos`) | **Vale**: `graficos` aparece en el panel como nodo que agrupa a sus hijos, con una marca de "solo existe como padre" en el color de aviso del tema | Es una forma legítima de crear una categoría sin página. La marca deja ver un typo (`parents: [rsut]`) sin bloquear a quien lo hizo a propósito. |
| Ciclo de padres | Se detecta, se avisa y no rompe nada | Ver "Padres: un grafo, no un árbol". |
| Archivo cuyo nombre no es un tag válido (`tags/mi tag.md`, `tags/a.b.md`, un nombre de más de 250 bytes) | No es página; es una nota normal y se lista como problema | No existe un `#tag` que lo nombre. |
| Dos archivos con la misma clave (`tags/Rust.md` y `tags/rust.md`, o `ó` compuesta y descompuesta) | Vale el primero en orden de bytes y se avisa en los dos | Mismo desempate que la forma escrita (§6): estable, sin depender del orden del walk. |
| Mover o renombrar `tags/rust.md` fuera de Metalmind | El watcher lo ve como cambio de página: `rust` se queda sin página y aparece la del nombre nuevo. Las notas no se reescriben | Inferir un renombre de tag desde un cambio de archivo reescribiría notas sin vista previa. El renombre de verdad es `tags::rename`. |
| **Renombrar un tag con página** | Se renombra la página y se actualizan los `parents` y `aliases` de las otras páginas que lo nombran, en la misma vista previa y el mismo deshacer que las notas | Ver §5. |

## 8. Fuera de M2.5

Van a [`docs/IDEAS.md`](../IDEAS.md#tags):

- **Vistas guardadas**: una consulta con nombre, como las carpetas inteligentes.
- "O" en las consultas, paréntesis, comodines y "solo este tag sin descendientes".
- Recordar el focus entre sesiones.
- Descargar sets de íconos desde la UI, en vez de dejar los SVG a mano en `<config>/icons/` (§7).
- Sugerir tags a partir del contenido.
- Que el índice siga al buffer sin guardar (§2).

No se hacen: tags inferidos de la carpeta (la carpeta y el tag son ejes distintos, y mezclarlos traería de vuelta la jerarquía única).

## 9. Hecho cuando…

El criterio de M2.5 en 03 §8. Desglosado:

- Fixture de casos borde del scanner, de las páginas de tag y del parseo de consultas en `insta` (§1, §4, §6, §7).
- Tabla de casos del `TagGraph`: alias que chocan con un tag con página o con otro alias, padre suelto, ciclo, varios padres; para cada uno, los descendientes de cada tag, los avisos y el panel resultante (§7).
- Proptest: índice y grafo incrementales == completos (§2).
- Test de renombrar `#a` → `#b` con página, con otras páginas que tienen `a` en `parents` y `aliases`, con frontmatter y texto: cada archivo cambia solo en los rangos esperados y la página cambia de nombre; deshacer deja todos los archivos byte a byte iguales al original y la página en su nombre viejo.
- Test de poner color e ícono desde una acción a un tag sin página: crea `tags/<nombre>.md` con solo esos campos; repetirlo sobre una página con frontmatter escrito a mano cambia solo ese campo.
- Las páginas no salen en el buzón, ni en `space f`, ni en los conteos, salvo que se pidan.
- Con el vault sintético de 5 k notas de M2 y 200 páginas de tag: índice y grafo completos en el arranque en < 200 ms **(objetivo)**, medido con `divan` como el de archivos; filtrar `space f` por un tag con descendientes en < 16 ms **(objetivo)**.
