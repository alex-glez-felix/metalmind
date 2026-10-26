# 06 — Decisiones de fondo

> Decisiones de producto del 2026-10-18 que afectan la arquitectura. Son **decisiones**, no propuestas; lo que no venía de ellas va marcado **(propuesta)** y queda pendiente de confirmar.
> Muchas de estas funciones llegan tarde o nunca. Lo que importa de cada una son las **restricciones que impone desde ya**: lo que el código de hoy no puede hacer sin cerrarle la puerta. Construye sobre [03](03-core-architecture.md) (crates en §6, roadmap en §8) y [05](05-tags.md).

## TL;DR

| # | Decisión | Restricción principal desde ya |
|---|---|---|
| 1 | La identidad de una nota es su **ruta**; sin ID en el frontmatter | Índice e historial siguen los renombres, también los que llegan por el watcher como borrar + crear |
| 2 | Celular: quizá algún día una app de captura; fuera de alcance | Núcleo sin GPUI ni supuestos de escritorio; archivos planos; cambios externos en cualquier momento |
| 3 | Historial de versiones por nota, escondido (`:history`) | Vive en el directorio de **datos**, por vault; no depende de git |
| 4 | Cifrado de notas: deseable, sin fecha; la ruta queda en claro y las notas cifradas van ocultas en la UI | Nada derivado guarda en claro el contenido de una nota cifrada, tampoco sus tags, sus links ni su historial |
| 5 | Sin sistema de tareas | No indexar tareas "por si acaso" |
| 6 | Una carpeta de adjuntos por vault, `ctrl-v` pega imágenes | Renombrar o borrar una nota no toca sus adjuntos |
| 7 | CLI de primera clase (`metalmind add`, `search`, `tags`, `--json`) | Todo lo del vault funciona sin ventana, desde el núcleo |
| 8 | Un vault abierto a la vez | Caché, historial y estado de UI van por vault; un vault movido puede recuperar su historial |
| 9 | Buzón = las notas sin tags | La captura deja la nota sin tags; `TagQuery` sabe decir "sin tags" |
| 10 | Vistas guardadas: consultas de tags con nombre, en el panel | Se guardan con el vault, no en la config global |
| 11 | Daily notes, plantillas, grafo, temas | Ninguna |
| 12 | Publicar a la web: fuera de alcance | — |
| 13 | Metalmind es una GUI con navegación estilo vim, no una TUI | Toda función se usa desde la UI además del teclado; nadie edita config ni metadata a mano por necesidad |
| 14 | Nunca habrá un panel de vista previa aparte | El markdown se ve con estilo en el mismo editor (live preview, 03 §2); nada del estilo depende de una segunda vista |
| 15 | Fórmulas `$…$` y `$$…$$` con RaTeX | Solo `metalmind-ui` depende de RaTeX; dentro de una fórmula no hay tags ni links, también para el índice |

---

## 1. La identidad de una nota es su ruta

**Decisión.** Una nota es su ruta relativa al vault. No hay ID en el frontmatter ni en ningún otro lado. Renombrar o mover una nota desde Metalmind actualiza los `[[links]]` y los tags que apuntan a ella (el motor de refactorización de 03 §4 y 05 §5).

**Por qué.** Un ID en cada archivo ensucia notas que deben leerse con cualquier herramienta, y la ruta ya es única. Es reversible: si algún día hace falta un ID, un comando lo escribe a todas las notas de una vez.

**Restricciones desde ya:**

- `NoteId` (03 §4) sigue siendo un entero interno de la sesión, nunca algo que se guarde en un archivo del usuario ni que aparezca en un link.
- Todo lo que se guarda fuera de la memoria (caché, historial §3, estado de UI §8) se indexa por **ruta relativa al vault**, y tiene que poder **seguir un renombre**: mover la entrada, no perderla ni duplicarla.
- El watcher entrega `Renamed` cuando inotify da el par rename-from/rename-to, pero `git checkout`, Syncthing y muchos editores renombran como **borrar + crear**. `metalmind-vault` trata un `Removed` y un `Created` con el mismo contenido (hash igual, dentro de la misma ventana del debouncer) como un `Renamed`. El `TagIndex` (05 §2), el índice de links y el historial reciben un solo evento de rename, no un borrado y una nota nueva.
- Lo mismo al abrir el vault después de cambios hechos con Metalmind cerrado: si falta una ruta que estaba en la caché y aparece una nueva con el mismo hash, es un rename **(propuesta)**. Así el historial de una nota renombrada desde fuera no queda huérfano.
- Ningún formato propio de Metalmind (caché, historial, vistas guardadas) puede suponer que la ruta es inmutable.

