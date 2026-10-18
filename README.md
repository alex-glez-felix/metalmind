# nota

> Nombre de trabajo. Se cambia cuando haya algo que valga la pena nombrar.

App de notas tipo Obsidian, **nativa, open source, Linux primero**, escrita en Rust.

Tres prioridades, en este orden cuando choquen:

1. **Performance**: arranque instantáneo, scroll fluido, vaults grandes sin despeinarse.
2. **UX**: pulida, sin fricción, que se sienta como herramienta y no como página web.
3. **Modo vim en todo**: edición modal en el buffer *y* en el resto de la UI (árbol de archivos, búsqueda, paleta de comandos, `:` comandos, leader keys).

Formato: **Markdown plano en carpetas**, legible y editable con cualquier otra herramienta. El eje de organización son los **tags**, no las carpetas: una nota puede pertenecer a varias cosas a la vez. Compatibilidad con vaults de Obsidian **no es requisito**: se adopta su sintaxis (`[[wikilinks]]`, `#tags`, frontmatter) solo donde sea la mejor opción, no por obligación.

## Estado

M0 hecho: `nota archivo.md` abre una ventana (GPUI) donde se edita con vim y se guarda con `:w` de forma atómica.

- `crates/nota-buffer`: texto, selecciones, undo.
- `crates/nota-vim`: motor vim puro, verificado contra `nvim --headless`, con una línea de comandos mínima.
- `crates/nota-vault`: el vault en disco: índice de archivos, watcher, operaciones de archivo y guardado atómico (M2, sin interfaz todavía).
- `crates/nota-search`: búsqueda difusa de rutas para el finder.
- `crates/nota-ui` y `crates/nota`: la vista de editor y el binario; los únicos que dependen de GPUI.

```sh
cargo run --release -p nota -- archivo.md
cargo nextest run --workspace   # incluye el oráculo contra nvim si `nvim` está en el PATH
cargo clippy --workspace --all-targets -- -D warnings
```

Las librerías de sistema que pide GPUI y cómo correrlo en Arch están en [Primeros pasos](docs/GETTING-STARTED.md#correr-nota).

## Investigación

- [01 — Panorama: qué existe ya](docs/research/01-landscape.md)
- [02 — Stack de GUI en Rust](docs/research/02-gui-stack.md)
- [03 — Arquitectura del núcleo](docs/research/03-core-architecture.md)
- [04 — Spike de GPUI: resultados](docs/research/04-spike-gpui.md)
- [05 — Tags: el eje de organización](docs/research/05-tags.md)
- [06 — Decisiones de fondo](docs/research/06-decisiones-de-fondo.md)
- [Primeros pasos](docs/GETTING-STARTED.md)
- [Cómo se hace un release](docs/release.md) y [CHANGELOG](CHANGELOG.md)
- [Ideas a futuro (plugins, temas…)](docs/IDEAS.md)

## Licencia

GPL-3.0-or-later. Ver `LICENSE`.
