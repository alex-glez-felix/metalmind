# CLAUDE.md

Reglas para agentes que trabajen en este repo.

- Antes de proponer arquitectura, leer `docs/GETTING-STARTED.md` y el documento de `docs/research/` relevante. Las decisiones viven ahí; si cambias una, actualiza el documento en el mismo commit.
- Antes de agregar un crate, verificar que existe y su última versión: `curl -s https://crates.io/api/v1/crates/<nombre> | jq .crate.max_version`. No inventar crates.
- El núcleo (`nota-core`, `nota-vim`, `nota-vault`, `nota-markdown`) **no depende de la GUI**. Solo `nota-ui` y el binario importan GPUI.
- Offsets de texto siempre en bytes; conversión a chars/columnas solo en el borde de la UI.
- Comportamiento vim: cada motion/operator nuevo lleva test de tabla. En caso de duda, el oráculo es `nvim --headless`.
- Nunca reescribir el YAML del frontmatter del usuario salvo el campo editado. Guardado siempre atómico.
- `cargo clippy -- -D warnings` y `cargo nextest run` en verde antes de cada commit.
- Commits en español, estilo conventional commits.
- Toda acción del usuario es una acción con nombre; atajos, paleta y `:` solo las invocan (base para plugins, ver `docs/IDEAS.md`).
- Ningún color ni tamaño hardcodeado en la UI: todo sale de tokens del tema.
