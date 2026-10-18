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
- Línea de comandos con `:w`, `:q`, `:q!` y `:wq`, con sus abreviaturas. `:q`
  y el cierre desde el gestor de ventanas se niegan si hay cambios sin
  guardar.
- Barra de estado con el modo, `[+]` si hay cambios y la línea de comandos. El
  `[+]` se quita al deshacer hasta lo guardado, como en vim.
- Portapapeles con `ctrl-shift-c` y `ctrl-shift-v`, y selección primaria con
  el ratón y el clic medio.
- La fuente es la primera monoespaciada instalada de una lista; si no hay
  ninguna, nota lo dice y no abre.

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
- Cada caso de la tabla de vim (488) se verifica contra Neovim 0.12.6.

#### Release

- `nota --version` y `nota --help`.
- Entrada de escritorio para abrir archivos `text/markdown`, ícono y metainfo
  de AppStream.
- Binarios para Linux x86_64 y aarch64 en cada GitHub Release, y paquetes
  `nota-bin` y `nota-git` para el AUR.

[Unreleased]: https://github.com/alex-glez-felix/nota/commits/main
