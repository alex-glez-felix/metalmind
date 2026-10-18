# La CLI de Metalmind

Capturar y consultar el vault desde la terminal, un script o un agente, sin abrir la ventana ([06](research/06-decisiones-de-fondo.md) §7). Los subcomandos no inicializan GPUI: funcionan sin `DISPLAY` ni `WAYLAND_DISPLAY`, por SSH y en CI, y `metalmind add` termina en milisegundos.

```sh
metalmind add "Llamar a la imprenta"               # crea llamar-a-la-imprenta.md
wl-paste | metalmind add -t receta                 # el texto desde stdin
metalmind add --title "Reunión" -t trabajo < acta.txt
metalmind search imprenta                          # rutas, de la mejor a la peor
metalmind open llamar-a-la-imprenta.md  # abre la app en esa nota
```

Si la app está abierta, la nota nueva aparece sola: el watcher ve el archivo como cualquier cambio externo. No hay comunicación entre procesos.

`metalmind <archivo>` sigue abriendo un archivo. Los subcomandos son palabras reservadas: para abrir un archivo que se llama `add`, `metalmind -- add`.

## Qué vault usa

La misma regla que `metalmind` sin argumentos ([06](research/06-decisiones-de-fondo.md) §8), con `--vault` en el lugar del argumento:

1. `--vault <carpeta>`.
2. La variable `METALMIND_VAULT` (vacía no cuenta).
3. El último vault abierto en la app, guardado en `$XDG_STATE_HOME/metalmind/last-vault` (por defecto `~/.local/state/metalmind/`). Lo anota la app cada vez que abre un vault.
4. `vault:` de `config.yaml` (ver [config](config.md)).

`--vault` y `METALMIND_VAULT` se eligieron a propósito: si no son una carpeta, es error (código 3) en vez de seguir con el siguiente, para que un `METALMIND_VAULT` mal escrito no escriba en otro vault. Del último abierto y la config se usa el primero que siga siendo una carpeta. Si no queda ninguno, también es error (código 3).

## `metalmind add [TEXTO]... [-t TAG]... [--title TÍTULO]`

Crea una nota en la **raíz** del vault. El texto sale de los argumentos (varias palabras se unen con espacios) o, si no hay, de stdin. Si stdin es una terminal no se espera a que alguien escriba: sin texto es error, salvo que haya `--title`.

- **Sin `-t`, la nota cae en el buzón** ([06](research/06-decisiones-de-fondo.md) §9): Metalmind nunca le pone un tag por su cuenta.
- Con `-t`, los tags van en el frontmatter, entre comillas para que `2024` o `true` sigan siendo texto: `tags: ["rust", "gpui"]`. El `#` es opcional (`-t '#rust'` = `-t rust`) y los repetidos se juntan. Un tag inválido (`-t "con espacio"`) es error de uso y no se crea nada.
- Si el texto ya trae frontmatter, se respeta tal cual; combinarlo con `-t` es error de uso (habría que reescribir el YAML del usuario).
- `--title` va como `# Título` al principio del cuerpo, después del frontmatter si lo hay.
- El archivo termina siempre en `\n`, y los saltos de línea del final del texto se juntan en uno.

**Nombre del archivo**: `slug.md`, sin fecha: la nota se nombra para enlazarla (`[[idea-nueva]]`), y la fecha ya va en los metadatos del archivo. El slug sale del título o, si no hay, de la primera línea del texto con letras o números: las palabras en minúsculas unidas con `-`, con acentos (también descompuestos) y otros alfabetos tal cual (`Reunión del lunes` → `reunión-del-lunes.md`), hasta 50 caracteres sin cortar palabras. Sin slug (el texto es solo emoji o puntuación), el nombre es la fecha y la hora local: `2026-10-18-090507.md`. Si el nombre está tomado, se prueba `idea-2.md`, `idea-3.md`, …; nunca se pisa una nota.

**Escritura**: atómica y sin pisar nunca nada. Se escribe un temporal en la raíz, se baja a disco y se renombra con `RENAME_NOREPLACE`; si otro proceso crea el mismo nombre a la vez, gana uno y el otro pasa al siguiente sufijo. Si falla, no queda nada a medias.

Imprime la ruta creada, relativa al vault.

## `metalmind search CONSULTA [-n N]`

Fuzzy sobre las rutas del vault (notas y adjuntos), el mismo de `space f`: palabras separadas por espacio que tienen que coincidir todas; mayúsculas y acentos cuentan solo si la consulta los lleva. Da los `N` mejores (50 si falta), del mejor al peor; con la consulta vacía (`""`), las primeras `N` ordenadas por ruta; para todo el vault, un `-n` grande.

