# Configuración

`metalmind` funciona sin configurar nada. Para cambiar algo, crea `config.yaml` en la carpeta de configuración:

| Sistema | Carpeta |
|---|---|
| Linux | `$XDG_CONFIG_HOME/metalmind/`, por defecto `~/.config/metalmind/` |
| macOS | `~/Library/Application Support/metalmind/` |
| Windows | `%APPDATA%\metalmind\` |

Si el archivo tiene un error, `metalmind` arranca igual con los valores por defecto y reporta el error con archivo, línea y columna. Una clave que no existe (por ejemplo un typo) no tumba el archivo: se avisa y el resto se aplica.

## `config.yaml`

Todas las claves son opcionales; estos son los valores por defecto:

```yaml
theme: metalmind-oscuro        # o metalmind-claro, o uno tuyo de themes/
# vault: ~/Notas          # vault que se abre si no pasas ninguno, no hay METALMIND_VAULT y no hay uno anterior

font:
  # family: JetBrains Mono  # sin esto, la primera monoespaciada instalada
  size: 15                  # entre 4 y 200

editor:
  number: true            # el número de cada línea
  relativenumber: true    # la distancia de cada línea a la del cursor
  gutter_marks: true      # la letra de las marks a-z y A-Z, a la izquierda de los números
  scrolloff: 0            # líneas visibles alrededor del cursor
  tabstop: 4              # ancho del tab, al menos 1
  expandtab: true         # Tab inserta espacios
  padding:                # espacio entre el borde del panel y el texto, en píxeles (0–255)
    horizontal: 12        # a la izquierda, antes de los números, y a la derecha
    vertical: 8           # arriba y abajo
  autosave: true          # guardar solo las notas con cambios (ver abajo)
  autosave_delay_ms: 1000 # cuánto esperar sin editar antes de guardar (100–60000)

panes:
  border_width: 2         # ancho de la línea entre paneles, en píxeles (0–255); el color es ui.pane_border

panel:
  width: 260              # ancho del panel lateral (el árbol de carpetas), en píxeles (120–1200)

search:
  ignorecase: false
  smartcase: false        # con ignorecase, una mayúscula vuelve la búsqueda exacta

templates_folder: plantillas  # carpeta de plantillas, relativa al vault

daily:                    # la nota del día
  enabled: false
  folder: diario          # relativa al vault; "" es la raíz
  format: YYYY-MM-DD      # códigos de fecha de las plantillas; un / hace subcarpetas
  # template: diario      # una plantilla de templates_folder; sin ella, nace vacía

history:                  # cuántas versiones viejas de cada nota se guardan
  keep_all_hours: 24      # todas las de las últimas 24 horas
  hourly_days: 30         # luego, la última de cada hora hasta los 30 días
  daily_days: ~           # luego, la última de cada día; ~ es para siempre
  deleted_days: 90        # la historia de una nota borrada

markdown:                 # la edición de markdown en el editor
  continue_lists: true        # Enter, o y O en un ítem de lista abren el siguiente (y renumeran)
  indent_list_children: true  # >> y << en un ítem de lista mueven también sus hijos
```

Los números de línea siguen a vim: con solo `number`, el número de cada línea; con solo `relativenumber`, la distancia al cursor; con los dos (el default), el modo híbrido: el número de la línea del cursor y la distancia en las demás. El margen crece con el número de líneas y resalta la del cursor (`ui.gutter_current`). Cada panel tiene los suyos: `:set nu`, `:set rnu`, `:set nonu`, `:set nornu`, `:set nu!` (o `invnu`) cambian solo el panel enfocado, y un split hereda los del panel que se divide. Por ejemplo, solo números absolutos:

```yaml
editor:
  relativenumber: false
