# CLAUDE.md

Reglas para agentes que trabajen en este repo.

- Antes de proponer arquitectura, leer `docs/GETTING-STARTED.md` y el documento de `docs/research/` relevante. Las decisiones viven ahí; si cambias una, actualiza el documento en el mismo commit.
- Antes de agregar un crate, verificar que existe y su última versión: `curl -s https://crates.io/api/v1/crates/<nombre> | jq .crate.max_version`. No inventar crates.
- El núcleo (`nota-buffer`, `nota-syntax`, `nota-vim`, `nota-keymap`, `nota-vault`, `nota-search`, `nota-config`; ver `docs/research/03-core-architecture.md` §6) **no depende de la GUI**. Solo `nota-ui` y el binario importan GPUI.
- Offsets de texto siempre en bytes; conversión a chars/columnas solo en el borde de la UI.
- Comportamiento vim: cada motion/operator nuevo lleva test de tabla. En caso de duda, el oráculo es `nvim --headless`.
- Nunca reescribir el YAML del frontmatter del usuario salvo el campo editado. Guardado siempre atómico.
- `cargo clippy -- -D warnings` y `cargo nextest run` en verde antes de cada commit.
- Commits en español, estilo conventional commits.
- Toda acción del usuario es una acción con nombre, en snake_case con espacio de nombres (`vim::next_word_start`, `editor::delete_line`); atajos, paleta y `:` solo las invocan (base para plugins, ver `docs/IDEAS.md`).
- Ningún color ni tamaño hardcodeado en la UI: todo sale de tokens del tema.

## Calidad de código

Que esté hecho con IA no es excusa. Se escribe como si lo fuera a revisar alguien exigente, porque así es.

- **Nada de parches.** Si algo falla, se arregla la causa, no el síntoma. No vale meter un `if` especial para el caso que rompió, ni un `sleep`/reintento para tapar una carrera, ni un `clone()` para callar al borrow checker sin entender por qué se queja. Si el arreglo correcto es grande, se dice y se propone; no se mete el atajo "por mientras".
- **Comentarios cortos o ninguno.** El código se explica con nombres y tipos. Un comentario dice *por qué*, nunca *qué*, y cabe en una o dos líneas. Si hace falta un párrafo para justificar algo, el problema es el código: se rediseña.
- **Sin silenciar al compilador.** Nada de `#[allow(...)]` para esconder un warning. Si de verdad aplica, lleva `reason = "..."` en una línea.
- **Sin `unwrap()`/`expect()` en código de producción.** Los errores se propagan con `?` y tipos propios. En tests sí se permiten.
- **Sin código muerto ni `TODO` huérfanos.** Lo que no se usa se borra; un pendiente va en un issue, no en un comentario.
- **Funciones chicas y módulos con una sola responsabilidad.** Si un archivo pasa de ~400 líneas o una función no cabe en pantalla, se parte.
- **Nada se da por terminado sin test** que demuestre el comportamiento.

Al crear el workspace, estas reglas se vuelven automáticas en `Cargo.toml` (`[workspace.lints.clippy]`): `unwrap_used`, `expect_used`, `todo`, `dbg_macro`, `allow_attributes_without_reason` en `deny`. Lo que la máquina puede revisar no depende de la buena voluntad del agente.
