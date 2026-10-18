# 06 — Decisiones de fondo

> Decisiones de producto del 2026-10-18 que afectan la arquitectura. Son **decisiones**, no propuestas; lo que no venía de ellas va marcado **(propuesta)** y queda pendiente de confirmar.
> Muchas de estas funciones llegan tarde o nunca. Lo que importa de cada una son las **restricciones que impone desde ya**: lo que el código de hoy no puede hacer sin cerrarle la puerta. Construye sobre [03](03-core-architecture.md) (crates en §6, roadmap en §8) y [05](05-tags.md).

## TL;DR

| # | Decisión | Restricción principal desde ya |
|---|---|---|
| 1 | La identidad de una nota es su **ruta**; sin ID en el frontmatter | Índice e historial siguen los renombres, también los que llegan por el watcher como borrar + crear |
| 2 | Celular: quizá algún día una app de captura; fuera de alcance | Núcleo sin GPUI ni supuestos de escritorio; archivos planos; cambios externos en cualquier momento |
| 3 | Historial de versiones por nota, escondido (`:history`) | Vive en el directorio de **datos**, por vault; no depende de git |
| 4 | Cifrado de notas: deseable, sin fecha | Nada derivado guarda en claro el contenido de una nota cifrada |
| 5 | Sin sistema de tareas | No indexar tareas "por si acaso" |
| 6 | Una carpeta de adjuntos por vault, `ctrl-v` pega imágenes | Renombrar o borrar una nota no toca sus adjuntos |
| 7 | CLI de primera clase (`nota add`, `search`, `tags`, `--json`) | Todo lo del vault funciona sin ventana, desde el núcleo |
| 8 | Un vault abierto a la vez | Caché, historial y estado de UI van por vault |
| 9 | Buzón = las notas sin tags | La captura deja la nota sin tags; `TagQuery` sabe decir "sin tags" |
| 10 | Vistas guardadas: consultas de tags con nombre, en el panel | Se guardan con el vault, no en la config global |
| 11 | Daily notes, plantillas, grafo, temas | Ninguna |
| 12 | Publicar a la web: fuera de alcance | — |
| 13 | nota es una GUI con navegación estilo vim, no una TUI | Toda función se usa desde la UI además del teclado; nadie edita config ni metadata a mano por necesidad |

---

## 1. La identidad de una nota es su ruta

**Decisión.** Una nota es su ruta relativa al vault. No hay ID en el frontmatter ni en ningún otro lado. Renombrar o mover una nota desde nota actualiza los `[[links]]` y los tags que apuntan a ella (el motor de refactorización de 03 §4 y 05 §5).

**Por qué.** Un ID en cada archivo ensucia notas que deben leerse con cualquier herramienta, y la ruta ya es única. Es reversible: si algún día hace falta un ID, un comando lo escribe a todas las notas de una vez.

**Restricciones desde ya:**

- `NoteId` (03 §4) sigue siendo un entero interno de la sesión, nunca algo que se guarde en un archivo del usuario ni que aparezca en un link.
- Todo lo que se guarda fuera de la memoria (caché, historial §3, estado de UI §8) se indexa por **ruta relativa al vault**, y tiene que poder **seguir un renombre**: mover la entrada, no perderla ni duplicarla.
- El watcher entrega `Renamed` cuando inotify da el par rename-from/rename-to, pero `git checkout`, Syncthing y muchos editores renombran como **borrar + crear**. `nota-vault` trata un `Removed` y un `Created` con el mismo contenido (hash igual, dentro de la misma ventana del debouncer) como un `Renamed`. El `TagIndex` (05 §2), el índice de links y el historial reciben un solo evento de rename, no un borrado y una nota nueva.
- Lo mismo al abrir el vault después de cambios hechos con nota cerrada: si falta una ruta que estaba en la caché y aparece una nueva con el mismo hash, es un rename **(propuesta)**. Así el historial de una nota renombrada desde fuera no queda huérfano.
- Ningún formato propio de nota (caché, historial, vistas guardadas) puede suponer que la ruta es inmutable.

## 2. Celular

**Decisión.** Fuera de alcance hoy. Si algún día hay algo, será una app simple de captura rápida; la app principal es la de escritorio.

**Por qué.** Lo que el autor necesita del celular es capturar, no editar ni navegar el vault; eso lo cubre la CLI (§7) más cualquier herramienta de sincronización.

**Restricciones desde ya:**