```

Con `gutter_marks`, el margen muestra a la izquierda de los números la letra de cada mark que cae en la línea, como marks.nvim o vim-signature: las `a`–`z` del buffer y las `A`–`Z` de este archivo; las especiales (`''`, `'<`, `'>`) no se muestran. Si una línea tiene varias, se ve la primera (las minúsculas antes que las mayúsculas) seguida de un `+`, y el tooltip al pasar el ratón las lista todas. La columna se reserva aunque no haya ninguna mark, así el texto no se mueve al poner la primera; con `gutter_marks: false` no ocupa nada. Se actualiza con `m`, con las ediciones (una mark sigue a su línea) y con `:delm a`, `:delm a-d` o `:delm!` (todas las `a`–`z` del buffer). El color de la letra es `ui.gutter_mark`.

Con `autosave` (el default), Metalmind guarda solo las notas que tienen archivo y cambios: cuando pasa `autosave_delay_ms` sin editar (moverse no cuenta), cuando un panel cambia de nota (`:e`, el árbol, el finder, `gd`, `ctrl-o`), cuando la ventana pierde el foco y antes de cerrarla. Un `[No Name]` no se autoguarda: `:w ruta` le da archivo. Así la protección de salida casi nunca salta: `:q` guarda antes y solo se niega si queda algo que no se pudo guardar. La barra de estado dice "guardado" o "sin guardar" junto al nombre, en lugar del `[+]` de vim; mientras escribes con pausas cortas sigue en "guardado", y pasa a "sin guardar" si los cambios llevan cinco veces el delay sin llegar al disco (o la nota no tiene archivo), así no cambia con cada guardado. Los colores son los del tema: `ui.status_saved`, `ui.status_unsaved` y `ui.status_changed_on_disk`.

Si el archivo cambió fuera de Metalmind (git, Syncthing, otro editor) mientras la nota tiene cambios, no se pisa: la barra avisa con el `W12` de vim y dice "cambió en disco", y lo decides tú: `:w!` escribe tus cambios encima y `:e!` descarta los tuyos y trae lo de disco. Lo mismo vale para `:w`, que se niega con `WARNING: The file has been changed since reading it!!!`. Si el archivo se borró fuera, tampoco se vuelve a crear solo.

Con `autosave: false`, guardar y salir son como en vim: nada se guarda sin `:w`, la barra muestra `[+]` y `:q` se niega con cambios sin guardar.

Las versiones del autoguardado no llenan el historial: mientras sigues editando queda una cada cinco minutos, la última de cada tramo, y la última al terminar; un `:w` siempre deja la suya.

La indentación marca qué clave va dentro de cuál: dos espacios por nivel, nunca tabs.

El historial guarda una versión de la nota cada vez que se guarda o cambia desde fuera, fuera del vault, en la carpeta de datos de cada vault: `$XDG_DATA_HOME/metalmind/vaults/` en Linux (por default `~/.local/share/metalmind/vaults/`). De cada hora o día se queda la última versión, con las horas y los días de la zona horaria del sistema; la última versión de una nota se guarda siempre, y la diaria es para siempre salvo que se ponga `daily_days`.

`tabstop` y `expandtab` son también el ancho y los caracteres con que indentan `>`, `<`, `:>` y `:<`: Metalmind no tiene un `shiftwidth` aparte. Con `expandtab: false`, la sangría se escribe con tabs y luego espacios, como vim.

Con `markdown`, `Enter` en Insert sobre `- algo`, `1. algo` o `- [ ] algo` abre otro ítem igual (el número siguiente, el checkbox sin marcar), renumera los de abajo y en un ítem vacío termina la lista; `o` y `O` hacen lo mismo. `>>` y `<<` sobre un ítem llevan consigo a sus hijos. Cada parte se apaga por separado: con `continue_lists: false`, `Enter` es solo un salto de línea; con `indent_list_children: false`, `>>` mueve solo la línea, como en vim.

Hoy la app usa el tema, la fuente, los números de línea, `gutter_marks`, `editor.padding`, `editor.autosave`, `editor.autosave_delay_ms`, `panes`, `panel`, `scrolloff`, `tabstop`, `expandtab`, `markdown`, `history`, `vault`, `templates_folder` y `daily` (ver [plantillas](plantillas.md)). El resto de los ajustes se irán aplicando a medida que lleguen las funciones que los usan.

Dentro de Metalmind, una ruta relativa en `:e` o `:w` parte de la raíz del vault, se haya lanzado Metalmind desde donde se haya lanzado: `:w ideas/hoy.md` siempre deja la nota en el vault. `metalmind` sin argumentos elige el vault en este orden: la carpeta de la variable de entorno `METALMIND_VAULT` (si está puesta tiene que ser una carpeta; si no, Metalmind avisa y no abre), el último vault que abriste y, si no hay ninguno (o ya no existe), el de `vault:`. Un argumento gana a todo eso: `METALMIND_VAULT=~/Notas metalmind` y `metalmind ~/Notas` abren lo mismo, pero `metalmind ~/otra/` abre `~/otra/` aunque la variable esté puesta. Lo que Metalmind recuerda de cada vault (si el panel lateral se ve y qué carpetas estaban abiertas) no va en `config.yaml` sino en la carpeta de estado (`$XDG_STATE_HOME/metalmind/`, por defecto `~/.local/state/metalmind/`): no es configuración y se puede borrar sin perder nada.

### En JSON

El archivo es YAML 1.2, que incluye a JSON: un `config.yaml` escrito en JSON puro, con llaves y comillas, funciona igual. Este equivale a un `config.yaml` con tema claro y tabs de 2:

```json
{
  "theme": "metalmind-claro",
  "font": { "family": "Iosevka", "size": 14 },
  "editor": { "tabstop": 2, "expandtab": true }
}
```

### Trampas de YAML que Metalmind evita

Metalmind lee YAML 1.2, no 1.1, y además usa el tipo de cada ajuste como esquema:

- **El problema de Noruega.** En YAML 1.1, `NO`, `no`, `yes`, `on`, `off`, `y` y `n` se leen como booleanos aunque quisieras texto: `country: NO` daría `false`. En Metalmind, en un campo de texto son ese texto (`theme: NO` busca un tema llamado `NO`), y en un booleano solo valen `true` y `false`: `expandtab: on` es un error que dice dónde está, nunca un `true` silencioso.
- **Números que parecen otra cosa.** `0755` no es octal y `1.10` en un campo de texto no se vuelve `1.1`: si el campo es texto, se guarda tal como lo escribiste.
- **Claves repetidas.** Escribir dos veces la misma clave en el mismo nivel es un error, en vez de quedarse en silencio con la última.

Lo que sí sigue siendo YAML:

- `#` empieza un comentario, así que un color hex va entre comillas: `"#7aa2f7"`.
- `~` solo es nulo: `vault: ~` es "sin vault", pero `vault: ~/Notas` es la ruta.
- Un texto con `: ` o que empieza con `{`, `[`, `*`, `&`, `!`, `%`, `@` o `` ` `` va entre comillas.

## Temas

Un tema es una **paleta** con nombres, los **roles** (cada color por su papel), los **colores semánticos** de cada parte de la interfaz y las **medidas**: espaciado, radios, anchos de línea, letra, filas y el tamaño de cada parte. Todo color y tamaño de la interfaz sale de aquí ([diseño](design.md#tokens)). Los temas propios van en `themes/<nombre>.yaml` dentro de la carpeta de configuración y se eligen con `theme: <nombre>`.

Cada color de los roles y de los grupos semánticos puede ser:

- un hex, entre comillas: `"#7aa2f7"`
- un nombre de la paleta: `blue`
- en `ui`, `cursor` y `syntax`, un rol: `accent`
- un mapa con opacidad: `{ color: accent, alpha: 0.5 }`

Con `extends` un tema parte de otro y solo cambia lo que define. Los colores heredados que nombran la paleta o un rol usan los finales, así que cambiar `blue` cambia todo lo que era azul y cambiar el rol `accent` cambia todo lo que lo usa:

```yaml
# ~/.config/metalmind/themes/mio.yaml
extends: metalmind-oscuro

