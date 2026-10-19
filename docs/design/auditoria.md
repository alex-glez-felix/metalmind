# Auditoría de la UI

> Hecha el 2026-10-19 sobre `main` en `4d59796`, antes de pasar la UI a tokens. Las rutas `archivo:línea` son de ese commit y de `crates/metalmind-ui/src/` salvo que digan otra cosa. Las medidas son con la config por defecto (fuente de 15 px). Las capturas salen de `scripts/capturas.sh` ([diseño, "Proceso"](../design.md#proceso)) y las de esta auditoría están en [`docs/capturas/tokens/`](../capturas/tokens/). Solo hechos: lo que hay que decidir está en las [preguntas](#preguntas-de-diseño-para-alex).

## Qué se miró

El editor con markdown (también al final de una nota de más de 1000 líneas), el árbol y su encabezado, la barra de botones del panel, los campos y confirmaciones del panel, el menú contextual, el picker (`space f`, `space /` y la paleta), la barra de estado y el margen de números (gutter), en los dos temas de fábrica y en ventanas de 1920x1080 y 1280x720, con el vault de fixtures de las capturas: nombres y rutas largos, multibyte y emoji, líneas más largas que la ventana, una nota de más de 1000 líneas, 120 notas que coinciden con una búsqueda, una carpeta vacía y el menú abierto junto al borde de abajo. El editor no parte las líneas largas (como `nowrap` de vim): las corta en el borde. Cerca del borde, el menú se corre hacia arriba y entra entero.

## Valores sueltos

Antes de esta PR, los colores ya salían del tema y casi todas las medidas vivían juntas en `theme.rs`, pero escritas en el código: no se podían cambiar desde el tema. Las demás estaban repartidas en las vistas.

### Medidas escritas en `theme.rs`

| Valor | Dónde | Qué es |
|---|---|---|
| 1.4 | `theme.rs:250` | Interlineado de todo: editor e interfaz |
| 2 px | `theme.rs:251` | Ancho del cursor de barra y del de los campos |
| 1 px | `theme.rs:252` | Subrayado del texto provisional (IME) |
| 1 px | `theme.rs:88` | Tachado |
| 1 px | `theme.rs:253` | Todos los bordes y separadores |
| 10 / 3 px | `theme.rs:257-258` | Padding horizontal / vertical de la barra de estado |
| 16 px | `theme.rs:259` | Separación entre campos de la barra de estado (también usada en el picker, abajo) |
| 640 px | `theme.rs:260` | Ancho del picker |
| 64 px | `theme.rs:261` | Del borde de arriba de la ventana al picker |
| 12 filas | `theme.rs:262` | Filas visibles del picker |
| 12 / 6 px | `theme.rs:263-264` | Padding horizontal / vertical de una fila del picker |
| 8 px | `theme.rs:265` | Radio del picker |
| fuente × 1.7, redondeado (26 px) | `theme.rs:267` | Alto de fila del árbol (y del menú y el encabezado, abajo) |
| 14 px | `theme.rs:268` | Sangría por nivel del árbol |
| 8 / 6 px | `theme.rs:269-270` | Padding horizontal / vertical del panel |
| 6 px | `theme.rs:271` | Separación en el panel: ícono y nombre, botones |
| fuente × 0.8, redondeado (12 px) | `theme.rs:272` | Tamaño de los íconos del panel |
| 36 / 28 px | `theme.rs:273-274` | Ancho de la barra de botones / lado de cada botón |
| 4 px | `theme.rs:275` | Radio de menú, entradas del menú, botones y campos |
| 240 px | `theme.rs:276` | Ancho del menú contextual |
| 4 px | `theme.rs:277` | Padding del menú (también usado en los botones del panel, abajo) |

### Medidas y estilos escritos en las vistas

| Valor | Dónde | Qué es |
|---|---|---|
| línea × 2 + padding × 2 | `picker/view/render.rs:18` | Alto de fila del picker: 21 + 2 × 6 = 33 px |
| padding × 2 | `picker/view/render.rs:73` | Padding vertical del input del picker: 12 px, el doble que una fila |
| 90 % de la ventana | `picker/view/render.rs:40` | Ancho máximo del picker |
| `FontWeight::BOLD` | `picker/view/render.rs:150` | Las letras que coinciden en el picker van en negrita |
| `FontWeight::NORMAL` / `BOLD` | `editor/text.rs:18`, `editor/text.rs:45-46` | Peso del texto normal y de la negrita del markdown |
| ícono × 0.75 | `sidebar/icons.rs:21`, `sidebar/icons.rs:32` | El lado corto de las formas de carpeta y nota |
| radio × 0.5 | `sidebar/icons.rs:22`, `sidebar/icons.rs:34` | Radio de los íconos: 2 px |
| fuente (15 px) | `sidebar/prompt.rs:78` | Alto del cursor de los campos del panel; el del picker mide una línea, 21 px (`picker/view/render.rs:67`) |

### Colores que no salen del tema

| Valor | Dónde | Qué es |
|---|---|---|
| `transparent_black()` | `sidebar/render.rs:247` | El borde de una fila del árbol sin marcar: transparente, para que la fila marcada no cambie de tamaño |

Las opacidades están todas en los temas (`{ color, alpha }`), ninguna en el código. No hay sombras ni degradados en ningún lado.

### Medidas que ya eran configurables

En `config.yaml`, no en el tema (`crates/metalmind-config/src/settings.rs`): el tamaño de la fuente, 15 px (`:180`); el ancho del borde entre paneles, 2 px (`:200`); el ancho del panel lateral, 260 px (`:207`); el margen del texto del editor, 12 px a los lados y 8 arriba y abajo (`:215-216`). El ancho del margen de números sigue a `numberwidth` de vim (`workspace/line_numbers.rs:43`): columnas, no píxeles.

## Incoherencias

El mismo concepto con valores distintos según la parte de la interfaz:

| Concepto | Árbol y panel | Menú | Picker y paleta | Barra de estado | Editor y gutter |
|---|---|---|---|---|---|
| Alto de fila | 26 px (fuente × 1.7) | 26 px, la misma medida que el árbol (`sidebar/menu.rs:187`) | 33 px (línea + 2 × 6); el input, 45 px | 28 px (línea + 2 × 3 + borde) y crece con el mensaje | una línea, 21 px |
| Padding horizontal | 8 px | 8 px dentro de un marco de 4 | 12 px | 10 px | 12 px (config) |
| Padding vertical | 6 px | 4 px | 6 px las filas, 12 el input | 3 px | 8 px (config) |
| Separación | 6 px | — | 16 px, la de la barra de estado (`picker/view/render.rs:90`, `:163`) | 16 px | — |
| Radio | 4 px botones y campos; 2 px íconos | 4 px el marco y cada entrada | 8 px | 0 | 0 |
| Borde | 1 px | 1 px | 1 px | 1 px arriba | 2 px entre paneles (config) |
| Letra | 15 px, interlineado 1.4 | igual | igual | igual | igual; los headings, de 1 a 1.6 veces |
| Fondo al pasar el ratón | `bg_highlight` (opaco) | azul al 25 % (oscuro) / 15 % (claro) | texto al 6 % / 5 % | — | — |
| Fila elegida | azul al 30 % / 20 % | la misma que el hover del menú | azul al 20 % / 12 % | — | selección de Visual: azul al 30 % / 20 % |
| Letras que coinciden | naranja, peso normal | — | azul, negrita | — | — |
| Borde del flotante | — | `comment` en oscuro, `border` en claro | `border` | — | — |
| Fondo del flotante | — | `bg_highlight` en oscuro, `bg` en claro: en el tema claro el menú tiene el fondo del editor y no el de los paneles | `bg_dark` | — | — |

Otras:

- **Medidas prestadas.** Los botones del encabezado del panel usan el padding del menú (`sidebar/render.rs:292`); el picker usa la separación de la barra de estado (`picker/view/render.rs:90`, `:163`); el formulario del panel usa su padding vertical como separación (`sidebar/prompt.rs:20`, `:94`); el menú usa el alto de fila y el padding del árbol (`sidebar/menu.rs:186-187`). Con esta PR cada una tiene su token, con el mismo valor. Cambiar una de esas medidas cambiaba otra parte sin quererlo.
- **Dos formas de medir una fila.** Árbol y menú, en veces la fuente redondeado; picker, una línea más padding. Con otra fuente no escalan igual.
- **Fuera de la escala de 4.** El padding vertical de la barra de estado (3 px), el del panel y las filas del picker y la separación del panel (6 px), el padding horizontal de la barra de estado (10 px), la sangría del árbol (14 px) y el radio de los íconos (2 px) no son múltiplos de 4. El resto sí: 4, 8, 12, 16, 28, 36, 64, 240 y 640.
- **Un solo tamaño de letra.** El título del encabezado, los botones, los atajos, los detalles del picker y el texto de las notas miden lo mismo; lo secundario se distingue solo por el color (`text_muted`).
- **El texto secundario tiene tres nombres** con el mismo valor en los dos temas: `picker_muted`, `panel_muted` y `status_foreground` (`fg_dim`).
- **Cursor de los campos.** En el picker mide una línea (21 px) y en el panel la fuente (15 px).

## Problemas visibles conocidos

- **El encabezado del árbol se ve apretado** (`sidebar/render.rs:103-127`; [captura](../capturas/tokens/editor-oscuro-1280.png)). `Carpetas +nota +carpeta T` va en una fila de 26 px con 8 px a los lados: el título y los tres botones casi llenan los 260 px del panel. Entre botón y botón quedan 14 px (6 de separación y 4 de padding de cada lado) y no hay borde, fondo ni otro tamaño de letra que los distinga: título y botones van en el mismo color (`panel_muted`) y tamaño, y se leen como una sola línea de texto. `T` es la tecla de la acción y no dice qué hace.
- **En el menú se enciman la etiqueta y el atajo** ([captura](../capturas/tokens/menu-oscuro-1280.png)). Con 240 px de ancho, "Abrir a la derecha" y `ctrl-v` quedan pegados (`Abrir a la derechactrl-v`): la fila reparte el espacio con `justify_between` y sin separación mínima (`sidebar/menu.rs:182-200`).
- **El texto del picker se encima** ([finder](../capturas/tokens/finder-oscuro-1280.png), [`space /`](../capturas/tokens/buscar-oscuro-1280.png)). Un nombre más largo que el picker parte en dos líneas y la segunda se monta sobre la fila de abajo, o queda cortada a media altura si es el único resultado (`finder-ruta-larga-*` de `scripts/capturas.sh`); en `space /`, con líneas largas, el texto de cada fila se encima entero. Lo arregla la PR #85 (`fix/picker-layout`).
- **La barra de estado crece con un mensaje largo** ([split](../capturas/tokens/split-oscuro-1280.png)). El mensaje parte en varias líneas (`editor/status_bar.rs:72`) y la barra le quita alto al editor: al abrir un split en 1280x720, el mensaje de la nota abierta llena el panel nuevo.
- **El picker cambia de alto mientras se escribe** (`picker/view/render.rs:20`, `:51`): mide las filas que hay, de 1 a 12. Ver [finder con resultados](../capturas/tokens/finder-oscuro-1280.png) y el finder vacío de `scripts/capturas.sh` (`finder-vacio-*`).

## Contra los principios

Lo que hoy no cumple los [principios](../design.md#principios). Solo los hechos; ninguno se arregla en esta PR.

1. **El contenido manda, la interfaz se calla.**
   - El picker es una tarjeta con esquinas de 8 px (`picker/view/render.rs:47`); menú, botones y campos tienen 4 px (`sidebar/menu.rs:140`, `:189`, `sidebar/render.rs:293`, `sidebar/rail.rs:58`, `sidebar/prompt.rs:71`).
   - El picker queda a 64 px del borde de arriba (`workspace_view/overlay.rs:63`) y separa el texto de su detalle con 16 px (`picker/view/render.rs:163`).
   - Sin sombras ni degradados: se cumple.
2. **Nativa, no web: nada brinca.**
   - El picker cambia de alto con cada tecla que cambia la cantidad de resultados (arriba).
   - La barra de estado cambia de alto con el largo del mensaje (arriba).
   - Abrir un campo del panel (filtrar, crear, renombrar, mover, confirmar un borrado) lo mete arriba del árbol y empuja las filas hacia abajo (`sidebar/render.rs:47`); un error del panel se agrega abajo (`sidebar/render.rs:49-55`).
   - No hay animaciones ni spinners: se cumple. El hover cambia el fondo al instante.
3. **Cada acción con ratón muestra su tecla.**
   - Los botones `+nota` y `+carpeta` no muestran su tecla (`a`, `A`) (`sidebar/render.rs:113-114`); `T` muestra solo la tecla y no qué hace (`:116`).
   - Los dos botones de la barra del panel no muestran `space e` ni `space t`, ni tienen tooltip (`sidebar/rail.rs:29-42`).
   - No hay tooltips en ninguna parte de la UI.
   - El picker no muestra que `ctrl-v` y `ctrl-s` abren en un split, ni qué tecla elige o cierra.
   - El menú contextual, la paleta y la confirmación de borrado (`Borrar (y)`, `Cancelar (n)`) sí muestran su tecla.
4. **Todo se puede configurar.** Antes de esta PR, las medidas de la tabla de `theme.rs` y de las vistas, los pesos y la tipografía de la interfaz no salían del tema. Con esta PR salen todas; quedan fijas solo las [excepciones justificadas](../../crates/metalmind-ui/tests/tokens.rs).
5. **Lo rápido es parte del diseño.** Nada que señalar desde la UI; los números están en [`performance.md`](../performance.md).

## Preguntas de diseño para Alex

1. **Escala.** ¿Las medidas fuera de la escala de 4 (3, 6, 10 y 14 px, y el radio de 2) pasan a un paso de la escala? ¿A cuál?
2. **Alto de fila.** ¿Árbol, menú y picker tienen el mismo alto? ¿Una fila se mide en veces la fuente o como línea más aire?
3. **Densidad del picker.** ¿Filas de 33 px y un input de 45 px, o más densas como las del árbol?
4. **Radios.** ¿El picker deja los 8 px? ¿Menú, botones y campos llevan radio, o esquinas rectas como los paneles?
5. **Posición del picker.** ¿A 64 px del borde, pegado arriba o centrado? ¿Un ancho fijo de 640 px con tope del 90 %, o relativo a la ventana?
6. **Alto fijo.** ¿El picker tiene siempre el alto de 12 filas, aunque haya menos resultados? ¿La barra de estado es siempre de una línea y el mensaje largo se recorta?
7. **Campos del panel.** ¿Filtrar, crear y confirmar se abren sin empujar el árbol (encima, o en la barra de estado como la línea de comandos)?
8. **Encabezado del árbol.** ¿Qué lleva: título, botones con ícono, botones con texto? ¿Cómo muestran su tecla (`a`, `A`, `T`)?
9. **Teclas visibles.** ¿Tooltips con la tecla en los botones del panel y de la barra? ¿El picker muestra sus teclas (`enter`, `ctrl-v`, `ctrl-s`, `esc`), y dónde?
10. **Tipografía de la interfaz.** ¿La interfaz usa el mismo tamaño que las notas, o uno menor? ¿Lo secundario (atajos, detalles, contadores) va más chico (`text.ui_sm`) o solo más tenue? ¿La interfaz usa la misma familia monoespaciada que el editor?
11. **Hover y elegido.** ¿Un solo color para pasar el ratón en árbol, menú y picker? ¿Uno solo para la fila elegida? Hoy son tres de cada uno.
12. **Coincidencias.** ¿Las letras que coinciden van igual en el árbol y en el picker (color y peso)?
13. **Menú en el tema claro.** ¿El menú tiene el fondo del editor (como hoy en claro) o uno levantado (como en oscuro)? ¿Su borde es el tenue o el fuerte?
14. **Borde entre paneles.** ¿2 px (hoy) o 1 px como el resto de los bordes?