- El núcleo (03 §6) sigue sin GPUI **ni supuestos de escritorio**: nada de rutas fijas de Linux fuera de `nota-config`, nada que necesite un proceso de fondo o un servidor para funcionar.
- Las notas siguen siendo archivos markdown planos en carpetas, sincronizables con herramientas externas (Syncthing, git). Nada que dé sentido al vault vive fuera de él salvo lo regenerable (caché) y lo que es de esta máquina (historial §3, estado de UI §8).
- nota tolera **cambios externos en cualquier momento**: archivos que aparecen, cambian o desaparecen con la app abierta o cerrada. Sin locks de archivo. El conflicto con un buffer sin guardar se resuelve como dice 03 §5 (mostrar diff y elegir), nunca sobrescribiendo.
- Los archivos de conflicto de las herramientas de sync (`*.sync-conflict-*.md`, por ejemplo) son notas normales: nota no los esconde ni los fusiona por su cuenta **(propuesta)**.

## 3. Historial de versiones por nota

**Decisión.** nota guarda versiones de cada nota y permite verlas, compararlas y restaurarlas. Está **escondido**: no hay panel ni indicador a la vista; se llega con algo como `:history`, que muestra las versiones de la nota actual con su diff y deja restaurar una.

Se guarda en el directorio de **datos** (`$XDG_DATA_HOME/nota/`, `Paths::data` en `nota-config`), **por vault**. No va en el vault (no es contenido del usuario ni debe sincronizarse) ni en la caché (si se borra la caché, el historial se pierde de verdad, y la caché tiene que poder borrarse sin miedo). Funciona aunque el vault no sea un repo de git.

**Por qué.** Deshacer un cambio de hace tres días no puede depender de que el usuario use git ni de que se acuerde de hacer commit. Y no debe estorbar: casi nunca se usa, pero cuando se usa salva el día.

**Restricciones desde ya:**

- El historial de versiones va en `data`, no en `state` ni en `cache`. (El comentario de `Paths::state` habla de "historial": es el de `:` y búsquedas, no este.)
- Se indexa por ruta relativa al vault y sigue los renombres (§1). La historia de una nota borrada se conserva un tiempo, para poder restaurarla.
- Toda escritura de una nota pasa por un solo punto en `nota-vault` (hoy `write_atomic`), que es donde se registra la versión. Ninguna escritura del vault, tampoco la de la CLI (§7) ni la de un renombre de tags, se salta ese punto.
- Los cambios externos que ve el watcher también son versiones **(propuesta)**: si Syncthing trae una versión mala, se puede volver a la anterior.
- Notas cifradas: ver §4.
- Retención **(propuesta)**: guardar una versión por cada guardado, pero juntar las del autosave (03 §5): todas las de las últimas 24 h, una por hora durante 30 días y una por día después. La historia de una nota borrada se guarda 90 días. Las versiones se guardan comprimidas y deduplicadas por contenido; el formato se decide al implementarlo. La retención se puede cambiar en la config.
- Solo notas markdown, no adjuntos (§6) **(propuesta)**.

## 4. Cifrado de notas

**Decisión.** Deseable, no obligatorio, sin fecha.

**Por qué.** Algunas notas no deberían poder leerse aunque alguien tenga una copia del vault o del disco. No es urgente, pero agregarlo tarde a un sistema que ya copió el contenido en cinco lugares es imposible de arreglar: lo que ya se filtró, se filtró.

**Restricción desde ya.** El índice, la caché (`redb`), el historial (§3), el índice de búsqueda (`tantivy`), el journal de recuperación (03 §5) y los logs **nunca guardan en claro** el contenido de una nota cifrada. En concreto:

- Cada componente que persiste algo derivado del contenido de una nota (texto, títulos, headings, tags, links, fragmentos de búsqueda) lo hace a través de un punto que pueda decir "esta nota no se persiste en claro". Hoy no hay cifrado, pero no puede haber caminos que se lo salten.
- Los logs llevan rutas, tamaños y errores, **nunca contenido** de notas, cifradas o no. Es la única forma de que la regla se cumpla sin acordarse caso por caso.
- Lo que solo vive en memoria mientras la nota está abierta y descifrada no tiene esta restricción.

Abierto **(propuesta)**:

