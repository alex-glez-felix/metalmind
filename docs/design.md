# Diseño

Cómo se ve Metalmind y por qué. Las reglas de aquí valen para toda la UI nueva; lo que hoy no las cumple está en la [auditoría](design/auditoria.md#contra-los-principios).

## Referencias

Zed y el estilo de las apps de escritorio en tiling de Linux, minimalistas pero configurables.

## Principios

1. **El contenido manda, la interfaz se calla.** Densa y sobria: bordes de 1 px, sin sombras ni degradados, sin tarjetas redondeadas ni espacios grandes.
2. **Nativa, no web.** Sin animaciones al pasar el ratón, sin spinners, sin botones gordos. Todo responde al instante y nada brinca.
3. **El teclado es el camino corto y la app lo enseña.** Todo se puede con ratón, y cada acción con ratón muestra su tecla (menú contextual, tooltips, paleta). Esto aplica a toda la UI nueva desde ahora.
4. **Todo se puede configurar:** colores, fuentes, tamaños y espaciado salen del tema.
5. **Lo rápido es parte del diseño:** si algo es bonito pero lento, no entra.

## Descartado

- **Leer o seguir el tema de un entorno de escritorio específico.** Los temas son de la app; quien quiera igualarlos escribe su tema.

## Tokens

Todo color, tamaño y espacio de la UI sale del tema ([`docs/config.md`](config.md#temas)); el código de `metalmind-ui` no escribe ni un píxel ni un color fuera de su módulo de tokens (`crates/metalmind-ui/src/theme.rs` y sus submódulos en `src/theme/`), y un test lo vigila (`crates/metalmind-ui/tests/tokens.rs`, con sus excepciones justificadas).

Lo que hay hoy, con los valores de los temas de fábrica:

| Grupo del tema | Qué es | Valores de hoy |
|---|---|---|
| `palette` | Los colores con nombre de cada tema | ver los temas |
| `roles` | Los colores por su papel: `background`, `surface`, `raised`, `text`, `text_muted`, `text_faint`, `text_inactive`, `accent`, `border_subtle`, `border_strong`, `danger`, `warning` | de la paleta |
| `ui`, `cursor`, `syntax` | El color de cada parte de la interfaz, del cursor y del markdown | un rol, un nombre de la paleta o un hex |
| `spacing` | La escala de espaciado, base 4 | `xs` 4, `sm` 8, `md` 12, `lg` 16 |
| `radius` | Los radios | `sm` 2, `md` 4, `lg` 8 |
| `stroke` | Los anchos de línea | `border` 1, `caret` 2, `text_line` 1 |
| `text` | Tamaño (veces `font.size`) e interlineado de `editor`, `ui_md` y `ui_sm`; `math`, las fórmulas en veces `editor`; pesos `normal` y `strong` | todo 1.0 / 1.4; `math` 1.21; 400 y 700 |
| `headings` | Tamaño y peso de `h1`…`h6` | de 1.6 a 1.0 |
| `rows` | Alto de las filas del árbol y del menú (veces la fuente) y el aire de las del picker | 1.7, 1.7, 6 px |
| `layout` | Las medidas de cada parte: barra de estado, picker, panel, barra de botones, menú y fórmulas, y el ancho de lectura de una nota (`note_max_width`) | un nombre de `spacing` o píxeles; `note_max_width`, un entero de columnas de texto, de 20 a 1000 (80) |

De `config.yaml` y no del tema, porque son preferencias de quien usa la app y no del estilo: `font` (familia y tamaño base), `editor.padding`, `panes.border_width` y `panel.width`.

En el código, `Theme` (`metalmind-ui`) guarda los grupos del tema ya en tipos de GPUI y con los nombres del YAML: `syntax.heading` es `theme.syntax.heading` (`Hsla`), `layout.picker_gap` es `theme.layout.picker_gap` (`Pixels`) y `text.strong`, `theme.text.strong` (`FontWeight`). Los grupos de `metalmind-config` son genéricos en sus unidades y la UI los convierte una vez (`convert::<Gpui>()`). `Theme` solo tiene a mano lo que se calcula (las letras multiplicadas por `font.size`, el alto de las filas del árbol y del menú, el tamaño de los íconos) y lo que sale de `config.yaml`. Un token nuevo se escribe en los dos temas de fábrica y en su grupo (`color_group!` o `metric_group!` en `metalmind-config`), y la vista ya lo lee: `theme.rs` no cambia.

De dónde sale cada valor, qué se repite con otro número y las preguntas abiertas: [auditoría](design/auditoria.md).

## Componentes

Lo que decide cada componente queda por escribir: cada sección se llena cuando Alex decida su estilo.

### Árbol del panel lateral

*Por decidir.*

### Picker (`space f`, `space /`, paleta)

*Por decidir.*

### Línea de comandos (`:`)

Decidido el 2026-10-27 ([#193](https://github.com/alex-glez-felix/metalmind/issues/193)): un pop-up centrado arriba, igual que la paleta y el finder (los mismos tokens `picker_*`, el mismo borde y el mismo radio). Arriba lo escrito, tras un `:` tenue, que se parte en filas en vez de recortarse; debajo, solo si hay algo que ofrecer, la lista con las filas del picker: el historial con la línea vacía, los nombres de comando al escribir uno (con la acción que corre contra el borde derecho, donde la paleta pone el atajo) o el argumento. La fila elegida muestra la tecla que la acepta (`tab`), y un clic la acepta. `/` y `?` se quedan en la barra de estado, como en vim, para no tapar lo que resaltan. El porqué y las teclas, en [03](research/03-core-architecture.md) ("Línea de `:`").

### Menú contextual

*Por decidir.*

### Barra de estado

*Por decidir.* Lleva el modo, la nota, su indicador, el focus y la posición; los avisos van en toasts.

### Toasts de avisos

Arriba a la derecha, sobre los paneles y bajo un picker abierto, con el fondo y el radio del picker y el borde del menú: un ícono por nivel (`●` información, `▲` advertencia, `◆` error, cada uno con su color), el texto entero partido en líneas y `esc`, apagado, a la derecha. La información dura 4 s, las advertencias 8 s y los errores hasta que se cierran (un clic, `esc` en Normal); `:messages` los lista en el picker. Detalles en [03 §"Estado (avisos en toasts…)"](research/03-core-architecture.md).

### Margen de números (gutter)

*Por decidir.*

### Barra de botones del panel

*Por decidir.*

### Campos de texto y confirmaciones del panel

*Por decidir.*

## Proceso

- **Toda PR que cambie algo visible lleva capturas en la descripción** (regla de las PRs de UI desde el 2026-10-19). Todas van en la descripción de la PR, no solo en la rama:
  - en Xvfb, nunca en el display real;
  - en tema **oscuro y claro**, con ventanas de **1920x1080 y 1280x720**: las cuatro combinaciones;
  - con **contenido que estrese el layout**: notas de más de 1000 líneas, líneas más largas que el ancho (el editor las parte en filas con `editor.wrap`, el default, y sin él se corre a lo ancho como `nowrap` de vim: las dos se prueban), rutas y nombres largos en el árbol y en los pickers, multibyte y emoji, muchos resultados y listas vacías, y menús y tooltips abiertos cerca de los bordes de la ventana. Con contenido cómodo no se ven los bugs (así se pasó el texto encimado del picker);
  - si la PR arregla o cambia algo visual, capturas de **antes y después** y el diff de imagen, también en los dos tamaños y los dos temas.

  Sin capturas, la PR no se mergea.
- **`scripts/capturas.sh`** las toma todas: abre cada pantalla (editor con markdown y al final de la nota larga, árbol con rutas hondas, menú contextual en medio y junto al borde de abajo, split, línea de `:` con un comando largo, con los nombres que empiezan con lo escrito y con el historial, toasts de avisos y su historial, finder con resultados, filtrado, con una ruta larga y vacío, búsqueda de texto con resultados, vacía y con texto en japonés detrás de la elipsis, paleta, y la jumplist y las marks de una sesión anterior al abrir, tras `ctrl-o` y tras `'B`, en `capturas/saltos.rs`) en un Xvfb propio, sobre un vault de fixtures que estresa el layout (`crates/metalmind/tests/capturas/vault.rs`), en los dos temas y los dos tamaños, y deja los PNG en `target/capturas/`. Es el test `capturas` de `metalmind`; necesita lo mismo que el smoke ([testing](testing.md#correrlo-en-local)). Una pantalla nueva es una línea más en `SCENES` (`crates/metalmind/tests/capturas.rs`); una UI nueva que abra algo cerca de un borde (un tooltip, un menú) suma su escena ahí.
- **Antes y después**: el script corrido en la base deja el "antes"; en la rama, `scripts/capturas.sh target/despues --antes target/capturas` compara cada par, deja `diff-*.png` con lo que cambió en rojo, un `resumen.md` y `titulos.txt` con el título de la ventana en cada pantalla (sin gestor de ventanas, la captura no lo muestra), y sale con error si cambió algún píxel. Un refactor que no debería cambiar nada tiene que dar 0.
- Las capturas que van en la PR se suben a `docs/capturas/<tema-de-la-pr>/` en la rama y se enlazan por commit (`https://raw.githubusercontent.com/alex-glez-felix/metalmind/<commit>/docs/capturas/…`).