## 2. Celular

**Decisión.** Fuera de alcance hoy. Si algún día hay algo, será una app simple de captura rápida; la app principal es la de escritorio.

**Por qué.** Lo que el autor necesita del celular es capturar, no editar ni navegar el vault; eso lo cubre la CLI (§7) más cualquier herramienta de sincronización.

**Restricciones desde ya:**

- El núcleo (03 §6) sigue sin GPUI **ni supuestos de escritorio**: nada de rutas fijas de Linux fuera de `metalmind-config`, nada que necesite un proceso de fondo o un servidor para funcionar.
- Las notas siguen siendo archivos markdown planos en carpetas, sincronizables con herramientas externas (Syncthing, git). Nada que dé sentido al vault vive fuera de él salvo lo regenerable (caché) y lo que es de esta máquina (historial §3, estado de UI §8).
- Metalmind tolera **cambios externos en cualquier momento**: archivos que aparecen, cambian o desaparecen con la app abierta o cerrada. Sin locks de archivo. El conflicto con un buffer sin guardar se resuelve como dice 03 §5 (mostrar diff y elegir), nunca sobrescribiendo.
- Los archivos de conflicto de las herramientas de sync (`*.sync-conflict-*.md`, por ejemplo) son notas normales: Metalmind no los esconde ni los fusiona por su cuenta **(propuesta)**.

## 3. Historial de versiones por nota

**Decisión.** Metalmind guarda versiones de cada nota y permite verlas, compararlas y restaurarlas. Está **escondido**: no hay panel ni indicador a la vista; se llega con algo como `:history`, que muestra las versiones de la nota actual con su diff y deja restaurar una.

Se guarda en el directorio de **datos** (`$XDG_DATA_HOME/metalmind/`, `Paths::data` en `metalmind-config`), **por vault**. No va en el vault (no es contenido del usuario ni debe sincronizarse) ni en la caché (si se borra la caché, el historial se pierde de verdad, y la caché tiene que poder borrarse sin miedo). Funciona aunque el vault no sea un repo de git.

**Por qué.** Deshacer un cambio de hace tres días no puede depender de que el usuario use git ni de que se acuerde de hacer commit. Y no debe estorbar: casi nunca se usa, pero cuando se usa salva el día.

**Restricciones desde ya:**

