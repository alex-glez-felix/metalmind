# Instrucciones para la revisión de código

Revisa en **español**. Este repo es una app de notas nativa en Rust (GPUI) con modo vim. Las reglas completas están en `CLAUDE.md`; aquí va lo que importa al revisar.

## Qué no revisar

El formato (rustfmt), clippy con `-D warnings`, `unwrap_used`, `expect_used`, `todo`, `dbg_macro` y `allow_attributes_without_reason` ya los vigila el CI. No comentes sobre eso salvo que el CI no lo pueda ver.

## Qué sí revisar, por prioridad

1. **Bugs reales**: offsets de texto mal convertidos (el núcleo usa bytes; chars/columnas/UTF-16 solo en el borde de la UI), off-by-one en rangos, pánicos posibles con texto multibyte (acentos, emoji, CJK), errores que se tragan en vez de propagarse con `?`.
2. **Parches**: un `if` especial para el caso que falló, un `clone()` para callar al borrow checker, reintentos o `sleep` para tapar una carrera. Señálalos y di cuál sería el arreglo de la causa.
3. **Tests que no prueban nada**: tests que pasan aunque el código esté mal, sin aserciones sobre el comportamiento, o comportamiento nuevo sin test. Todo motion u operador de vim nuevo necesita su caso en la tabla verificada contra `nvim --headless`.
4. **Frontera del núcleo**: `nota-buffer`, `nota-vim`, `nota-vault` y el resto del núcleo no pueden importar GPUI. Solo `nota-ui` y el binario.
5. **Guardado**: siempre atómico (temporal en el mismo directorio, fsync, rename). Nunca reescribir el frontmatter YAML del usuario salvo el campo editado.
6. **UI**: ningún color ni tamaño hardcodeado fuera de `theme.rs`; todo sale de tokens del tema.
7. **Acciones con nombre**: toda acción del usuario es una acción en snake_case con espacio de nombres (`vim::next_word_start`, `editor::save`). Atajos, paleta y `:` solo la invocan.
8. **Diseño**: funciones que no caben en pantalla, archivos de más de ~400 líneas, módulos con más de una responsabilidad, código muerto.
9. **Comentarios**: cortos y que digan *por qué*, no *qué*. Un párrafo para justificar algo es señal de que hay que rediseñar.

Si las decisiones de `docs/research/` contradicen el cambio, dilo y cita el documento.

Sé concreto: archivo, línea, qué falla y con qué entrada. Si no estás seguro de que algo sea un bug, dilo así en vez de afirmarlo.

## Datos personales

El repo es público. Señala cualquier nombre de máquina, spec de hardware concreto, IP, ruta de un home o referencia a una persona en código, docs o mensajes; deben ir en términos genéricos.