**Por ahora solo rutas.** La consulta de tags (`#rust -#archivado`, [05](research/05-tags.md) §4) llega con el índice de tags ([#47](https://github.com/alex-glez-felix/metalmind/issues/47)) y el texto completo con M6. Mientras tanto, un término que empieza con `#` o `-#` es error de uso en vez de buscarse como texto: así, cuando llegue, ninguna consulta que hoy funciona cambia de respuesta.

Sin coincidencias no es error: sale vacío y con código 0. En texto, una ruta por línea; si un nombre trae caracteres de control (un salto de línea, una secuencia de escape), salen escapados como `\n` o `\u{1b}`. `--json` da siempre la ruta exacta.

## `metalmind open RUTA`

Abre la app en una nota del vault. `RUTA` es relativa a la raíz del vault (no al directorio actual: para eso está `metalmind <archivo>`) o absoluta dentro de él, así que `metalmind open "$(metalmind add idea)"` funciona. Tiene que ser una nota (`.md`) que exista; si no, error de uso sin abrir nada. La app lo anota como el último vault abierto al abrirlo.

Este sí abre la ventana. Con `--json`, imprime qué va a abrir una vez leída la nota, justo antes de abrir la ventana, y el proceso dura lo que ella. Si después la ventana no puede abrirse (sin sesión gráfica, por ejemplo), el error sale en stderr con `kind: "io"` y código 1.

## `metalmind tags`

Todavía no existe: espera al índice de tags ([#47](https://github.com/alex-glez-felix/metalmind/issues/47)).

## Salida para agentes: `--json`

Todos los subcomandos aceptan `--json` y `--vault`. Con `--json`:

- La salida es **un solo objeto JSON en una línea** en stdout, terminado en `\n`.
- Las rutas de notas son **relativas al vault**, con `/`. `vault` es la raíz absoluta, sin enlaces simbólicos; si no es UTF-8, los bytes inválidos salen como `�`.
- Los errores van a stderr, también en una línea: `{"error":{"kind":"…","message":"…"}}`. `message` es para personas y puede cambiar; `kind` y el código de salida son el contrato.
- stderr puede traer además avisos en texto (una config rota, por ejemplo), que no cambian el resultado. El error, si lo hay, es siempre la última línea.

| Comando | stdout con `--json` |
|---|---|
| `add` | `{"vault": string, "path": string, "tags": [string]}` |
| `search` | `{"vault": string, "query": string, "results": [{"path": string, "kind": "note" \| "attachment"}]}` |
| `open` | `{"vault": string, "path": string}` |

Ejemplos:

```sh
$ echo "probar el watcher" | metalmind add -t metalmind --json
{"vault":"/home/usuario/notas","path":"probar-el-watcher.md","tags":["nota"]}

$ metalmind search watcher --json
{"vault":"/home/usuario/notas","query":"watcher","results":[{"path":"probar-el-watcher.md","kind":"note"}]}

$ metalmind search "#nota" --json
{"error":{"kind":"usage","message":"`#nota`: la búsqueda por tags todavía no existe; por ahora solo se buscan rutas"}}
```

Las claves no cambian de significado; si se agrega una, los que no la conocen la ignoran.

## Códigos de salida

| Código | `kind` | Cuándo |
|---|---|---|
| 0 | — | Salió bien, aunque la búsqueda no encuentre nada |
| 1 | `io` | Falló una lectura o escritura (sin permiso de escritura en el vault, disco lleno, stdin roto, una nota que no es UTF-8) o `open` no pudo abrir la ventana |
| 2 | `usage` | Lo pedido no vale: argumentos (los errores de clap también salen con 2, en texto), nota vacía, tag o título inválido, stdin que no es UTF-8, consulta con tags, `open` de algo que no es una nota del vault |
| 3 | `vault` | No hay vault, o el elegido no existe, no es una carpeta o no se puede leer |

## Desde un agente

Un agente puede usar el vault como memoria: capturar lo que aprende y consultarlo antes de preguntar. Con `METALMIND_VAULT` en su entorno, todo funciona sin ventana:

```sh
export METALMIND_VAULT=~/notas

# Guardar un hallazgo; el texto por stdin evita problemas de comillas.
path=$(printf '%s\n' "El build falla sin libxkbcommon-x11-dev." "Se instala con apt." \
  | metalmind add --json --title "Build: dependencias de GPUI" -t build | jq -r .path)

# ¿Ya hay algo sobre esto?
metalmind search --json "build gpui" | jq -r '.results[] | select(.kind == "note") | .path' \
  | while read -r note; do cat "$METALMIND_VAULT/$note"; done

# Un error se reconoce por el código, sin leer el mensaje.
metalmind search --json "#build" 2>err.json; [ $? -eq 2 ] && jq -r .error.message err.json
```

Lo que el agente crea sin `-t` queda en el buzón, para que una persona lo revise y lo etiquete.