- El historial de versiones va en `data`, no en `state` ni en `cache`.
- Se indexa por ruta relativa al vault y sigue los renombres (§1). La historia de una nota borrada se conserva un tiempo, para poder restaurarla.
- Toda escritura de una nota pasa por un solo punto en `metalmind-vault` (hoy `write_atomic`), que es donde se registra la versión. Ninguna escritura del vault, tampoco la de la CLI (§7) ni la de un renombre de tags, se salta ese punto.
- Los cambios externos que ve el watcher también son versiones (decidido el 2026-10-18, en la PR #53): si Syncthing trae una versión mala, se puede volver a la anterior. Hecho así en el núcleo (03 §4): cada nota creada, modificada o renombrada desde fuera se registra en el hilo del vault; lo que cambió con Metalmind cerrado todavía no ([#55](https://github.com/alex-glez-felix/metalmind/issues/55)).
- Notas cifradas: ver §4.
- Retención (decidido el 2026-10-18, en la PR #53): guardar una versión por cada guardado, pero juntar las del autosave (03 §5): todas las de las últimas 24 h, una por hora durante 30 días y una por día después. La historia de una nota borrada se guarda 90 días. Las versiones se guardan comprimidas y deduplicadas por contenido. La retención se puede cambiar en la config.
- Detalles de la retención (decidido el 2026-10-18, en la PR #53): de cada hora o día se queda la **última** versión guardada en él, el estado con que se cerró ese periodo; horas y días son los de la **zona horaria local del sistema**, no de UTC ni de una zona fija: "un día" es el día del usuario, y la hora que se repite al volver del horario de verano cuenta como una sola; la última versión de una nota se guarda siempre, por vieja que sea; "una por día después" es **para siempre** (`daily_days: ~`), porque el texto pesa poco; si hace falta un tope, se configura. Defaults en `history:` de `config.yaml`: `keep_all_hours: 24`, `hourly_days: 30`, `daily_days: ~`, `deleted_days: 90`.
- Cómo se juntan las del autosave (decidido el 2026-10-19, en la PR del autoguardado): una versión autoguardada **reemplaza** a la anterior si esa también fue autoguardada y su grupo empezó hace menos de **5 minutos** (`AUTOSAVE_GROUP`). Mientras se sigue editando queda una versión por tramo de 5 minutos, la última de cada uno, y la última al dejar de editar, sin temporizadores: con un autoguardado por segundo, doce minutos de escritura dejan 3 versiones y no 720. Un guardado pedido (`:w`, `:wa`, `:w!`) siempre deja la suya y corta el grupo: el siguiente autoguardado empieza otro. Un `:w` de lo que se acaba de autoguardar no repite el contenido: fija esa versión, que el siguiente autoguardado ya no reemplaza. Un cambio desde fuera tampoco se reemplaza ni reemplaza nada; el eco del propio guardado que ve el watcher es igual a la última versión y no suma. El reemplazo se escribe como un marco más del log (sigue siendo de solo agregar, a prueba de cortes); los registros de las reemplazadas ocupan lugar hasta la poda siguiente, que rehace el log de la nota aunque no se lleve ninguna versión. Encima de esto corre la retención de siempre. Se eligieron 5 minutos porque dentro de una sesión de escritura se vuelve a "lo que tenía hace un rato", no a un segundo concreto, y así una hora de escritura deja a lo sumo 12 versiones, que la retención baja a una por hora pasadas 24 h. No se configura hasta que alguien lo pida.
- Formato (2026-10-18, 03 §4 "Estado (historial…)"): un log por nota con deltas por línea sobre la versión anterior y snapshots comprimidos cada tanto, más un índice ruta → log. Se eligió midiendo: con cambios chicos, copias enteras comprimidas ocupan de 70 a 600 veces más.
- Renombrar encima de una ruta que ya tenía historia (una nota borrada ahí, o una pisada desde fuera) **deja la historia vieja aparte**, como una nota borrada que se puede recuperar: no mezcla dos notas distintas (decidido el 2026-10-18, en la PR #53). Volver a guardar una nota en la ruta de una borrada sigue su historia.
- Solo notas markdown, no adjuntos (§6) **(propuesta)**.

## 4. Cifrado de notas

**Decisión.** Deseable, no obligatorio, sin fecha.

**Por qué.** Algunas notas no deberían poder leerse aunque alguien tenga una copia del vault o del disco. No es urgente, pero agregarlo tarde a un sistema que ya copió el contenido en cinco lugares es imposible de arreglar: lo que ya se filtró, se filtró.

**Restricción desde ya.** El índice, la caché (`redb`), el historial (§3), el índice de búsqueda (`tantivy`), el journal de recuperación (03 §5) y los logs **nunca guardan en claro** el contenido de una nota cifrada. En concreto:

- Cada componente que persiste algo derivado del contenido de una nota (texto, títulos, headings, tags, links, fragmentos de búsqueda) lo hace a través de un punto que pueda decir "esta nota no se persiste en claro". Hoy no hay cifrado, pero no puede haber caminos que se lo salten.
- Los logs llevan rutas, tamaños y errores, **nunca contenido** de notas, cifradas o no. Es la única forma de que la regla se cumpla sin acordarse caso por caso.
- Lo que solo vive en memoria mientras la nota está abierta y descifrada no tiene esta restricción.
- El historial (§3) ya tiene ese punto: todo contenido pasa a disco por un códec (`metalmind-vault`, `history/codec.rs`) y cada registro guarda cuál usó. Una nota cifrada será un códec que cifra después de comprimir, así que ni snapshots ni deltas quedan en claro; los deltas se calculan en memoria. El hash que identifica cada versión tendrá que ser con clave (`blake3::keyed_hash`) para no delatar un contenido conocido.
- Al cifrar una nota que ya tiene historial, sus versiones se vuelven a cifrar; ninguna queda en claro ni se borra (decidido el 2026-10-24, [07 §5](07-features-otras-apps.md#5-preguntas-y-respuestas), pregunta 7).

Decidido el 2026-10-24 ([07 §5](07-features-otras-apps.md#5-preguntas-y-respuestas), pregunta 6):

- **La ruta queda en claro**, en disco y en git: carpeta y nombre de una nota cifrada se ven. Es el costo aceptado de que la ruta sea la identidad (§1); esconderla exigiría nombres opacos. Los tags, los links y el frontmatter van dentro del cifrado, así que de una nota cifrada cerrada el índice solo sabe la ruta.
- **Ocultas por defecto en la UI**: el árbol, el finder, la búsqueda y el picker de links no muestran las notas cifradas. Una acción con nombre, con su toggle en la UI, las muestra, y se vuelven a ocultar solas pasado un tiempo, igual que se vence la sesión de desbloqueo (07 §2.7).

Abierto **(propuesta)**:

- Formato: [`age`](https://age-encryption.org/) (archivo `nota.md.age`, legible con la herramienta estándar fuera de Metalmind), frente a algo propio. `age` mantiene la regla de que cualquier herramienta puede leer el vault.
- Unidad: la nota o la carpeta. Por carpeta es más simple de usar (todo lo de `privado/` va cifrado) pero cambia qué pasa al mover una nota dentro o fuera.

## 5. Sin sistema de tareas

**Decisión.** Metalmind no junta los `- [ ]` del vault ni tiene una vista de tareas. Notas y tareas son cosas distintas, y las tareas viven en otra herramienta. Los checkboxes siguen siendo markdown normal que se pinta y se puede marcar (M4, 03 §8).

**Por qué.** Un gestor de tareas a medias dentro de una app de notas compite mal con uno de verdad y empuja el formato de las notas hacia el de una base de datos.

**Restricción desde ya.** Ninguna. Lo que hay que evitar es lo contrario: no indexar tareas ni guardar su estado "por si acaso".

## 6. Adjuntos

**Decisión.** Una carpeta de adjuntos por vault, configurable, por defecto `adjuntos/`. Pegar una imagen con `ctrl-v` en el editor la guarda ahí y escribe el `![[...]]` en la nota. Renombrar o borrar una nota **no** toca sus adjuntos, salvo que se pida explícitamente.

**Por qué.** Una sola carpeta es predecible y fácil de sincronizar o limpiar; un adjunto puede usarse en varias notas, así que no "pertenece" a una.

**Restricciones desde ya:**

- La carpeta de adjuntos es un ajuste del vault (config por vault, 03 §5), no global.
- El walk del vault (M2) conoce los archivos que no son `.md`: `![[img.png]]` se resuelve contra ellos igual que un wikilink contra las notas.
- Ninguna operación sobre notas (renombrar, mover, borrar, renombrar tags) borra o mueve adjuntos por su cuenta. Mover un adjunto sí actualiza los `![[...]]` que lo usan (mismo motor que §1).
- Nombre del archivo pegado **(propuesta)**: `<nombre-de-la-nota>-<fecha y hora>.png`, para que dos pegados no choquen y se sepa de dónde vino. Es lo que hace el núcleo hoy, también al arrastrar (con el nombre del archivo) y con la extensión que dice el contenido (03 §4, "Estado: adjuntos en el núcleo").

## 7. CLI de primera clase

**Decisión.** Metalmind tiene una CLI para capturar e ingerir notas desde la terminal, desde el celular (vía SSH o un atajo) o desde agentes, y extensible. Todo lo que hace el vault (crear notas, leer tags, buscar, listar) funciona **sin la ventana abierta**, desde los crates del núcleo.

`metalmind <archivo>` sigue abriendo un archivo y `metalmind <carpeta>` un vault. Los subcomandos son palabras reservadas: `metalmind -- add` abre un archivo llamado `add`.

**Por qué.** Capturar tiene que costar un comando, desde donde sea. Y un agente que puede consultar el vault en JSON lo usa mejor que uno que tiene que leer los archivos a mano.

Comandos (2026-10-18: `add`, `search` solo con rutas, `open` y `--json` hechos; la referencia es [`cli.md`](../cli.md)):

| Comando | Qué hace |
|---|---|
| `metalmind add "texto" -t tag` | Crea una nota. Sin `-t`, cae en el buzón (§9). Sin texto, lee de stdin (`wl-paste \| metalmind add`). |
| `metalmind search "#rust gpui"` | La misma consulta que `space f` (05 §4): tags más fuzzy sobre rutas; full-text cuando llegue M6. Hoy solo rutas: un término `#…` es error de uso hasta que la CLI use el `TagIndex` ([#47](https://github.com/alex-glez-felix/metalmind/issues/47)). |
| `metalmind tags` | Los tags con conteo y sus padres (el grafo de `parents` de las páginas de tag, 05 §7). Pendiente en [#47](https://github.com/alex-glez-felix/metalmind/issues/47), sobre el `TagIndex`. |
| `metalmind open <ruta>` | Abre la app en esa nota. La ruta es relativa al vault. |
| `--json` en todos | Salida estable para que la consuman agentes y scripts: un objeto por línea, errores con `kind` y código de salida fijos. |

**Restricciones desde ya:**

- La lógica de cada comando vive en el núcleo (`metalmind-vault`, `metalmind-search`); el binario solo parsea argumentos y elige. Un subcomando **no inicializa GPUI** ni abre ventana, y arranca en lo que tarda leer el vault, no en lo que tarda Vulkan.
- Las escrituras de la CLI son atómicas y pasan por el mismo punto que las de la app (§3). La app abierta se entera por el watcher, como de cualquier otro cambio externo (§2). Nada de IPC obligatorio entre la CLI y la app.
- Los tipos que devuelven las consultas del núcleo (notas, tags, resultados de búsqueda) se pueden serializar, con rutas relativas al vault: eso es el `--json`.
- Qué vault usa la CLI (decidido el 2026-10-18, PRs #44 y #48): la misma regla que `metalmind` sin argumentos (§8), con `--vault` en el lugar del argumento: `--vault`, luego `METALMIND_VAULT`, luego el último que se abrió, luego `vault:` de `config.yaml`. `--vault` y `METALMIND_VAULT` se eligieron a propósito: si no sirven, es error; del último abierto y la config se usa el primero que siga siendo una carpeta. Es una sola función del binario, `target::default_vault`, para la app y la CLI.
- Dónde y con qué nombre crea `metalmind add` la nota (decidido el 2026-10-18, PR #48): en la raíz del vault, como `slug.md`, con el slug del título o de la primera línea; si no hay ninguno (solo emoji o puntuación), `AAAA-MM-DD-HHMMSS.md`. Sin fecha delante: la nota se nombra para enlazarla (`[[idea-nueva]]`), y la fecha ya va en los metadatos del archivo y en el historial. Si el nombre está tomado, `-2`, `-3`, … sin pisar nunca (`create_atomic`, el mismo módulo de `write_atomic`).
- `metalmind open` con la app ya abierta: si reutiliza la ventana o abre otra queda abierto **(propuesta)**; reutilizarla sí necesita IPC, pero solo para eso.

## 8. Un vault a la vez

**Decisión.** Puede haber varios vaults, pero se abre uno. Metalmind recuerda el último y se cambia con `metalmind <carpeta>` o con `:vault`.

**Por qué.** Varios vaults abiertos a la vez multiplican estado y casos raros para un caso de uso que nadie pidió.

**Restricciones desde ya:**

- Caché, historial (§3) y estado de UI (vista del panel, último focus, notas abiertas) van **por vault**, en su directorio de `cache`, `data` o `state` según corresponda, nunca mezclados.
- El vault es un valor que se crea y se suelta, no un global: `:vault` tiene que poder cerrar uno y abrir otro en el mismo proceso sin que nada quede apuntando al anterior.
- Config: la global y encima la del vault (`<vault>/.metalmind/`, 03 §5).
- El último vault abierto se guarda en `state`. Prioridad al arrancar (decidido por el autor el 2026-10-18, en la PR #44): el argumento (o `--vault` en los subcomandos de la CLI, §7), luego la variable de entorno `METALMIND_VAULT`, luego el último abierto, luego `vault:` de `config.yaml` (hecho en M2, 03 §8).
- Cómo se identifica un vault para su directorio en `data`/`cache`/`state` (decidido por el autor el 2026-10-18, en la PR #44): por su ruta canónica, con la carpeta `<nombre>-<hash>`: el nombre de la carpeta del vault para reconocerla a ojo y un FNV-1a de 64 bits de la ruta canónica para que dos vaults no choquen (`Paths::vault_state`, hecho en M2 para los recientes del finder y el estado del panel lateral; `Paths::vault_data`, con la misma clave, para el historial de §3). Si se mueve el vault, la clave cambia; al abrir un vault que parece movido, Metalmind ofrece reasignarle el historial del directorio viejo (decidido el 2026-10-24, [07 §5](07-features-otras-apps.md#5-preguntas-y-respuestas), pregunta 18). Cómo se reconoce "parece movido" **(propuesta)**: no tiene directorio en `data` y hay otro con el mismo `<nombre>` cuyas rutas de historial coinciden con las notas del vault. Nada de identificador dentro del vault (`<vault>/.metalmind/`): sobreviviría a mover la carpeta, pero choca con la sincronización, porque deja de ser único en cuanto el vault se copia a otra máquina con Syncthing.

## 9. Buzón

**Decisión.** El flujo de trabajo del autor es: capturar rápido a un buzón y después categorizar. El buzón es una **vista: las notas sin tags**. No hay carpeta especial. Se vacía etiquetando: en cuanto una nota tiene un tag, sale del buzón. La CLI (§7) y la captura rápida dejan la nota ahí, sin tags.

**Por qué.** Encaja con que los tags son el eje (05): categorizar *es* poner tags, así que una nota sin tags es, por definición, una nota sin categorizar. Una carpeta de buzón obligaría a mover archivos al categorizar, y mover no es lo que organiza en Metalmind.

**Restricciones desde ya:**

- `TagQuery` (05 §4) expresa "sin tags" con `-#`; `-#a` excluye un tag concreto.
- El nodo **Sin tags** que 05 §3 proponía al final del panel es el buzón, y se llama así en la UI.
- La captura nunca pone tags por su cuenta (ni `#inbox` ni similares): eso rompería la definición.
- Las páginas de tag (`tags/<nombre>.md`, 05 §7) no son notas del buzón aunque no tengan tags, y "sin tags" en `TagQuery` no las devuelve.
- En focus del buzón, una nota nueva nace sin tags, que es justo lo que ya dice 05 §3 para el focus de tags.

## 10. Vistas guardadas

**Decisión.** Una vista guardada es una consulta de tags (05 §4) con nombre. Aparecen en el panel junto a los tags y se puede enfocar en ellas (05 §3). Ejemplos: `buzón` = sin tags (§9); `gs` = `#gs -#archivado`.

**Por qué.** Las consultas que se repiten a diario no deberían escribirse cada vez; es lo que 05 §8 y [`IDEAS.md`](../IDEAS.md#tags) ya anotaban como idea.

**Restricciones desde ya:**

- Se guardan con el vault, no en la config global: una consulta de tags solo tiene sentido en el vault donde existen esos tags, y así viaja con él.
- Una vista apunta a una consulta en texto, que se parsea con el mismo `TagQuery` de 05. Agregarle "o" o comodines después no cambia el formato de la vista.
- Dónde exactamente **(propuesta)**: en `<vault>/.metalmind/config.yaml`, bajo una clave `views` con nombre → consulta. Es la config del vault de 03 §5 y ya está dentro del vault, así que se sincroniza con él.
- `buzón` viene predefinida **(propuesta)**: no hace falta escribirla, pero se le puede cambiar el nombre.

## 11. Baratas, sin restricción de arquitectura

Se quieren y no condicionan nada de lo que se escriba hoy:

- **Daily notes**: opcional y **apagado por defecto**, porque el flujo del autor es el buzón (§9). Hecho en el núcleo (abajo).
- **Plantillas de notas**: hechas en el núcleo (abajo).
- **Grafo**: deseable, prioridad baja. Sale del índice de links de M3 (03 §4).
- **Temas**: ya existen (03 §5).

### Plantillas y nota del día, en el núcleo

Hecho en `metalmind-template`, sin UI todavía. Decidido el 2026-10-18 en la PR #56: la carpeta, las variables, las fechas en español, la plantilla por tag con su regla de choque y la nota del día apagada en `diario/YYYY-MM-DD`.

- **Plantillas**: notas markdown en `plantillas/` (clave `templates_folder` de `config.yaml`), con subcarpetas. El nombre de una plantilla es su ruta dentro de la carpeta sin `.md`: `trabajo/semanal`, exacto, con mayúsculas.
- **Variables**: solo las básicas, `{{title}}`, `{{date}}` (`YYYY-MM-DD`) y `{{time}}` (`HH:mm`), más `{{date:FORMATO}}` y `{{time:FORMATO}}` para otra forma de la fecha. Se quedan **porque son simples**: cubren lo que una nota nueva necesita y se aprenden de un vistazo. No es por compatibilidad con Obsidian, que no es requisito; que se escriban igual que allá es una coincidencia que no obliga a seguirlo. Se aceptan espacios dentro de las llaves. Sin condicionales, bucles ni escapes.
- **Formato de fecha**: los códigos más comunes de moment.js, una notación conocida y corta: `YYYY YY MMMM MMM MM M DD D dddd ddd HH H hh h mm m ss s A a Z ZZ` y `[texto]` literal. Otra letra se copia tal cual, como en moment. Los nombres de meses y días van en español y en minúsculas (`domingo 18 de octubre`); cuando la app tenga idioma, salen de él.
- **Lo mal escrito se deja tal cual y se reporta**, con su rango en bytes de la plantilla: una variable desconocida (`{{autor}}`, `{{Date}}`), un `{{` sin `}}` en su línea y un formato inválido (`{{date:}}`, un `[` sin cerrar). La nota se crea igual.
- **La fecha es la local**: la API recibe un `jiff::Zoned` y todo sale en su zona horaria. Pasada la medianoche local ya es la nota del día siguiente.
- **Plantilla por tag**: `template: nombre` en la página del tag (05 §7), que lee el `TagGraph` de `metalmind-vault` como los demás campos y la UI cambia con `PageChange::Template`. Una nota nueva con ese tag, o con uno de sus alias, o creada en focus de él, la usa. Si sus tags piden plantillas **distintas**, no se usa ninguna y se reporta, como con los alias que chocan (05 §7): elegir una adivinaría. Una plantilla elegida a mano gana sobre la del tag, y se puede pedir "sin plantilla". Un tag no hereda la plantilla de sus `parents`.
- **Tags de la nota nueva**: van al `tags:` del frontmatter de la plantilla ya llena, después de los suyos y sin repetir. Una lista de flujo de una línea o una lista en bloque se alarga en su estilo, con sus comentarios; otro valor (un texto, una lista de flujo en varias líneas) se reescribe como `tags: [a, b]`. Un tag que YAML leería como otra cosa (`null`, `007`) va entre comillas, con el mismo `yaml_scalar` que la metadata de las páginas. Sin frontmatter se le agrega uno. Si el frontmatter de la plantilla tiene errores o el `tags:` no está escrito en bloque, la nota sale con la plantilla tal cual y se reporta.
- **Nota del día**: apagada por defecto. Con `daily.enabled: true`, una función pura da su ruta (`daily.folder`, por defecto `diario/`, más el nombre en `daily.format`, por defecto `YYYY-MM-DD`; un `/` en el formato hace subcarpetas) y su texto con `daily.template`, si tiene. Sin plantilla nace vacía. Si ya existe, quien llama la abre en vez de crearla.
- **La UI** (2026-10-26, [plantillas](../plantillas.md)): `notes::new_from_template` (`space n`, `:template [plantilla [nombre]]`) elige la plantilla en un picker y pregunta el nombre, que es una ruta del vault con o sin `.md` y da `{{title}}` con su último componente. `a` sobre un tag del panel crea una nota con él y con la plantilla de su página (`Pick::ByTags`). `notes::open_daily` (`space d`, `:today`) abre la nota del día y la crea si falta; apagada, lo dice. Las plantillas se leen del disco al empezar cada pedido, no se mantienen en memoria: son pocas y chicas.
- **Una plantilla elegida a mano trae el tag que la pide**, si lo pide la página de un solo tag (`tag_of_template`); si la piden varias, ninguno, por la misma razón que el choque de arriba. Elegirla a mano sigue ganando sobre las de los tags.
- **El cursor** de la nota nueva queda bajo el primer heading con la sección vacía, sin contar uno cuyo contenido son solo subsecciones (`Draft::cursor`). Sin una variable `{{cursor}}`: la regla acierta con las plantillas de secciones, y otra variable sería más lenguaje.
- **Crear no pisa nunca**: `Vault::create_note` escribe el texto de una vez, crea las carpetas que falten y falla si la nota aparece mientras tanto. La nota del día que ya está en el índice se abre sin escribir nada; si el índice va detrás del disco, crearla falla y se abre igual. Sus plantillas se leen solo si usa una: una `templates_folder` inválida no la bloquea.
- **Pendiente**: que el finder, el buzón y los conteos no traten las plantillas como notas normales (lo mismo que las páginas de tag, 05 §7) y `metalmind add --template` ([#145](https://github.com/alex-glez-felix/metalmind/issues/145)).

## 12. Fuera de alcance

- **Publicar a la web.** Son archivos markdown en git; para publicarlos ya hay muchas herramientas. Exportar una nota a HTML o PDF (post-MVP, 03 §8) es otra cosa y sigue ahí.

## 13. Una GUI, no una TUI

**Decisión.** Metalmind es una GUI con navegación estilo vim, no una TUI. El vim es para moverse y editar rápido con el teclado, pero no todo tiene que hacerse editando archivos a mano: toda función se puede usar desde la UI (ratón, menús, paleta) además del teclado, y el usuario nunca debería necesitar editar archivos de config o de metadata a mano.

**Referencia de estilo: Zed.** La experiencia buscada se parece a la de Zed: navegar con la facilidad de vim en una UI de escritorio bonita, no en una TUI. Zed es también de donde sale GPUI (02, 04).

**Por qué.** Una ventana puede mostrar y ofrecer cosas que una terminal no; si la única forma de darle color a un tag es escribir YAML, Metalmind desperdicia lo que lo hace distinto de un editor en la terminal.

**Restricciones desde ya:**

- Cada acción con nombre (`CLAUDE.md`) es alcanzable por teclado (atajo, paleta o `:`) **y** por la UI (menú contextual, botón o selector). Una función que solo existe como comando, o solo como clic, está a medias.
- Los archivos son el **almacenamiento**, no la interfaz. Lo que se cambia desde la UI se escribe en el archivo de siempre (frontmatter de una página de tag, `config.yaml`), tocando solo el campo editado, y editarlo a mano da el mismo resultado. Ejemplo: las páginas de tag, que Metalmind crea solo en cuanto les pones metadata desde la UI (05 §3, §7).
- La UI no crea archivos que nadie pidió: escribir un `#tag-nuevo` no crea su página.
- La config (03 §5) también se edita desde una pantalla de ajustes que escribe el YAML; cuándo llega esa pantalla se decide con el roadmap.

## 14. Sin panel de vista previa

**Decisión** (2026-10-18). Metalmind nunca tendrá un panel de vista previa aparte, al lado o en otra pestaña, como el de muchos editores de markdown. Se descartó.

**Por qué.** En Metalmind se escribe y se lee en el mismo lugar. Una vista duplicada parte la atención entre dos copias del mismo texto y parte el foco de vim: los movimientos, las búsquedas y las marcas valen en una y no en la otra. El caso que cubriría una vista previa (leer la nota ya formateada) lo cubre el live preview: en las líneas donde no está el cursor, la sintaxis se oculta y se ve el resultado, y la línea del cursor muestra la fuente.

**Restricciones desde ya:**

- El estilo del markdown vive en el editor: sale de `metalmind-syntax` y lo pinta la misma vista que edita (03 §2, "Estilo en el editor"). Nada se calcula pensando en un render aparte.
- Lo que hoy se atenúa (`**`, `[[`, `#`) queda marcado en el modelo como ocultable, para que el live preview (paso 2) lo esconda en la misma vista.
- Exportar a HTML o PDF (03 §8, post-MVP) es otra cosa: un archivo que sale de Metalmind, no una vista que acompaña a la nota.

## 15. Fórmulas con RaTeX

**Decisión** (Alex, 2026-10-19, en la PR #80). Las fórmulas `$…$` y `$$…$$` se componen con RaTeX (`ratex-parser`, `ratex-layout`, `ratex-render`), con las fuentes de KaTeX embebidas. Documentos LaTeX completos, no.

**Por qué.** En el spike ([08](08-matematicas.md)) fue el único camino que compuso las 22 fórmulas de prueba, diagrama conmutativo incluido. Suma 3.1 MB al binario y tarda 0.26 ms por fórmula; mitex + Typst sumaba 45 MB y necesita un mitex parcheado. Su layout da el alto de la fórmula en µs, antes de pintarla.

**Restricciones desde ya:**

- Solo `metalmind-ui` depende de RaTeX. El núcleo dice dónde hay una fórmula y cuál es su texto, nada más.
- Dentro de `$…$` y `$$…$$` no hay tags ni links, igual que en el código inline. Es el mismo scanner para el editor y el índice, así que cuando llegue, un `#` dentro de una fórmula deja de ser tag.
- La línea del cursor muestra la fuente, como en el resto del live preview: el motor de vim trabaja siempre sobre el texto (08 §7).

## Dónde entra en el roadmap

Sin hitos nuevos (03 §8) **(propuesta)**:

- La CLI (`metalmind add`, `search`, `tags`, con `--json`) y el buzón, junto con M2.5 o justo después: necesitan el walk de M2 y el `TagIndex` de M2.5, y nada más.
- El historial de versiones (§3), en M6, junto con el autosave, que es lo que decide su retención.
- Daily notes, plantillas y grafo siguen en post-MVP. Celular y cifrado no tienen lugar en el roadmap ([`IDEAS.md`](../IDEAS.md)).
