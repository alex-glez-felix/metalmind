# Changelog

Los cambios que importan a quien usa Metalmind. El formato sigue
[Keep a Changelog](https://keepachangelog.com/es-ES/1.1.0/) y las versiones,
[SemVer](https://semver.org/lang/es/). Cómo se publica una versión:
[docs/release.md](docs/release.md).

## [Unreleased]

### Agregado

#### CLI

- `metalmind add`: crea una nota en el vault con el texto del argumento o de stdin,
  como `slug.md` (`idea-2.md` si ya existe) y sin pisar nunca otra. Sin `-t`, la nota queda
  en el buzón; con `-t`, los tags van al frontmatter.
- `metalmind search`: búsqueda difusa sobre las rutas del vault.
- `metalmind open`: abre la app en una nota del vault.
- `--json` en todos, con una forma estable, y códigos de salida fijos. El
  vault sale de `--vault`, `METALMIND_VAULT`, el último abierto o `vault:` de la
  config, la misma regla que `metalmind` sin argumentos. Ninguno abre ventana
  salvo `open`. Ver [docs/cli.md](docs/cli.md).

#### Editor (M0)

- `metalmind archivo.md` abre el archivo en una ventana nativa (GPUI). Si no existe,
  empieza vacío y se crea al guardar.
- Guardado atómico: temporal en el mismo directorio, fsync y rename. Conserva
  los permisos, sigue enlaces simbólicos y, si algo falla, el original queda
  intacto.
- Como el `fixeol` de vim, el `\n` final del archivo no se ve como una línea
  vacía de más y al guardar se agrega si faltaba.
- Línea de comandos con `:w`, `:q`, `:q!` y `:wq`, con sus abreviaturas. `:q`
  y el cierre desde el gestor de ventanas se niegan si hay cambios sin
  guardar.
- Barra de estado con el modo, `[+]` si hay cambios y la línea de comandos. El
  `[+]` se quita al deshacer hasta lo guardado, como en vim.
- Portapapeles con `ctrl-shift-c` y `ctrl-shift-v`, y selección primaria con
  el ratón y el clic medio.
- La fuente es la primera monoespaciada instalada de una lista; si no hay
  ninguna, Metalmind lo dice y no abre.
- Sin sesión gráfica (ni Wayland ni X11) o sin driver de Vulkan ni OpenGL,
  Metalmind dice qué falta y sale con error en vez de quedarse corriendo sin
  ventana.

#### Modo vim (M1)

- Modos Normal e Insert, con cursor de bloque o de línea según el modo.
- Motions `h j k l`, `w W b B e E`, `0 ^ $`, `gg G`, y `f F t T` con `;` y
  `,`, todos con count.
- Operadores `d c y` (y `dd cc yy`), `x`, `p P` y el registro sin nombre.
- Text objects `iw aw iW aW`, `i( a(`, `i" a"` e `ip ap`.
- `u` y `ctrl-r` deshacen y rehacen. El historial es un árbol, como el
  undotree de vim, y `g-` y `g+` lo recorren en orden cronológico.
- `.` repite el último cambio, con count.
- Un count en Insert repite lo tecleado o pegado al salir con `Esc`;
  `ctrl-c` sale sin repetir.
- Cada caso de la tabla de vim se verifica contra Neovim 0.12.6.

#### Registros, macros y marks

- Registros de vim: `"a`–`"z` (con mayúscula se agrega al final), `"0`,
  `"1`–`"9`, `"-`, `"_`, `".` y `"/`, con `y d c p P x`, counts y `.`.
- `"+` y `"*` usan el portapapeles y la selección primaria del sistema.
- Macros con `q`, `@`, `@@` y count; una macro que falla se detiene, y `u`
  deshace la reproducción entera.
- Marks `a`–`z` con `m`, `'` y `` ` ``, también como motions de operador, y
  `''` para volver del último salto. Siguen al texto cuando se edita arriba.
  Una mark sin poner avisa `E20: Mark not set`.
- Marks de archivo `A`–`Z`: `mA` guarda la nota y la posición, y `'A` o
  `` `A `` vuelven a ella desde cualquier nota, en el panel activo. Siguen a
  la nota si se renombra o se mueve, desde el árbol o desde fuera; si se
  borra, dan `E20`.
- Jumplist como la de vim, de cada panel y entre notas: `ctrl-o` vuelve y
  `ctrl-i` (o Tab en Normal) avanza, con count. Agregan un salto `gg`, `G`,
  `%`, `H`, `M`, `L`, `/`, `?`, `n`, `N`, `*`, `#`, las marks, `:g` y abrir
  otra nota (`gd`, el finder, el árbol, `:e`). Hasta 100 entradas, sin
  repetir línea; se saltan las de notas que ya no existen.
- `%` va al paréntesis, corchete o llave que hace pareja, como en vim (ignora
  lo que está entre comillas y lo escapado); `50%` va a la mitad de la nota.

#### Comandos de `:`

- Rangos como en vim: `.`, `$`, `%`, números, marks (`'a`), `'<,'>` al
  pulsar `:` en Visual, `/patrón/` y `?patrón?`, con `+N`/`-N` y separados
  por `,` o `;`. Un rango solo (`:12`) va a esa línea.
- `:d` y `:y` con registro y count, `:m` y `:t`/`:co` para mover y copiar
  líneas, `:j` y `:j!`, `:>` y `:<`.
- `:sort` con `!`, `i`, `u`, `/patrón/` (con `r`) y por números: `n`
  (decimal), `x` (hexadecimal), `o` (octal), `b` (binario) y `f` (flotante).
- `:normal` (`:norm`) teclea lo que sigue en la línea del cursor o en cada
  línea del rango.
- `:g/patrón/comando`, `:g!` y `:v` corren `d`, `s`, `m0`, `t.`, `normal` o
  cualquier otro comando en cada línea donde está (o no) el patrón.
- `@:` repite la última línea de `:`, con count.
- Cada comando es un solo paso de `u`, también dentro de una macro.
- Los errores de vim se muestran con su número y su texto original
  (`E492: Not an editor command`), para buscarlos en `:help`, y el aviso de
  `:s` y `:g` también es el de vim (`3 substitutions on 2 lines`).

#### Visual

- Visual por carácter (`v`) y por línea (`V`), con `o`, `gv` y `[count]v`.
  Todas las motions, `/ ? n N` y las marks extienden la selección, y los text
  objects la seleccionan o la hacen crecer.
- Operadores `d x c y p P r ~ u U` sobre la selección, con registro, y `.`
  repite sobre la misma cantidad de texto. En Normal, `gu gU g~`.
- La ventana todavía no pinta bien la selección: le falta el último carácter
  y, con `V`, las líneas completas.

#### Scroll

- `ctrl-d` y `ctrl-u` (media pantalla; con count fijan las líneas para los
  siguientes), `ctrl-f` y `ctrl-b` (pantalla completa), `ctrl-e` y `ctrl-y`
  (una línea), `zt zz zb` y `z<CR> z. z-`, como en Neovim. En Visual extienden
  la selección.
- `H`, `M` y `L` van a la primera, la de en medio y la última línea visible,
  con count, y son motions de operador (`dL`).
- `scrolloff` de la configuración deja líneas visibles alrededor del cursor,
  también al moverlo con `j` y `k`. Cada panel tiene su propio scroll.
- La rueda del ratón lleva el cursor consigo para que siempre se vea, como en
  vim.

#### Markdown con estilo

- El editor muestra el markdown con estilo, como iA Writer: la sintaxis (`**`,
  `#`, `[[ ]]`, `` ` ``) se ve atenuada; los headings son más grandes y en
  negrita según el nivel; negrita, cursiva, código, tachado y `==resaltado==`
  tienen su estilo; links, wikilinks, tags y embeds su color, y un link a una
  nota que no existe, otro. Los bloques de código cercados llevan fondo y
  resaltado de los lenguajes más comunes; listas, tareas, citas, reglas y el
  frontmatter también se marcan.
- Colores y tamaños salen del tema: `syntax.*` y `headings.h1`–`h6` (tamaño
  relativo a la fuente y peso).
- Con headings más altos, el scroll, `H`, `M`, `L`, `zt`, `zz`, `zb`,
  `ctrl-d`, `ctrl-u`, el cursor, la selección, la rueda y los números de línea
  cuentan lo que mide cada línea.

#### Buffers y paneles (M2)

- Varios archivos abiertos a la vez. `:e ruta` abre otro en el panel activo
  (relativo a la raíz del vault), `:e` sin ruta relee el archivo y `:e!`
  descarta los cambios.
- Splits: `ctrl-w v` y `:vs` a la derecha, `ctrl-w s` y `:sp` abajo,
  `ctrl-w h j k l` y `ctrl-w w` mueven el foco, `ctrl-w q` y `:q` cierran el
  panel y `ctrl-w o` y `:on` dejan solo el actual. Dos paneles pueden mostrar
  el mismo archivo con cursores independientes.
- Como nvim, con `hidden`: cambiar de nota (con `:e`, el finder o el panel
  lateral) o cerrar un panel deja la nota de antes abierta en segundo plano,
  con sus cambios, su historial y su cursor. Al volver a una sin cambios, se
  relee si cambió en disco. Salir (`:q` en el último panel, `:qa` o cerrar
  la ventana) se niega mientras alguna nota tenga cambios: muestra cuál con
  `E162` y la abre. `:wa` guarda todas, `:wqa`/`:xa` guardan y salen y
  `:qa!` sale sin guardar.

#### Links (M3)

- `gd` o `enter` sobre un link abre su nota en el panel activo y pone el
  cursor en el heading o el bloque que nombre (`[[Vin#Entrenamiento]]`,
  `[[nota#^id]]`). También sobre un embed `![[…]]` y un link markdown
  `[x](nota.md)`, y como `editor::follow_link` desde la paleta. Si el nombre
  empata entre varias notas, se elige en el picker; si no lleva a ninguna,
  un aviso en la barra.
- La nota con cambios sin guardar queda abierta en segundo plano, como vim
  con `hidden`.

#### Vault y panel lateral (M2)

- Un clic en una nota del árbol la abre y deja el foco en el árbol, como en
  Zed; el doble clic pasa al editor. `space e` con el árbol a la vista le da
  el foco en vez de esconderlo. El cursor del árbol se ve con el acento del
  tema solo si el árbol tiene el foco, y la nota abierta se marca aparte.
- `metalmind carpeta/` abre la carpeta como vault; `metalmind archivo.md` abre el
  archivo en el vault de la carpeta que lo contiene, y `metalmind` sin argumentos,
  la carpeta de `METALMIND_VAULT`, el último vault abierto o el `vault:` de la
  config, en ese orden.
- Panel lateral con el árbol de carpetas del vault, que se actualiza solo
  cuando algo cambia en disco. Se muestra y se esconde con `space e` o con el
  botón de la barra de la izquierda, y recuerda por vault si estaba visible y
  qué carpetas estaban abiertas.
- En el árbol: `j k`, `gg G`, `h l`, `o` o `Enter` para abrir, `ctrl-v` y
  `ctrl-s` para abrir en un split, `/` para filtrar, `a` y `A` para crear una
  nota o una carpeta, `r` para renombrar, `m` para mover y `d` para mandar a
  la papelera del vault (`.trash/`), con confirmación. `ctrl-w h` y `ctrl-w l`
  pasan del árbol a los editores y de vuelta.
- Con el ratón: clic para seleccionar (y abrir o cerrar una carpeta), doble
  clic para abrir y clic derecho para un menú con lo mismo.
- Renombrar o mover un archivo abierto lleva su buffer al nombre nuevo, con
  sus cambios sin guardar. Borrar avisa si hay cambios sin guardar, que se
  pierden: los paneles de lo borrado se cierran.
- `:w ruta` y `:wq ruta` le dan archivo a un buffer sin nombre, como en vim; en
  uno con archivo, `:w ruta` escribe una copia. Nunca pisan un archivo (`E13`)
  ni escriben en uno abierto en otro buffer (`E139`).
- Las rutas relativas de `:e` y `:w` parten de la raíz del vault, no del
  directorio desde el que se lanzó Metalmind.
- El ancho del panel se ajusta con `panel.width` en `config.yaml` y sus colores
  con las claves `ui.panel_*` y `ui.menu_*` del tema.

#### Búsqueda de texto (M6)

- `space /` busca texto en todas las notas del vault, como el live grep de
  Telescope. Cada resultado es una línea con su contexto y lo que coincide
  resaltado, y a la derecha la nota y el número de línea. `Enter` abre la nota
  en esa línea, con el cursor en la coincidencia; `ctrl-v` y `ctrl-s`, en un
  split, como en `space f`.
- El patrón es un regex, y distingue mayúsculas solo si las lleva. Uno a medio
  escribir no borra los resultados: dice "patrón inválido" hasta que se arregla.
- Las notas abiertas con cambios sin guardar se buscan en lo que se ve, no en
  lo que hay en disco.
- La búsqueda corre en segundo plano y cada tecla cancela la anterior: la
  ventana nunca se traba, aunque el vault tenga 50 000 notas. Con más de
  10 000 líneas se corta y lo dice.
- También desde la paleta (`workspace: open text search`).

#### Configuración

- `config.yaml` en el directorio de configuración del usuario
  (XDG), con tema y fuente. Es YAML 1.2, así que también se puede escribir en
  JSON, y `yes`, `on` o `NO` nunca se leen como booleanos. Un archivo roto o
  con tipos equivocados se reemplaza por los defaults y el error dice la línea
  y la columna; una clave desconocida es solo un aviso. Detalle en
  [docs/config.md](docs/config.md).
- Temas `metalmind-oscuro` y `metalmind-claro`, y temas propios en `themes/*.yaml` que
  pueden extender a otro. `ui.pane_border`, `ui.status_unfocused_foreground`
  y `cursor.unfocused` pintan los paneles sin foco.

#### Release

- `metalmind --version` y `metalmind --help`.
- Entrada de escritorio para abrir archivos `text/markdown`, ícono y metainfo
  de AppStream.
- Binarios para Linux x86_64 y aarch64 en cada GitHub Release, y paquetes
  `metalmind-bin` y `metalmind-git` para el AUR.

[Unreleased]: https://github.com/alex-glez-felix/metalmind/commits/main