- Formato: [`age`](https://age-encryption.org/) (archivo `nota.md.age`, legible con la herramienta estándar fuera de nota), frente a algo propio. `age` mantiene la regla de que cualquier herramienta puede leer el vault.
- Unidad: la nota o la carpeta. Por carpeta es más simple de usar (todo lo de `privado/` va cifrado) pero cambia qué pasa al mover una nota dentro o fuera.
- Qué ve el índice de una nota cifrada cerrada: solo la ruta, sin tags ni links, es lo más simple y lo único que cumple la regla sin una caché cifrada.

## 5. Sin sistema de tareas

**Decisión.** nota no junta los `- [ ]` del vault ni tiene una vista de tareas. Notas y tareas son cosas distintas, y las tareas viven en otra herramienta. Los checkboxes siguen siendo markdown normal que se pinta y se puede marcar (M4, 03 §8).

**Por qué.** Un gestor de tareas a medias dentro de una app de notas compite mal con uno de verdad y empuja el formato de las notas hacia el de una base de datos.

**Restricción desde ya.** Ninguna. Lo que hay que evitar es lo contrario: no indexar tareas ni guardar su estado "por si acaso".

## 6. Adjuntos

**Decisión.** Una carpeta de adjuntos por vault, configurable, por defecto `adjuntos/`. Pegar una imagen con `ctrl-v` en el editor la guarda ahí y escribe el `![[...]]` en la nota. Renombrar o borrar una nota **no** toca sus adjuntos, salvo que se pida explícitamente.

**Por qué.** Una sola carpeta es predecible y fácil de sincronizar o limpiar; un adjunto puede usarse en varias notas, así que no "pertenece" a una.

**Restricciones desde ya:**

- La carpeta de adjuntos es un ajuste del vault (config por vault, 03 §5), no global.
- El walk del vault (M2) conoce los archivos que no son `.md`: `![[img.png]]` se resuelve contra ellos igual que un wikilink contra las notas.
- Ninguna operación sobre notas (renombrar, mover, borrar, renombrar tags) borra o mueve adjuntos por su cuenta. Mover un adjunto sí actualiza los `![[...]]` que lo usan (mismo motor que §1).
- Nombre del archivo pegado **(propuesta)**: `<nombre-de-la-nota>-<fecha y hora>.png`, para que dos pegados no choquen y se sepa de dónde vino.

## 7. CLI de primera clase

**Decisión.** nota tiene una CLI para capturar e ingerir notas desde la terminal, desde el celular (vía SSH o un atajo) o desde agentes, y extensible. Todo lo que hace el vault (crear notas, leer tags, buscar, listar) funciona **sin la ventana abierta**, desde los crates del núcleo.

`nota <archivo>` sigue abriendo un archivo y `nota <carpeta>` un vault. Los subcomandos son palabras reservadas: `nota -- add` abre un archivo llamado `add`.

**Por qué.** Capturar tiene que costar un comando, desde donde sea. Y un agente que puede consultar el vault en JSON lo usa mejor que uno que tiene que leer los archivos a mano.

Comandos tentativos **(propuesta)**:

| Comando | Qué hace |
|---|---|
| `nota add "texto" -t tag` | Crea una nota. Sin `-t`, cae en el buzón (§9). Sin texto, lee de stdin (`wl-paste \| nota add`). |
| `nota search "#rust gpui"` | La misma consulta que `space f` (05 §4): tags más fuzzy sobre rutas; full-text cuando llegue M6. |
| `nota tags` | Los tags con conteo y sus padres (el grafo de `parents` de las páginas de tag, 05 §7). |
| `nota open <ruta>` | Abre la app en esa nota. |
| `--json` en todos | Salida estable para que la consuman agentes y scripts. |

**Restricciones desde ya:**

- La lógica de cada comando vive en el núcleo (`nota-vault`, `nota-search`); el binario solo parsea argumentos y elige. Un subcomando **no inicializa GPUI** ni abre ventana, y arranca en lo que tarda leer el vault, no en lo que tarda Vulkan.
- Las escrituras de la CLI son atómicas y pasan por el mismo punto que las de la app (§3). La app abierta se entera por el watcher, como de cualquier otro cambio externo (§2). Nada de IPC obligatorio entre la CLI y la app.
- Los tipos que devuelven las consultas del núcleo (notas, tags, resultados de búsqueda) se pueden serializar, con rutas relativas al vault: eso es el `--json`.
- Qué vault usa la CLI sin `--vault`: el último que se abrió (§8) **(propuesta)**.
- Dónde y con qué nombre crea `nota add` la nota **(propuesta)**: en la raíz del vault, con nombre a partir de la primera línea y, si no hay, de la fecha y hora.
- `nota open` con la app ya abierta: si reutiliza la ventana o abre otra queda abierto **(propuesta)**; reutilizarla sí necesita IPC, pero solo para eso.

## 8. Un vault a la vez

**Decisión.** Puede haber varios vaults, pero se abre uno. nota recuerda el último y se cambia con `nota <carpeta>` o con `:vault`.

**Por qué.** Varios vaults abiertos a la vez multiplican estado y casos raros para un caso de uso que nadie pidió.

**Restricciones desde ya:**

- Caché, historial (§3) y estado de UI (vista del panel, último focus, notas abiertas) van **por vault**, en su directorio de `cache`, `data` o `state` según corresponda, nunca mezclados.
- El vault es un valor que se crea y se suelta, no un global: `:vault` tiene que poder cerrar uno y abrir otro en el mismo proceso sin que nada quede apuntando al anterior.
- Config: la global y encima la del vault (`<vault>/.nota/`, 03 §5).
- El último vault abierto se guarda en `state`. Prioridad al arrancar: el argumento, luego el último abierto, luego `vault:` de `config.yaml` (hecho en M2, 03 §8).
- Cómo se identifica un vault para su directorio en `data`/`cache`/`state`: por su ruta canónica, con la carpeta `<nombre>-<hash>`: el nombre de la carpeta del vault para reconocerla a ojo y un FNV-1a de 64 bits de la ruta canónica para que dos vaults no choquen (`Paths::vault_state`, hecho en M2 para los recientes del finder y el estado del panel lateral). Si se mueve el vault, su historial queda en la ruta vieja; un comando lo reasigna **(propuesta)**. La alternativa, un identificador en `<vault>/.nota/`, sobrevive a mover la carpeta pero deja de ser único en cuanto el vault se copia a otra máquina con Syncthing.

## 9. Buzón

**Decisión.** El flujo de trabajo del autor es: capturar rápido a un buzón y después categorizar. El buzón es una **vista: las notas sin tags**. No hay carpeta especial. Se vacía etiquetando: en cuanto una nota tiene un tag, sale del buzón. La CLI (§7) y la captura rápida dejan la nota ahí, sin tags.

**Por qué.** Encaja con que los tags son el eje (05): categorizar *es* poner tags, así que una nota sin tags es, por definición, una nota sin categorizar. Una carpeta de buzón obligaría a mover archivos al categorizar, y mover no es lo que organiza en nota.

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
- Dónde exactamente **(propuesta)**: en `<vault>/.nota/config.yaml`, bajo una clave `views` con nombre → consulta. Es la config del vault de 03 §5 y ya está dentro del vault, así que se sincroniza con él.
- `buzón` viene predefinida **(propuesta)**: no hace falta escribirla, pero se le puede cambiar el nombre.

## 11. Baratas, sin restricción de arquitectura

Se quieren y no condicionan nada de lo que se escriba hoy:

- **Daily notes**: opcional y **apagado por defecto**, porque el flujo del autor es el buzón (§9).
- **Plantillas de notas**: deseables.
- **Grafo**: deseable, prioridad baja. Sale del índice de links de M3 (03 §4).
- **Temas**: ya existen (03 §5).

## 12. Fuera de alcance

- **Publicar a la web.** Son archivos markdown en git; para publicarlos ya hay muchas herramientas. Exportar una nota a HTML o PDF (post-MVP, 03 §8) es otra cosa y sigue ahí.

## 13. Una GUI, no una TUI

**Decisión.** nota es una GUI con navegación estilo vim, no una TUI. El vim es para moverse y editar rápido con el teclado, pero no todo tiene que hacerse editando archivos a mano: toda función se puede usar desde la UI (ratón, menús, paleta) además del teclado, y el usuario nunca debería necesitar editar archivos de config o de metadata a mano.

**Referencia de estilo: Zed.** La experiencia buscada se parece a la de Zed: navegar con la facilidad de vim en una UI de escritorio bonita, no en una TUI. Zed es también de donde sale GPUI (02, 04).

**Por qué.** Una ventana puede mostrar y ofrecer cosas que una terminal no; si la única forma de darle color a un tag es escribir YAML, nota desperdicia lo que la hace distinta de un editor en la terminal.

**Restricciones desde ya:**

- Cada acción con nombre (`CLAUDE.md`) es alcanzable por teclado (atajo, paleta o `:`) **y** por la UI (menú contextual, botón o selector). Una función que solo existe como comando, o solo como clic, está a medias.
- Los archivos son el **almacenamiento**, no la interfaz. Lo que se cambia desde la UI se escribe en el archivo de siempre (frontmatter de una página de tag, `config.yaml`), tocando solo el campo editado, y editarlo a mano da el mismo resultado. Ejemplo: las páginas de tag, que nota crea sola en cuanto les pones metadata desde la UI (05 §3, §7).
- La UI no crea archivos que nadie pidió: escribir un `#tag-nuevo` no crea su página.
- La config (03 §5) también se edita desde una pantalla de ajustes que escribe el YAML; cuándo llega esa pantalla se decide con el roadmap.

## Dónde entra en el roadmap

Sin hitos nuevos (03 §8) **(propuesta)**:

- La CLI (`nota add`, `search`, `tags`, con `--json`) y el buzón, junto con M2.5 o justo después: necesitan el walk de M2 y el `TagIndex` de M2.5, y nada más.
- El historial de versiones (§3), en M6, junto con el autosave, que es lo que decide su retención.
- Daily notes, plantillas y grafo siguen en post-MVP. Celular y cifrado no tienen lugar en el roadmap ([`IDEAS.md`](../IDEAS.md)).