palette:
  blue: "#82aaff"

roles:
  accent: green           # lo elegido, lo que coincide y las carpetas del panel

ui:
  background: "#101216"
  selection: { color: accent, alpha: 0.25 }

spacing:
  md: 10                  # todo lo que usa `md` (el padding del picker, por ejemplo)

radius:
  lg: 0                   # el picker con esquinas rectas
```

Para heredar un color, no pongas la clave: una clave vacía o `null` es un error, no una herencia. Un tema sin `extends` tiene que definir todos los colores y todas las medidas. Si un tema no carga (no existe, tiene un error, su cadena de `extends` es circular o le faltan claves), se usa `metalmind-oscuro` y se reporta el problema. Un tema tuyo con el nombre de uno embebido lo reemplaza, y puede extenderlo: `themes/metalmind-oscuro.yaml` con `extends: metalmind-oscuro` parte del embebido.

Las secciones y sus claves son las de los temas embebidos, que sirven de plantilla: [`metalmind-oscuro.yaml`](../crates/metalmind-config/themes/metalmind-oscuro.yaml) y [`metalmind-claro.yaml`](../crates/metalmind-config/themes/metalmind-claro.yaml).

- `roles`: los colores por su papel, que `ui`, `cursor` y `syntax` nombran como a la paleta: `background` (el editor), `surface` (paneles, barras y lo que flota), `raised` (hover y la línea del cursor), `text`, `text_muted` (atajos, detalles, encabezados), `text_faint` (números de línea, la sintaxis atenuada), `text_inactive` (lo que no tiene el foco), `accent`, `border_subtle`, `border_strong` (la línea entre paneles), `danger` y `warning`. La paleta no puede usar el nombre de un rol.
- `ui`: `background`, `foreground`, `selection`, `search` (las coincidencias de la búsqueda), `search_current` (la coincidencia donde está el cursor), `current_line`, `gutter`, `gutter_foreground`, `gutter_current` (el número de la línea del cursor), `gutter_mark` (la letra de una mark), `status_background`, `status_foreground`, `status_unfocused_foreground`, `status_border`, `pane_border`, `modified`, `status_saved`, `status_unsaved` y `status_changed_on_disk` (el indicador del autoguardado), `error`, y los del finder y la paleta: `picker_background`, `picker_border`, `picker_selected` (la fila elegida), `picker_hover` (la fila bajo el ratón), `picker_match` (las letras que coinciden), `picker_warning` (lo escrito que no encuentra nada, como un tag que no existe) y `picker_muted` (carpeta, atajo y texto de ayuda).
  - El panel lateral y su barra de botones: `panel_background`, `panel_foreground`, `panel_muted` (adjuntos, encabezado y botones), `panel_selection` (el fondo de la fila seleccionada con el foco en el panel), `panel_selection_unfocused` (su borde con el foco en otro lado), `panel_active` (la fila de la nota abierta en el panel de edición enfocado, y el botón de la vista que se ve), `panel_hover`, `panel_border`, `panel_folder` y `panel_note` (los íconos), `panel_match` (las letras que coinciden con el filtro).
  - Los menús contextuales y los tooltips: `menu_background`, `menu_border`, `menu_hover`.
- `cursor`: `normal`, `insert`, `visual`, `replace`, `unfocused` (el cursor de un panel sin foco).
- `syntax`: el markdown del editor. `markup` es la sintaxis (`**`, `#`, `[[`, `` ` ``, `>`), que se ve atenuada; `heading`, `bold`, `italic`, `link` (wikilinks y links a algo del vault, y los externos), `link_unresolved` (un link a una nota que no existe), `embed`, `tag`, `code` y `code_background` (código inline), `code_block_background` (el fondo de un bloque cercado), `quote`, `list_marker`, `checkbox` (`[ ]` y `[x]`), `task_done` (el texto de una tarea marcada), `highlight` (el fondo de `==resaltado==`), `comment` (`%% %%`), `frontmatter`, `frontmatter_key`, `block_id` (`^bloque`), `fence_info` (el lenguaje tras la cerca) y el resaltado de los bloques de código: `code_keyword`, `code_string`, `code_number`, `code_comment`; `math` es el LaTeX de una fórmula cuando se ve su fuente (con el cursor encima). La fórmula compuesta se pinta con `ui.foreground`, el color del texto, y una que no compone muestra su fuente en `ui.error`.
- `headings`: el tamaño y el peso de cada nivel, de `h1` a `h6`, como `{ size: 1.6, weight: 700 }`. `size` es cuántas veces la fuente del editor (de 0.5 a 4) y `weight` el grosor, de 100 a 900 de cien en cien (400 es normal y 700 negrita). Un heading más grande ocupa más alto: el scroll, `H`, `M`, `L`, `zt`, `zz`, `zb`, `ctrl-d` y `ctrl-u` lo cuentan. Como los colores, un tema con `extends` hereda los niveles que no define.

