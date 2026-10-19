# CLAUDE.md

Reglas para agentes que trabajen en este repo.

- Metalmind es una GUI con navegación estilo vim, no una TUI: toda función tiene que poder usarse desde la UI (ratón, menús, paleta) además del teclado, y el usuario nunca debería necesitar editar archivos de config o de metadata a mano.
- Antes de proponer arquitectura, leer `docs/GETTING-STARTED.md` y el documento de `docs/research/` relevante. Las decisiones viven ahí; si cambias una, actualiza el documento en el mismo commit.
- Los documentos de `docs/research/` que digan "pendiente de revisión" no son decisiones: no implementes nada de ellos.
- Antes de agregar un crate, verificar que existe y su última versión: `curl -s https://crates.io/api/v1/crates/<nombre> | jq .crate.max_version`. No inventar crates.
- El núcleo (`metalmind-buffer`, `metalmind-syntax`, `metalmind-vim`, `metalmind-keymap`, `metalmind-vault`, `metalmind-search`, `metalmind-config`, `metalmind-template`; ver `docs/research/03-core-architecture.md` §6) **no depende de la GUI**. Solo `metalmind-ui` y el binario importan GPUI.
- Offsets de texto siempre en bytes; conversión a chars/columnas solo en el borde de la UI.
- Comportamiento vim: cada motion/operator nuevo lleva test de tabla. En caso de duda, el oráculo es `nvim --headless`.
- Nunca reescribir el YAML del frontmatter del usuario salvo el campo editado. Guardado siempre atómico.
- `cargo clippy -- -D warnings` y `cargo nextest run` en verde antes de cada commit.
- Commits en español, estilo conventional commits.
- Toda acción del usuario es una acción con nombre, en snake_case con espacio de nombres (`vim::next_word_start`, `editor::delete_line`); atajos, paleta y `:` solo las invocan (base para plugins, ver `docs/IDEAS.md`).
- Ningún color ni tamaño hardcodeado en la UI: todo sale de tokens del tema (`tests/tokens.rs` de `metalmind-ui` lo vigila).
- **UI nueva**: sigue los principios de `docs/design.md` (sobria y densa, nada brinca, cada acción con ratón muestra su tecla, todo del tema, rápido antes que bonito).
- **PR que cambia algo visible**: lleva en la descripción las capturas de `scripts/capturas.sh` en las cuatro combinaciones (tema oscuro y claro, 1920x1080 y 1280x720), con contenido que estresa el layout (notas de más de 1000 líneas, líneas y rutas largas, menús y tooltips junto a los bordes); si cambia algo visual, también antes, después y el diff de imagen en las cuatro. Sin capturas no se mergea; detalles en `docs/design.md`, "Proceso".
- Metalmind no tiene usuarios todavía: los cambios de formato, API o nombres de archivo son directos. Nada de compatibilidad hacia atrás, migraciones, avisos de formatos viejos ni alias "por si acaso". Esto se revisa cuando haya un release público.

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

## Diseño: red flags (A Philosophy of Software Design)

Antes de abrir una PR, revisa el cambio contra esta lista y di en la descripción cuáles revisaste. Si encuentras uno en código que ya existía, abre un issue con la etiqueta `diseño` en vez de arreglarlo de paso.

**Preferir módulos profundos**: interfaz chica, mucha funcionalidad escondida. `Vault::open(raíz)` es el modelo: una llamada, y adentro el walk paralelo, `.gitignore`, ocultos, symlinks y el orden del árbol.

- **Módulo poco profundo**: la interfaz cuesta casi lo mismo de entender que lo que hace. P. ej., un módulo de `metalmind-vault` que solo renombra funciones de `std::fs`.
- **Fuga de información**: una misma decisión vive en dos módulos y cambiarla obliga a tocar los dos. P. ej., `metalmind-ui` y `metalmind-vault` sabiendo cada uno cómo se escribe un tag en el frontmatter.
- **Descomposición temporal**: el código se parte por el orden en que pasan las cosas y no por lo que cada parte sabe. P. ej., leer, parsear y guardar una nota en tres módulos que comparten el formato del archivo.
- **Métodos o capas que solo pasan argumentos**: reciben algo solo para dárselo a otro sin decidir nada. P. ej., un método de `metalmind-ui` que reenvía a `metalmind-vim` los mismos parámetros.
- **La misma abstracción en dos capas**: cada capa ofrece algo distinto de la de abajo. P. ej., una `Selection` en `metalmind-ui` con los campos de la de `metalmind-buffer`.
- **Nombres vagos**: `data`, `info`, `manager`, `utils`, `handle`. `LinkIndex`, no `IndexData`.
- **Difícil de describir en una frase**: si el doc comment necesita "y además", hace dos cosas. P. ej., una vista del editor que pinta y además decide qué es un heading (eso es de `metalmind-syntax`).
- **Comentarios que repiten el código**: `// sube el count` encima de `count += 1`. Ver "Comentarios cortos o ninguno".
- **Errores que podrían dejar de existir**: antes de agregar un `Err` o un caso especial, buscar el diseño que lo defina fuera de existencia. P. ej., `VaultPath` no admite `..`, así que ninguna operación del vault comprueba si una ruta se sale de la raíz.

## Repo público

El repo es público. Commits, PRs, docs y comentarios no llevan datos personales: nada de nombres de máquinas, specs de hardware, IPs, rutas de un home, ni quién corrió qué. Se habla del proyecto, no de quien lo usa ("en una máquina con GPU NVIDIA", no "en la PC de X"). Los números de rendimiento sí van, con el tipo de máquina en términos genéricos.
