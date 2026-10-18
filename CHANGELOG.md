# Changelog

Los cambios que importan a quien usa nota. El formato sigue
[Keep a Changelog](https://keepachangelog.com/es-ES/1.1.0/) y las versiones,
[SemVer](https://semver.org/lang/es/). Cómo se publica una versión:
[docs/release.md](docs/release.md).

## [Unreleased]

### Agregado

#### Editor (M0)

- `nota archivo.md` abre el archivo en una ventana nativa (GPUI). Si no existe,
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
  ninguna, nota lo dice y no abre.
- Sin sesión gráfica (ni Wayland ni X11) o sin driver de Vulkan ni OpenGL,
  nota dice qué falta y sale con error en vez de quedarse corriendo sin
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

#### Buffers y paneles (M2)

- Varios archivos abiertos a la vez. `:e ruta` abre otro en el panel activo
  (relativo a la raíz del vault), `:e` sin ruta relee el archivo y `:e!`
  descarta los cambios.
- Splits: `ctrl-w v` y `:vs` a la derecha, `ctrl-w s` y `:sp` abajo,
  `ctrl-w h j k l` y `ctrl-w w` mueven el foco, `ctrl-w q` y `:q` cierran el
  panel y `ctrl-w o` y `:on` dejan solo el actual. Dos paneles pueden mostrar
  el mismo archivo con cursores independientes.
- Como vim sin `hidden`, `:e` y `:q` se niegan si se perderían cambios, salvo
  que otro panel siga mostrando ese archivo; cerrar la ventana revisa todos
  los archivos abiertos.

#### Vault y panel lateral (M2)

- `nota carpeta/` abre la carpeta como vault; `nota archivo.md` abre el
  archivo en el vault de la carpeta que lo contiene, y `nota` sin argumentos,
  el último vault abierto (o el `vault:` de la config).
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
  directorio desde el que se lanzó nota.
- El ancho del panel se ajusta con `panel.width` en `config.yaml` y sus colores
  con las claves `ui.panel_*` y `ui.menu_*` del tema.

#### Configuración

- `config.yaml` en el directorio de configuración del usuario
  (XDG), con tema y fuente. Es YAML 1.2, así que también se puede escribir en
  JSON, y `yes`, `on` o `NO` nunca se leen como booleanos. Un archivo roto o
  con tipos equivocados se reemplaza por los defaults y el error dice la línea
  y la columna; una clave desconocida es solo un aviso. Detalle en
  [docs/config.md](docs/config.md).
- Temas `nota-oscuro` y `nota-claro`, y temas propios en `themes/*.yaml` que
  pueden extender a otro. `ui.pane_border`, `ui.status_unfocused_foreground`
  y `cursor.unfocused` pintan los paneles sin foco.

#### Release

- `nota --version` y `nota --help`.
- Entrada de escritorio para abrir archivos `text/markdown`, ícono y metainfo
  de AppStream.
- Binarios para Linux x86_64 y aarch64 en cada GitHub Release, y paquetes
  `nota-bin` y `nota-git` para el AUR.

[Unreleased]: https://github.com/alex-glez-felix/nota/commits/main
