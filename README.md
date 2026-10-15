# nota

> Nombre de trabajo. Se cambia cuando haya algo que valga la pena nombrar.

App de notas tipo Obsidian, **nativa, open source, Linux primero**, escrita en Rust.

Tres prioridades, en este orden cuando choquen:

1. **Performance**: arranque instantáneo, scroll fluido, vaults grandes sin despeinarse.
2. **UX**: pulida, sin fricción, que se sienta como herramienta y no como página web.
3. **Modo vim en todo**: edición modal en el buffer *y* en el resto de la UI (árbol de archivos, búsqueda, paleta de comandos, `:` comandos, leader keys).

Restricción de compatibilidad: abrir un vault de Obsidian existente tal cual (`.md` planos, `[[wikilinks]]`, `#tags`, frontmatter YAML, embeds `![[...]]`).

## Estado

Fase de investigación. Todavía no hay código.

## Investigación

- [01 — Panorama: qué existe ya](docs/research/01-landscape.md)
- [02 — Stack de GUI en Rust](docs/research/02-gui-stack.md)
- [03 — Arquitectura del núcleo](docs/research/03-core-architecture.md)
- [Primeros pasos](docs/GETTING-STARTED.md)
