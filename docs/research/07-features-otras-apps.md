# 07 — Features de otras apps: qué falta decidir

> [!WARNING]
> **Estado: pendiente de revisión de Alex.** Nada de este documento está decidido ni entra al roadmap. Las recomendaciones son solo eso: no implementes nada de aquí hasta que este aviso se quite.

> Investigación del 2026-10-18. Objetivo: decidir el alcance **ahora** y no descubrir a medio camino algo que cambie los cimientos. No repite el panorama de [01](01-landscape.md) (qué apps existen y en qué stack): aquí importan sus **features** y si nota las quiere.
> Parte de lo ya decidido en [05](05-tags.md) (tags) y [06](06-decisiones-de-fondo.md) (decisiones de fondo), y del roadmap de [03 §8](03-core-architecture.md#8-hitos-del-mvp). Este documento **no cambia** ninguno de los tres: todo lo de aquí es recomendación hasta que el autor decida (§5).

## Cómo se verificó

- Se revisó documentación oficial, changelogs y foros de las apps pedidas: Obsidian, Notion, Inkdrop, Evernote, Logseq, Bear, Apple Notes, Joplin, Standard Notes, Notesnook, Anytype, Capacities, Tana, Reflect, Craft, SiYuan y Zettlr. Del lado terminal y Emacs: zk, nb, Neorg, org-mode/org-roam, Denote y SilverBullet. Además Dendron y Foam por las jerarquías, y Trilium, Memos, UpNote y Heptabase, que aparecieron al buscar.
- Cada feature cita su fuente. Un link sin marca es una página que se abrió y se leyó. **(b)** quiere decir que la página salió en los resultados del buscador con ese contenido, pero no se abrió; el Help Center de Evernote, por ejemplo, devolvió 403. **(sin verificar)** quiere decir que no se encontró fuente.
- Clasificación:
  - **MVP**: recomendado para antes del primer release (M0–M6).
  - **Después**: post-MVP. Hoy no cuesta nada salvo la restricción que se indica.
  - **Baja**: deseable, sin fecha.
  - **No encaja**: no va, con la razón.
- "Cimientos" es formato de archivo, índice, identidad, sync o CLI. **Sí** quiere decir que hacerlo tarde obliga a cambiar uno de ellos, o que hay una restricción que respetar desde ya. **No** quiere decir que se puede agregar después sin costo.

---

## 1. Resumen ejecutivo: lo que hay que decidir

Las primeras once tocan cimientos y van primero. Las demás se pueden agregar después sin costo, pero conviene decidir ahora si entran.

| # | Feature | Recomendación | Por qué, en una línea |
|---|---|---|---|
| 1 | **Frontmatter completo en el índice**, como clave/valor genérico | **MVP (restricción)** | Hoy `Frontmatter` solo guarda `tags`, `aliases` y `cssclasses`. Guardarlo entero permite filtrar por cualquier campo y deja abierta una vista de tabla sin tocar los cimientos. Lo recomendado es que las bases de datos no entren (§2.2). |
| 2 | **Gramática de tags cerrada desde M2.5 y sin espacios** | **MVP** | Bear 2 cambió la suya y rompió los tags de sus usuarios. La sintaxis `#tag con espacios#` de Bear no la entiende ninguna otra herramienta. |
| 3 | **Consultas extensibles**: reservar `clave:valor` y agregar "sin tags", fechas, "solo este tag" y "tiene adjuntos" | **MVP** para la reserva y "sin tags"; **Después** para el resto | Bear, Evernote, Inkdrop y zk filtran por fecha, adjunto y título. Si la gramática no reserva sitio, cada filtro nuevo rompe consultas guardadas y el `--json` de la CLI. |
| 4 | **Fechas en consultas** a partir del `mtime`; "creada" solo si la nota la trae en el frontmatter | **MVP** | Sincronizar con Syncthing o git cambia el `mtime` y la fecha de creación del sistema de archivos no es fiable. Escribir `created:` en cada nota sería ensuciarla, como el ID que 06 §1 ya descartó. |
| 5 | **Un solo motor de refactor** para notas, carpetas y tags: incluye los links relativos salientes, quitar un tag de todo el vault y reparar renombres hechos fuera | **MVP** | Obsidian lleva años sin arreglar los links relativos salientes al mover una nota, y zk no tiene renombrar. Con "la ruta es la identidad", este motor es lo que sostiene todo. |
| 6 | **Metadatos de tag** (fijar arriba, color, ícono, alias, vocabulario) en la config del vault, indexados por `TagKey` y que el renombre arrastra | **Después (restricción)** | En Bear, renombrar un tag le quita el pin y el ícono. Si el renombre de M2.5 no sabe de ellos, cada metadato nuevo trae ese bug. |
| 7 | **Block refs `^id`**: leerlos y resolverlos sí, pero que nota los inserte solo cuando se pide | **MVP (M3, ya previsto)** con esa regla | 03 ya los indexa. La pregunta es si nota escribe IDs en las notas. Logseq lo hace con UUIDs y es justo lo que 06 §1 quiere evitar. |
| 8 | **Historial**: guardar el origen de cada versión (app, CLI, refactor o watcher) y restaurar como cambio deshacible | **Después (restricción)** | Trilium y Joplin lo hacen. El origen se registra en el único punto de escritura (06 §3) o se pierde. |
| 9 | **Cifrado**: la ruta queda en claro, los tags y links van dentro, el historial se vuelve a cifrar y el OCR sigue la misma regla | **Decidir ahora, hacer después** | Trilium deja en claro los atributos y Anytype el índice local; Notesnook borra el historial al bloquear una nota. Con `nota.md.age`, el nombre del archivo filtra algo sí o sí. |
| 10 | **Estado de revisión y resurfacing fuera del markdown** | **Restricción** (la feature es **Baja**) | La queja número uno del plugin de revisión espaciada de Obsidian es que llena el frontmatter. |
| 11 | **Identidad del vault que sobrevive a mover la carpeta** | **Restricción** | En Obsidian el historial de File recovery se pierde si mueves el vault. 06 §8 propone usar la ruta canónica, que tiene el mismo problema; un comando para reasignar lo cubre. |
| 12 | **Captura rápida**: `nota add` más un atajo del sistema; una ventana flotante de captura, después | **MVP (CLI)** y **Después (ventana)** | Apple Notes (Quick Note), Craft (Quick Entry) e Inkdrop tienen captura global. La CLI headless ya la da sin IPC. |
| 13 | **Conflictos de sync**: `:conflicts` lista los `*.sync-conflict-*` y abre un diff de dos lados | **Después** | La pantalla de diff de Notesnook es lo que la gente pide. Inkdrop duplica notas con "[conflicted]" y es una fuente constante de quejas. |
| 14 | **Filtros por links**: huérfanas, links rotos y relacionadas, en la CLI y en `space f` | **Después (M3)** | zk los tiene (`--orphan`, `--broken-links`, `--related`) y salen gratis del índice de links. |
| 15 | **Nota al azar y "notas olvidadas"** | **Baja** | Hay plugins de Obsidian y zk tiene `--sort random`. Es casi una consulta más, si la consulta sabe ordenar. |

Lo que **no encaja**: bases de datos y vistas de tabla (§2.2), supertags y tipos de objeto (Tana, Capacities), jerarquía o tags en el nombre del archivo (Dendron, Denote), flashcards dentro de las notas, sync propio, IA integrada en la app, email-to-note y transclusión editable (synced blocks).

---

## 2. Features por área

### 2.1 Captura

| Feature | Qué es | Apps y fuente | Encaje con nota | Cimientos | Clasificación |
|---|---|---|---|---|---|
| Captura global con atajo | Una ventanita para escribir sin cambiar de app | Apple Notes, [Quick Note](https://support.apple.com/guide/imac/apdf79aa72fa/2022/mac/13); Craft, [Quick Entry](https://support.craft.do/en/introduction/quick-entry) | Alto: es el buzón (06 §9). En Linux basta un bind del gestor de ventanas a `nota add` o a una ventana `nota capture`. | No: la CLI ya escribe sin la app abierta (06 §7). | CLI en **MVP**; ventana **Después** |
| Destino y tags en la misma línea | `@libreta #tag` en el asunto del correo o en el texto capturado | Evernote, email-to-note [(b)](https://help.evernote.com/hc/articles/209005347) | Medio: choca con el buzón si pone tags por defecto. `nota add -t` ya cubre el caso explícito. | No | **No encaja** como sintaxis implícita; `-t` basta |
| Email-to-note | Una dirección de correo que crea notas | Evernote (b) | Bajo: necesita un servidor. | — | **No encaja** (06 §2: nada que necesite un servidor) |
| Web clipper a markdown | Una extensión guarda el artículo como markdown con tags | [Obsidian](https://obsidian.md/clipper), [Inkdrop](https://docs.inkdrop.app/reference/clip-web-pages-from-browser), [Joplin](https://joplinapp.org/help/apps/clipper/) (con servicio local y REST) | Medio. Lo barato es `nota add --url` (como `nb <url>`, [nb](https://xwmx.github.io/nb/)), que descarga y convierte. La extensión de navegador sería otro proyecto. | No: entra por el mismo `nota add`. | `--url` **Baja**; extensión **No encaja** |
| Captura por voz | Graba audio y lo transcribe a texto | [Reflect](https://reflect.app/) | Bajo: necesita un modelo de transcripción. | No | **No encaja** en el núcleo; un script con `nota add` lo cubre |
| URI scheme | `obsidian://new?…`, `inkdrop://actions/…` para atajos y lanzadores | [Obsidian](https://obsidian.md/help/uri), Inkdrop, Bear [x-callback-url](https://bear.app/faq/x-callback-url-scheme-documentation/) | Medio: en Linux la CLI cumple ese papel. `nota://` solo hace falta para abrir desde el navegador. | No: las acciones con nombre (CLAUDE.md) ya son la base. | **Baja** |
| Plantilla al capturar | Una tecla por plantilla, con destino y variables (`%a` = link al origen) | org-mode [capture templates](https://orgmode.org/manual/Capture-templates.html); Inkdrop [templates](https://docs.inkdrop.app/reference/note-templates) | Alto, siempre que la captura por defecto siga sin tags (06 §9). | No | **Después**, con las plantillas |
| Importar subrayados (Kindle, Readwise) | — | Inkdrop (sin verificar), Reflect | Bajo. | No | **No encaja** en el núcleo; plugin |

### 2.2 Organización y tags

Lo que hacen bien las apps de tags primero y qué aprovecha nota está en el §3. Esta tabla es el detalle.

| Feature | Qué es | Apps y fuente | Encaje con nota | Cimientos | Clasificación |
|---|---|---|---|---|---|
| **Bases de datos / vistas de tabla sobre propiedades** | Tablas, tableros y filtros sobre campos tipados | Notion; Obsidian Bases; Tana (supertags con campos); Anytype y SiYuan (tipos de objeto) | **No encaja (recomendado).** Hay demanda, pero los tags y las vistas guardadas (06 §10) ya cubren el caso real: listas filtradas. Una tabla de propiedades empuja a usar las notas como base de datos o como lista de tareas, y las tareas ya se descartaron (06 §5). | **Restricción barata**: el índice guarda el frontmatter **completo** como clave/valor genérico, no solo `tags`. Así, si algún día se quisiera, una vista así se agrega sin tocar los cimientos. | **No encaja** |
| Tags con espacios | `#vacation plans#` | [Bear](https://bear.app/faq/how-to-use-tags-in-bear/) | Bajo: es ambiguo en markdown y ninguna otra herramienta lo lee. El tag de nota (05 §1) ya admite `-` y `_`. | **Sí** (gramática) | **No encaja** |
| Renombrar = reestructurar | Renombrar `b` a `a/b` lo anida | [Bear](https://bear.app/faq/how-to-use-tags-in-bear/) | Alto: ya sale de 05 §5 (renombrar con descendientes). | Ya contemplado | — |
| Quitar un tag de todo el vault | Borra el tag sin borrar las notas | [Bear](https://bear.app/faq/how-to-use-tags-in-bear/) | Alto: es renombrar hacia "nada" con el mismo flujo (vista previa y deshacer). | Sí (motor de refactor) | **MVP** (M2.5) |
| Fijar tags arriba del panel | Pin de tags | [Bear](https://bear.app/faq/how-to-use-tags-in-bear/), UpNote | Alto. | Sí: el metadato del tag vive fuera del `.md` y el renombre lo tiene que mover. En Bear se pierde ([foro, b](https://community.bear.app/t/renaming-tag-to-existing-pinned-tag-removes-pinned-tag/18833)). | **Después** |
| Ícono o color por tag | TagCons | [Bear](https://bear.app/faq/how-to-use-tags-in-bear/) | Medio. Ya está en IDEAS. Renombrar el padre en Bear borra los íconos de los hijos ([foro, b](https://community.bear.app/t/renaming-parent-tag-removes-tag-icons/6763)). | Igual que el pin | **Baja** |
| Ordenar el árbol por número de notas | Además del orden alfabético | Bear [(b)](https://blog.bear.app/2020/05/getting-started-with-using-and-organizing-tags-in-bear/) | Alto, barato. | No | **Después** |
| Alias de tags | `#rs` → `#rust` | [Capacities](https://docs.capacities.io/reference/tags) | Medio. Ya está en IDEAS. | Igual que el pin | **Baja** |
| Vocabulario controlado suave | Avisar al crear un tag que todavía no existe | Denote, `denote-known-keywords` ([manual](https://protesilaos.com/emacs/denote)) | Alto: evita `#rsut` sin prohibir nada. El autocompletado de 05 §5 ya lo empuja. | No | **Después** (pregunta 9) |
| Página de tag como vista | Abrir un tag muestra sus notas, no un archivo | [Capacities](https://docs.capacities.io/reference/tags) | Alto: es lo que hace el panel. Una nota que describa el tag está en IDEAS. | No | Ya contemplado |
| Tags y colecciones manuales | Colecciones = grupos a mano dentro de un tipo | Capacities [(b)](https://docs.capacities.io/tutorials/tags-vs-collections) | Bajo: dos conceptos para lo mismo. | — | **No encaja** |
| Supertags (el tag es un tipo con campos y herencia) | — | [Tana](https://outliner.tana.inc/docs/supertags) | Bajo: son bases de datos. Además, los usuarios confunden tag y supertag ([ideas.tana, b](https://ideas.tana.inc/posts/649-please-expand-further-in-regards-to-inline-tagging-in-tana-docs)). | — | **No encaja** |
| Plantilla por tag | Poner `#reunión` ofrece la plantilla de reunión | Tana ([supertags](https://outliner.tana.inc/docs/supertags)); Dendron schemas [(b)](https://wiki.dendron.so/notes/c5e5adde-5459-409b-b34d-a0d75cbb1052) | Alto: une tags y plantillas sin tipos. Que la nota nueva en focus de un tag use su plantilla es la versión mínima. | No | **Después** |
| Plantilla y nombre por carpeta | Cada grupo tiene su patrón de nombre y su plantilla | zk [groups](https://zk-org.github.io/zk/config/config-group.html) | Medio: la carpeta no es el eje, pero `diario/` sí tiene otra forma. | No: va en la config del vault. | **Baja** |
| Jerarquía en el nombre del archivo (`a.b.c.md`) | — | [Dendron](https://wiki.dendron.so/notes/hierarchies) | Bajo: los tags jerárquicos ya cubren el caso sin atar la jerarquía a la ruta. | — | **No encaja** |
| Tags en el nombre del archivo (`ID--titulo__tag1_tag2.md`) | — | [Denote](https://protesilaos.com/emacs/denote) | Bajo: cambiar un tag cambiaría la ruta, que es la identidad (06 §1). | — | **No encaja** |
| Silos | Varios directorios independientes | [Denote](https://protesilaos.com/emacs/denote) | Ya decidido: un vault a la vez (06 §8). | — | Ya contemplado |
| Fijar notas | La nota fijada va primero en las listas | [nb](https://xwmx.github.io/nb/), Inkdrop (sin verificar), Apple Notes | Medio: los bookmarks (§2.3) cubren casi lo mismo. | Sí, si se guarda: va en la config del vault y sigue los renombres. | **Baja** |
| Un estado de nota (`status:`) | — | Inkdrop | — | — | **Descartado** (no se vuelve a proponer) |

### 2.3 Navegación y búsqueda

| Feature | Qué es | Apps y fuente | Encaje con nota | Cimientos | Clasificación |
|---|---|---|---|---|---|
| "Solo este tag" y "subtag a cualquier profundidad" | `!#tag` excluye los hijos y `#*/tag` busca `tag` en cualquier nivel | [Bear](https://bear.app/faq/how-to-search-notes-in-bear/) | Alto: Bear tiene casi la gramática de 05 §4. El primero ya está en IDEAS. | Sí (gramática de consulta) | **Después** |
| Filtros con `@` o `clave:` | `@untagged`, `@lastXdays`, `@attachments`, `@backlinks`, `@locked`, `@empty`; `tag:* intitle: created:` | [Bear](https://bear.app/faq/how-to-search-notes-in-bear/); Evernote [(b)](https://help.evernote.com/hc/articles/208313828); [Inkdrop](https://docs.inkdrop.app/reference/search-and-filter-notes) (`title: body: -tag:`); [zk](https://zk-org.github.io/zk/notes/note-filtering.html) (`--created-after`, `--tagless`) | Alto. `@untagged` es el buzón y cinco apps convergen en lo mismo. | **Sí**: reservar ya la forma `clave:valor` en `TagQuery` para que las vistas guardadas (06 §10) y el `--json` no cambien después. | "Sin tags" **MVP**; resto **Después** |
| Fechas: modificada / creada | Filtrar y ordenar por fecha | Las mismas | Alto, pero ¿de dónde sale la fecha? El `mtime` lo cambian Syncthing y git; la fecha de creación del sistema de archivos no siempre existe. | **Sí**: si nota escribe `created:` en cada nota, cambia el formato. Lo recomendado es no escribirlo, leerlo si está (gracias al frontmatter genérico) y usar el `mtime` para "modificada". | **MVP** (pregunta 3) |
| Ordenar al azar o por cantidad de palabras | `--sort random`, `word-count` | [zk](https://zk-org.github.io/zk/notes/note-filtering.html) | Alto, barato. | No | **Después** |
| Vistas guardadas | Consultas con nombre | Apple Notes Smart Folders; Standard Notes [Smart Views](https://standardnotes.com/features); Evernote saved searches (b); Obsidian bookmarks de búsquedas ([help](https://obsidian.md/help/plugins/bookmarks)). Bear **no** las tiene ([foro, b](https://community.bear.app/t/feature-requests-search-keywords-autocompletion-saved-searching-conditions/7736)). | Ya decidido (06 §10). | — | Ya contemplado |
| Consulta embebida en una nota | Un bloque que pinta una lista viva de resultados | Denote query links ([manual](https://protesilaos.com/emacs/denote)); Tana [live search](https://outliner.tana.inc/docs/search-nodes); [SilverBullet](https://docs.silverbullet.md/Space%20Lua/Integrated%20Query); vista de tag embebida de [Capacities](https://docs.capacities.io/reference/tags) | Medio-alto: una nota "índice de proyecto" que siempre está al día. Un bloque cercado (```` ```nota-query ````) sigue siendo markdown legible fuera de nota. | Toca el formato solo si se elige la sintaxis; un bloque de código no rompe nada. | **Después** (pregunta 10) |
| Lenguaje de consulta completo (SQL, Datalog, Lua) | — | SilverBullet, Logseq (sin verificar), SiYuan | Bajo: convierte las consultas en programación. Si hace falta, es trabajo de los plugins WASM. | — | **No encaja** |
| Bookmarks | Guardar notas, headings y búsquedas | [Obsidian](https://obsidian.md/help/plugins/bookmarks) | Medio: las vistas guardadas cubren las búsquedas; los marks de vim `A`–`Z` (M6) cubren las notas. | Sí, si se guardan: config del vault y siguen los renombres. | **Baja** |
| Workspaces | Guardar layouts de paneles | [Obsidian](https://obsidian.md/help/plugins/workspaces); Inkdrop "workspace view" ([guía](https://docs.inkdrop.app/start-guide/how-to-organize-notes)) es en realidad el focus | El focus ya es la idea útil (05 §3). Guardar layouts pesa poco. | No: va en `state` por vault (06 §8). | **Baja** |
| Vista previa al pasar el puntero | — | [Obsidian](https://obsidian.md/help/plugins/page-preview) | Medio: en vim es `K` sobre un link. | No | **Después** (M3) |
| Outline de headings | Árbol de headings de la nota | Obsidian (sin verificar); Zettlr | Alto y barato: es un picker más. | No | **Después** |
| Búsqueda full-text con coincidencias parciales | Que `trin` encuentre `string` | Inkdrop **no** puede, por usar SQLite FTS ([doc](https://docs.inkdrop.app/reference/search-and-filter-notes)) | Alto: es un aviso para M6, que tiene que elegir el tokenizer (trigramas o grep). | No: es caché. | **MVP** (M6, ya previsto) |
| Búsqueda en adjuntos y OCR | El texto de imágenes y PDF entra en la búsqueda | [Joplin](https://joplinapp.org/help/apps/ocr/) (Tesseract local); [Apple Notes](https://support.apple.com/guide/notes/not18ab658ed/mac); Bear `@ocr` | Medio: es útil para capturas de pantalla. | **Sí**, por una regla: el texto del OCR es contenido derivado y cumple 06 §4 (nada en claro de una nota cifrada). Va en la caché, nunca en el vault. | **Baja** (pregunta 12) |
| LSP de notas | Exponer el índice a Neovim y Helix: completado de `[[` y `#`, rename, diagnósticos | [Markdown Oxide](https://github.com/Feel-ix-343/markdown-oxide), [zk](https://zk-org.github.io/zk/tips/editors-integration.html) | Medio-alto: es la manera de que quien vive en nvim use el índice de nota sin abrir la ventana. | No: el núcleo ya es headless (06 §7). | **Baja** (pregunta 13) |

### 2.4 Edición y markdown

| Feature | Qué es | Apps y fuente | Encaje con nota | Cimientos | Clasificación |
|---|---|---|---|---|---|
| Plegar por heading | Colapsar la sección | Apple Notes [(b)](https://iphonelife.com/content/how-to-use-collapsible-sections-notes-app) | Alto: `za`/`zc`/`zo` de vim sobre el árbol de tree-sitter de M4. | No | **Después** |
| Slash commands | `/` inserta bloques o ejecuta acciones | [Notion](https://www.notion.com/help/guides/using-slash-commands) | Bajo: la paleta y `:` cumplen ese papel en un editor modal. | — | **No encaja** |
| Plantillas con variables | Fecha, título, tags automáticos; instrucciones que no se copian | [Inkdrop](https://docs.inkdrop.app/reference/note-templates) (Liquid); [SilverBullet](https://docs.silverbullet.md/Template) | Ya decidido (06 §11). El aporte es la lista mínima de variables: fecha, título y los tags del focus. | No | Ya contemplado |
| Citas (Zotero/CSL) | Autocompletado de citas y bibliografía | [Zettlr](https://docs.zettlr.com/en/editor/citations.html) | Bajo para el uso previsto. | No | **No encaja** en el núcleo; plugin |
| Legibilidad y estadísticas de escritura | Colorea frases difíciles; palabras por día | [Zettlr](https://docs.zettlr.com/en/language-style/readability.html) | Bajo. | No | **No encaja** en el núcleo; plugin |
| Pegar una URL como link con título | — | (sin verificar) | Medio. | No | **Baja** |

### 2.5 Links y conocimiento

| Feature | Qué es | Apps y fuente | Encaje con nota | Cimientos | Clasificación |
|---|---|---|---|---|---|
| Links relativos salientes al mover | Mover una nota reescribe también **sus** links relativos, no solo los que llegan | Obsidian lo hace mal desde hace años ([foro, b](https://forum.obsidian.md/t/broken-links-in-relative-path-mode-on-move-rename/4386)) | Alto: nota resuelve links markdown relativos (`resolve_from`, ya en `nota-syntax`). | **Sí** (motor de refactor) | **MVP** (M3) |
| Renombrar en lote con vista previa | Regex sobre rutas, vista previa, aplicar y actualizar links | Dendron [refactor hierarchy](https://wiki.dendron.so/notes/srajljj10V2dl19nCSFiC/) | Alto: es el mismo flujo que renombrar tags (05 §5), aplicado a carpetas. | Sí (mismo motor) | **Después** |
| Un solo punto que renombra | Obsidian solo actualiza links si se renombra por `fileManager`; `vault.rename()` no lo hace ([foro](https://forum.obsidian.md/t/vault-rename-file-path-doesnt-trigger-link-update/32317)) | — | Lección: el renombre del árbol, el de `:` y el de la CLI (`nota mv`) pasan por la misma función, igual que la escritura (06 §3). | **Sí** | **MVP** |
| Reparar links tras un renombre hecho fuera | Detectar el renombre por hash y ofrecer arreglar los links | zk no tiene renombrar y el issue se cerró ([#200](https://github.com/zk-org/zk/issues/200)) | Alto: 06 §1 ya detecta el renombre; falta *ofrecer* reescribir los links. | Sí (motor) | **Después** |
| Aliases de nota | `aliases:` en el frontmatter, en el completado y en la resolución | [Obsidian](https://obsidian.md/help/aliases), org-roam `ROAM_ALIASES` ([manual](https://www.orgroam.com/manual.html)) | Ya previsto en 03 §2 (M3). | — | Ya contemplado |
| Menciones sin link | Texto que coincide con el título o un alias de la nota, con opción de convertirlo en link | [Obsidian](https://obsidian.md/help/plugins/backlinks), Bear (b), org-roam (lento, apagado por defecto) | Medio: depende de la búsqueda full-text (M6); org-roam muestra lo que cuesta. | No | **Después** |
| Block refs `^id` | `[[nota#^id]]` | [Obsidian](https://obsidian.md/help/links), Foam; Logseq usa `((uuid))` (sin verificar) | Ya previsto en 03 §2. La recomendación nueva es que nota **no inserte IDs sola**: solo cuando el usuario pide "link a este bloque". | **Sí** (formato) | **MVP** con esa regla (pregunta 5) |
| Transclusión editable | Editar el bloque incrustado cambia el original | Notion [synced blocks](https://www.notion.com/help/synced-blocks); [org-transclusion](https://github.com/nobiot/org-transclusion) | Bajo: M5 ya tiene transclusión de solo lectura. Editar a través de otro archivo complica el undo, el historial y los conflictos. | — | **No encaja** (solo lectura basta) |
| Filtros por grafo | Huérfanas, links rotos, backlinks que faltan, relacionadas por vecinos | [zk](https://zk-org.github.io/zk/notes/note-filtering.html) | Alto: salen del índice de links. | No | **Después** (M3, pregunta 11) |
| Grafo 2D/3D | — | [org-roam-ui](https://github.com/org-roam/org-roam-ui), Obsidian | Ya decidido: baja (06 §11). | — | Ya contemplado |
| Canvas | Lienzo infinito; el formato abierto es [JSON Canvas](https://jsoncanvas.org/) | Obsidian, Heptabase (b) | Bajo: es otra app dentro de la app. | No | **No encaja** por ahora |
| IDs Zettelkasten | Un timestamp como destino del link | [Zettlr](https://docs.zettlr.com/en/pkms/zkn-method.html) | Bajo: choca con "la ruta es la identidad". | — | **No encaja** |
| Fechas en lenguaje natural | `[[next monday]]` | [Markdown Oxide](https://oxide.md/index) | Bajo: daily notes está apagado por defecto. | — | **No encaja** |

### 2.6 Adjuntos y media

| Feature | Qué es | Apps y fuente | Encaje con nota | Cimientos | Clasificación |
|---|---|---|---|---|---|
| Escanear documentos | La cámara escanea y adjunta | [Apple Notes](https://support.apple.com/guide/notes/not18ab658ed/mac) | Bajo: es cosa del celular, que está fuera de alcance (06 §2). | — | **No encaja** |
| OCR | Ver §2.3 | — | — | Regla de cifrado | **Baja** |
| Redimensionar imágenes | — | Inkdrop (sin verificar) | Medio. Obsidian usa `![[img.png\|300]]`; si se adopta, es sintaxis. | Sí, si se adopta la sintaxis | **Después** |
| Limpiar adjuntos huérfanos | Listar los adjuntos que ninguna nota usa | (sin verificar en estas apps; plugin común en Obsidian) | Alto: 06 §6 dice que nada borra adjuntos solo, así que se acumulan. Un comando que los liste encaja. | No | **Después** |
| Anotar PDF | — | Heptabase (b) | Bajo. | — | **No encaja** |

### 2.7 Privacidad y seguridad

| Feature | Qué es | Apps y fuente | Encaje con nota | Cimientos | Clasificación |
|---|---|---|---|---|---|
| Qué queda en claro en una nota cifrada | Trilium cifra el título y deja los atributos ([doc](https://docs.triliumnotes.org/user-guide/concepts/notes/protected-notes)); Joplin deja `updated_time` y `type` ([foro](https://discourse.joplinapp.org/t/resolved-joplin-leaks-private-info-despite-end-to-end-encryption-enabled/25373)); Standard Notes cifra también los tags ([doc](https://standardnotes.com/help/security/encryption)) | — | Con `nota.md.age`, la ruta queda visible sí o sí. Lo honesto es documentarlo y meter tags y links dentro del cifrado (06 §4 ya proponía indexar solo la ruta). | **Sí** | **Decidir ahora** (pregunta 6) |
| Índice cifrado o nada | Anytype deja el índice de búsqueda local en claro ([doc](https://doc.anytype.io/anytype-docs/advanced/data-and-security/how-we-keep-your-data-safe)) | — | 06 §4 ya lo prohíbe. Es un diferenciador real. | Ya contemplado | — |
| Historial al cifrar una nota | Notesnook borra todo el historial al meter una nota en su vault ([doc](https://notesnook.com/help/lock-notes-with-private-vault)) | — | nota tendría que volver a cifrar las versiones viejas, o avisar y borrarlas. Dejarlas en claro rompe 06 §4. | **Sí** (historial) | **Decidir ahora** (pregunta 7) |
| Sesión de desbloqueo con tiempo límite | Las notas se vuelven a bloquear tras N minutos | Trilium (10 min); Notesnook [app lock](https://notesnook.com/help/app-lock); Standard Notes (el passcode envuelve la clave) | Alto: la versión de nota es un agente tipo `ssh-agent` que guarda la identidad de `age` en memoria y sirve también a la CLI. | No | **Después**, con el cifrado |
| Bloquear notas con adjuntos | Apple Notes no deja bloquear notas con PDF o audio ([doc](https://support.apple.com/102537)) | — | Lección: definir desde el principio qué pasa con un adjunto de una nota cifrada (06 §6 dice que no le pertenece). | Sí, cuando se diseñe el cifrado | **Después** |
| Sin recuperación de contraseña | Inkdrop ([doc](https://docs.inkdrop.app/security)), Bear y Anytype lo avisan | — | Con `age` es igual. Hay que decirlo claro en la UI. | No | **Después** |
| Bloquear la app entera | PIN al abrir | Notesnook, UpNote ([doc](https://help.getupnote.com/lock/lock-password)) | Bajo en escritorio: lo cubre la sesión del sistema. | — | **No encaja** |

### 2.8 Sync y dispositivos

| Feature | Qué es | Apps y fuente | Encaje con nota | Cimientos | Clasificación |
|---|---|---|---|---|---|
| Sync propio | Servidor de la app, con E2EE | Obsidian Sync, Joplin, Standard Notes, Notesnook, Anytype (CRDT, [any-sync](https://github.com/anyproto/any-sync)), SiYuan | Bajo: 06 §2 deja el sync a Syncthing y git. | — | **No encaja** |
| Pantalla de conflictos con diff | Actual frente a entrante, con elegir uno o guardar copia | [Notesnook](https://notesnook.com/help/faqs/what-are-merge-conflicts); Joplin los manda a un cuaderno "Conflicts" ([doc](https://joplinapp.org/help/apps/conflict/)) | Alto: nota no los fusiona sola (06 §2), pero `:conflicts` puede listar los `*.sync-conflict-*` de [Syncthing](https://docs.syncthing.net/users/syncing.html) y abrir el diff de 03 §5. Ojo: con cifrado, Syncthing genera `x.sync-conflict-….md.age`. | No | **Después** (pregunta 14) |
| Sync automático con git | Commit en segundo plano y `nb git sync` | [nb](https://xwmx.github.io/nb/), SilverBullet | Medio: el historial por nota (06 §3) cubre deshacer sin git. Sincronizar con git es trabajo de otra herramienta. | — | **No encaja** en el núcleo |
| Historial que no se pierde al mover el vault | El File recovery de Obsidian se pierde si el vault se mueve sin su gestor ([help](https://obsidian.md/help/plugins/file-recovery)) | — | 06 §8 propone identificar el vault por su ruta canónica, que tiene ese mismo problema. Un comando para reasignar el historial lo cubre; detectar el vault movido al abrirlo sería mejor. | **Sí** (identidad del vault) | **Restricción** |

### 2.9 Automatización y extensibilidad

| Feature | Qué es | Apps y fuente | Encaje con nota | Cimientos | Clasificación |
|---|---|---|---|---|---|
| CLI headless | Obsidian tiene CLI oficial, pero **necesita la app abierta** y la lanza si no lo está ([help](https://obsidian.md/help/cli)) | — | La de nota no (06 §7). Es un diferenciador. | Ya contemplado | — |
| Alias de comandos | `edlast = "zk edit --limit 1 --sort modified-"` | zk [alias](https://zk-org.github.io/zk/config/config-alias.html) | Alto: en nota, un alias es una acción con nombre que llama a otras. Encaja con la paleta. | No | **Después** |
| Servidor MCP | Exponer las notas a agentes | Inkdrop (sin verificar) | Medio: `--json` cubre casi todo. | No | **Baja** |
| API REST local | La app abre un puerto | [Joplin](https://joplinapp.org/help/apps/clipper/) | Bajo: es un proceso de fondo, cosa que 06 §2 evita. La CLI cubre lo mismo. | — | **No encaja** |
| Scripting dentro de las notas | Lua en el vault | [SilverBullet](https://silverbullet.md/features), Trilium | Bajo: es el papel de los plugins WASM (IDEAS). | — | **No encaja** |
| IA integrada | Chat, resumen, notas relacionadas | Notion, Reflect | Bajo en el núcleo. Un agente externo con `--json` hace lo mismo sin atar nota a un proveedor. | — | **No encaja** |

### 2.10 Lectura y revisión

| Feature | Qué es | Apps y fuente | Encaje con nota | Cimientos | Clasificación |
|---|---|---|---|---|---|
| Nota al azar | — | Obsidian (plugin core, (b)), zk `--sort random` | Alto, barato. | No | **Baja** |
| "En este día" y notas olvidadas | Notas del mismo día de otros años; sin tocar hace N meses | Plugins de Obsidian (Daily Echoes, Journal Review, (b)); con zk sale combinando `--modified-before`, `--orphan` y `--sort random` | Medio-alto: es una consulta, si la consulta sabe de fechas (§2.3). | No | **Baja** (pregunta 15) |
| Revisión espaciada de notas enteras | Una cola de notas con `#review` y SM-2/FSRS | [Obsidian Spaced Repetition](https://github.com/st3v3nmw/obsidian-spaced-repetition) | Medio. **Lección**: guarda el estado en el frontmatter (`sr-due`, `sr-ease`) y la queja más repetida es que eso ensucia las notas ([issues](https://github.com/st3v3nmw/obsidian-spaced-repetition/issues?q=frontmatter)). | **Sí**: si se hace, el estado va en `data` (por vault, sigue los renombres), nunca en el archivo. | **Baja** |
| Flashcards | `Q::A`, cloze, mazos por tag | Obsidian SR; Logseq `#card` (b); [org-srs](https://github.com/bohonghuang/org-srs); [org-drill](https://elpa.nongnu.org/nongnu/org-drill.html) | Bajo: es otra app (Anki) y mete sintaxis propia en las notas. | — | **No encaja** |
| Revisión diaria de N fragmentos | Muestra unos pocos, con pesos | Readwise (b) | Medio: es la versión sin tarjetas de lo anterior. | Igual que la revisión espaciada | **Baja** |

---

## 3. Qué hacen bien las apps de tags primero y qué aprovecha nota

**Bear** es el modelo más cercano: tags en el texto, jerarquía con `/`, árbol en el panel y renombrar para reestructurar.
- Su gramática de búsqueda es casi la de 05 §4: `#a -#b`, más `!#a` (sin hijos) y `#*/a`. Sus filtros con `@` (`@untagged`, `@lastXdays`, `@attachments`) confirman que "sin tags" y las fechas son los primeros filtros que se piden.
- Aprovecha: quitar un tag de todo el vault, fijar tags y ordenar el árbol por número de notas.
- Lecciones de sus quejas:
  - la gramática de tags no se cambia después de publicarla ([foro, b](https://community.bear.app/t/upgrade-to-bear-2-0-square-brackets-dont-work-in-tags/11272));
  - no se reconocen tags dentro de comillas ni de código ([foro, b](https://community.bear.app/t/tag-recognized-but-it-wasnt-one/3714)); 05 §6 ya excluye el código;
  - el autocompletado no mete la ruta más larga al escribir `/`;
  - renombrar conserva los metadatos del tag.
- Lo que nota ya hace mejor: vistas guardadas, que Bear no tiene, y focus.

**Denote** demuestra que se puede tener todo sin base de datos: los archivos son la verdad y el índice se reconstruye. nota ya piensa así.
- Aprovecha: el vocabulario controlado opcional, los query links (una consulta como destino de link, §2.3) y que renombrar sincroniza el frontmatter.
- Lo que choca: poner los tags en el nombre del archivo, porque en nota cambiaría la identidad.

**Tana** enseña que un tag puede *hacer* algo al aplicarse.
- Aprovecha: la versión mínima, **plantilla por tag**, que junta dos cosas ya decididas (tags y plantillas) sin crear tipos. También los resultados vivos que se pueden poner en cualquier sitio y algún operador de fecha.
- Lo que choca: los campos, la herencia y las vistas de tabla, que son bases de datos.
- Lección: los usuarios se confunden entre tags en el texto y supertags. **Un solo concepto de tag**, que es lo que 05 ya decidió.

**Capacities** separa la página de tag (una vista) de las notas.
- Aprovecha: fusionar como acción de primera clase (05 ya lo tiene), los alias de tags y embeber la vista de un tag con su filtro y su orden.
- Lo que choca: los objetos y tipos, las colecciones manuales, que no tenga jerarquía de tags y que el markdown sea solo exportación.

**En resumen**, lo que nota se lleva de las cuatro:
- los filtros de Bear;
- la plantilla por tag de Tana;
- las consultas embebidas de Denote y Tana;
- los alias y el fusionar de Capacities.

Todo eso cabe en markdown plano con la ruta como identidad. Ninguna de las cuatro junta tags jerárquicos, vistas guardadas, focus y archivos planos; es el hueco de nota.

---

## 4. Anti-features: lo que nota evita a propósito

| Anti-feature | Dónde pasa | Qué hace nota |
|---|---|---|
| **Lentitud con vaults grandes** | Obsidian: unos 4 s por tecla en el picker `[[` con 40 k archivos ([foro](https://forum.obsidian.md/t/slow-performance-with-large-vaults/16633)) y búsqueda lenta en 30 k notas ([foro, b](https://forum.obsidian.md/t/slow-search-yet-fast-quick-switcher-for-large-vault-why/62969)). Notion: la lentitud domina las reseñas negativas (sin verificar, fuente secundaria). | El rendimiento es la prioridad 1. Los objetivos de 03 §8 se miden con el vault sintético de 5 k notas; conviene agregar uno de 40 k. |
| **Formato opaco y lock-in** | SiYuan: `.sy` en JSON, y su README avisa que sincronizar con Dropbox o Drive puede corromper datos ([repo](https://github.com/siyuan-note/siyuan)). Notion: el export rompe callouts e imágenes y llena los nombres de UUIDs (sin verificar). Anytype: el export perdía las relaciones (b). Inkdrop: no son archivos, por diseño ([foro](https://forum.inkdrop.app/t/folder-with-markdown-files/1313)). | Markdown plano; el índice es caché; nada que dé sentido al vault vive fuera de él (06 §2). |
| **Abandonar el markdown a medio camino** | Logseq con la versión DB: los usuarios temen perder grep, regex y git ([foro](https://discuss.logseq.com/t/database-version-too-drastic-choice/20346)). | El archivo es la verdad, también en el futuro. Lo recomendado es que las bases de datos no entren (§2.2). |
| **Sync propietario y conflictos duplicados** | Inkdrop duplica notas con "[conflicted]" ([foro, b](https://forum.inkdrop.app/t/conflicted-all-the-time/1073)). Standard Notes perdía notas por una carrera en el sync ([post](https://o.ctrl.blog/entry/standard-notes-dataloss.html)). Evernote limitó los dispositivos del plan gratis (b). | Sin sync propio. Tolera cambios externos y nunca sobrescribe un buffer sin guardar (06 §2). |
| **Plugins que rompen la app** | Obsidian tuvo que desactivar plugins de la comunidad que rompían el core (b). El plugin de mappings de vim busca mantenedor (01 §5). | Plugins WASM en sandbox y API versionada (IDEAS). Vim y lo esencial van en el núcleo, nunca en un plugin. |
| **Metadatos de plugins en el frontmatter** | Obsidian Spaced Repetition ([issues](https://github.com/st3v3nmw/obsidian-spaced-repetition/issues?q=frontmatter)) | El estado de nota vive en `data` o `state`; nunca reescribe el YAML del usuario salvo el campo editado (CLAUDE.md). |
| **IDs en los archivos** | Logseq (UUID por bloque), org-roam (`:ID:`, que se adoptó para no romper links al renombrar, [hilo](https://org-roam.discourse.group/t/org-roam-major-redesign/1198)) | La ruta es la identidad (06 §1). El costo de esa decisión es el motor de refactor (§1, punto 5), y hay que pagarlo bien. |
| **Cambiar la gramática después de publicarla** | Bear 2 ([foro, b](https://community.bear.app/t/upgrade-to-bear-2-0-square-brackets-dont-work-in-tags/11272)) | Gramática de tags cerrada en M2.5 y con tabla de casos en `insta`. |
| **Renombrar pierde metadatos** | Bear: pin e íconos ([foro, b](https://community.bear.app/t/renaming-tag-to-existing-pinned-tag-removes-pinned-tag/18833)) | Todo lo que se guarde por tag o por ruta lo mueve el mismo motor de renombre. |
| **CLI que necesita la app abierta** | Obsidian ([help](https://obsidian.md/help/cli)) | CLI headless desde el núcleo (06 §7). |
| **Índices en claro** | Anytype ([doc](https://doc.anytype.io/anytype-docs/advanced/data-and-security/how-we-keep-your-data-safe)) | Nada derivado en claro de una nota cifrada (06 §4). |
| **Historial que se pierde** | Notesnook al bloquear una nota; Obsidian al mover el vault | Ver §2.7 y §2.8. |
| **Offline como opción de pago** | Notion: hay que marcar cada página como disponible offline y la descarga automática es de pago (b) | Local primero, sin cuenta. |
| **Vim solo en el editor** | Obsidian, SilverBullet (los atajos de la app tienen prioridad, [doc](https://docs.silverbullet.md/Vim)), Inkdrop | Vim en toda la UI (prioridad 3 del README). |
| **Demasiados conceptos para lo mismo** | Tana (tags y supertags), Capacities (tags, colecciones y queries), Inkdrop (libretas, tags y estados) | Un solo tag. Las carpetas existen, pero no son el eje. |
| **Depender de una empresa o una persona** | Dendron se abandonó en 2023: no encontró product-market fit y el fundador se quemó ([discusión](https://github.com/dendronhq/dendron/discussions/3890)) | Archivos planos legibles con cualquier herramienta. Si nota muere, las notas siguen. |

---

## 5. Preguntas para el autor

Cada una se contesta con sí/no o con una opción. La recomendación va primero. Las bases de datos no están aquí porque lo recomendado es que no encajen (§2.2).

1. **Frontmatter completo en el índice** (clave/valor genérico, solo lectura): ¿se agrega la restricción a 03 §4 ya? **Sí (recomendado)** / No.
2. **Tags con espacios**: **A) No, en ningún lado (recomendado)** / B) Solo en `tags:` del frontmatter / C) Sintaxis de Bear `#a b#`.
3. **Fechas en consultas**: **A) "Modificada" desde el `mtime`; "creada" solo si la nota trae `created:` y nota nunca lo escribe (recomendado)** / B) nota escribe `created:` al crear cada nota / C) Sin fechas en las consultas.
4. **Gramática de consulta**: ¿se reserva ya la forma `clave:valor` (`modified:7d`, `has:adjunto`) aunque solo se implemente "sin tags"? **Sí (recomendado)** / No.
5. **Block refs `^id`**: **A) Se leen y resuelven; nota solo escribe un `^id` cuando el usuario pide "link a este bloque" (recomendado)** / B) Ni se leen ni se escriben; solo links a headings / C) Como Obsidian.
6. **Nota cifrada**: ¿se acepta que su ruta (carpeta y nombre) quede en claro, con tags y links dentro del cifrado? **Sí (recomendado)** / No (exige nombres opacos y choca con "la ruta es la identidad").
7. **Historial de una nota que se cifra**: **A) Se vuelven a cifrar sus versiones (recomendado)** / B) Se borran, avisando.
8. **Quitar un tag de todo el vault** en M2.5, con el mismo flujo que renombrar: **Sí (recomendado)** / No.
9. **Aviso al crear un tag que no existía** (vocabulario controlado suave): Sí, apagado por defecto / **Sí, encendido (recomendado)** / No.
10. **Consultas embebidas** en una nota (un bloque ```` ```nota-query ```` que pinta la lista): **Sí, después (recomendado)** / No.
11. **Filtros por links** (huérfanas, links rotos) en `space f` y en la CLI, con M3: **Sí (recomendado)** / No.
12. **OCR de imágenes pegadas** para la búsqueda: Sí, después / **Baja, sin fecha (recomendado)** / No.
13. **`nota lsp`** para usar el índice desde Neovim o Helix: Sí, después / **Baja, sin fecha (recomendado)** / No.
14. **`:conflicts`** que lista los `*.sync-conflict-*` y abre un diff: **Sí, después (recomendado)** / No.
15. **Resurfacing**: **A) Nota al azar y "notas olvidadas" como consultas (recomendado)** / B) A, más revisión espaciada con estado en `data` / C) Nada.
16. **Captura**: **A) Solo `nota add` más un atajo del sistema en el MVP; la ventana flotante, después (recomendado)** / B) Ventana flotante en el MVP.
17. **Plantilla por tag** (una nota nueva en focus de `#reunión` usa la plantilla de reunión): **Sí, con las plantillas (recomendado)** / No.
18. **Vault movido**: ¿al abrir un vault que parece movido se ofrece reasignarle el historial? **Sí (recomendado)** / No, basta con un comando.
