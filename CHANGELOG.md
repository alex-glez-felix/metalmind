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
- `:sort` con `!`, `i`, `n`, `u` y `/patrón/` (con `r`).
- `:normal` (`:norm`) teclea lo que sigue en la línea del cursor o en cada
  línea del rango.
- `:g/patrón/comando`, `:g!` y `:v` corren `d`, `s`, `m0`, `t.`, `normal` o
  cualquier otro comando en cada línea donde está (o no) el patrón.
- `@:` repite la última línea de `:`, con count.
- Cada comando es un solo paso de `u`, también dentro de una macro.
- Los errores de vim se muestran con su número y su texto original
  (`E492: Not an editor command`), para buscarlos en `:help`.

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
  (relativo al directorio de trabajo), `:e` sin ruta relee el archivo y `:e!`
  descarta los cambios.
- Splits: `ctrl-w v` y `:vs` a la derecha, `ctrl-w s` y `:sp` abajo,
  `ctrl-w h j k l` y `ctrl-w w` mueven el foco, `ctrl-w q` y `:q` cierran el
  panel y `ctrl-w o` y `:on` dejan solo el actual. Dos paneles pueden mostrar
  el mismo archivo con cursores independientes.
- Como vim sin `hidden`, `:e` y `:q` se niegan si se perderían cambios, salvo
  que otro panel siga mostrando ese archivo; cerrar la ventana revisa todos
  los archivos abiertos.

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