Las medidas, también heredables una por una:

- `spacing`: la escala de espaciado en píxeles, base 4: `xs` (4), `sm` (8), `md` (12) y `lg` (16). `rows` y `layout` nombran sus pasos (`picker_padding_x: md`) o dan píxeles (`status_padding_y: 3`); un paso heredado usa la escala final.
- `radius`: los radios en píxeles, `sm` (2, los íconos), `md` (4: menú, botones y campos) y `lg` (8, el picker).
- `stroke`: los anchos de línea en píxeles: `border` (bordes y separadores), `caret` (el cursor de barra) y `text_line` (subrayado y tachado).
- `text`: el tamaño de letra, en veces `font.size` (de 0.5 a 4), y el interlineado, en veces el tamaño (de 1 a 3), de `editor` (las notas), `ui_md` (la interfaz) y `ui_sm` (lo secundario: atajos, detalles y contadores), como `{ size: 1.0, line_height: 1.4 }`; `math`, el tamaño de las fórmulas en veces el de `editor` (1.21, como KaTeX, para que se vean del tamaño del texto que las rodea); y los pesos `normal` y `strong` (la negrita del markdown y las letras que coinciden en el picker).
- `rows`: el alto de una fila del árbol (`tree`, que es también el del encabezado del panel y sus campos) y del menú (`menu`), en veces la fuente de la interfaz, redondeado; y el aire arriba y abajo de la línea de texto de una fila del picker (`picker_padding`).
- `layout`: las medidas de cada parte, en píxeles o un paso de `spacing`: la barra de estado (`status_padding_x`, `status_padding_y`, `status_gap` y `status_message_max_width`, de 0 a 1: la fracción del espacio entre el modo y la posición que puede ocupar el mensaje; hasta ahí se lee entero y el nombre de la nota se recorta antes), el picker (`picker_width`, `picker_top`, `picker_rows`, que son filas, `picker_padding_x`, `picker_input_padding_y`, `picker_gap` y `picker_detail_share`, la fracción de la fila que se queda el detalle cuando el texto no cabe), la vista previa de un cambio de tags (`tag_change_detail_lines`, las líneas del cambio elegido que se ven enteras bajo la lista), el panel lateral (`panel_padding_x`, `panel_padding_y`, `panel_gap`, `panel_indent`, `panel_button_padding_x` e `icon_size`, en veces la fuente), su barra de botones (`rail_width`, `rail_button_size`), el menú contextual (`menu_width`, que crece si una fila no cabe, sin pasar del ancho de la ventana; `menu_padding` y `menu_item_padding_x`, a los lados del texto de cada entrada y entre su etiqueta y su tecla), las listas del autocompletado de `#` y `[[` (`completion_rows`, que son filas; usan los colores y medidas del menú), los tooltips (`tooltip_padding_x`, `tooltip_padding_y`), la columna de las marks en el margen (`gutter_mark_columns`, en columnas de texto, de 3 a 8: la letra, el `+` de las demás y al menos un espacio) y las fórmulas (`math_padding`, el margen alrededor de la imagen de una fórmula, y `math_block_margin`, el aire arriba y abajo de una de bloque).

El tamaño base de la letra (`font.size`), el margen del texto (`editor.padding`), el borde entre paneles (`panes.border_width`) y el ancho del panel (`panel.width`) siguen en `config.yaml`: son de quien usa la app, no del estilo.
