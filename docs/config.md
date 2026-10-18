# Configuración

`nota` funciona sin configurar nada. Para cambiar algo, crea `config.yaml` en la carpeta de configuración:

| Sistema | Carpeta |
|---|---|
| Linux | `$XDG_CONFIG_HOME/nota/`, por defecto `~/.config/nota/` |
| macOS | `~/Library/Application Support/nota/` |
| Windows | `%APPDATA%\nota\` |

Si el archivo tiene un error, `nota` arranca igual con los valores por defecto y reporta el error con archivo, línea y columna. Una clave que no existe (por ejemplo un typo) no tumba el archivo: se avisa y el resto se aplica.

## `config.yaml`

Todas las claves son opcionales; estos son los valores por defecto:

```yaml
theme: nota-oscuro        # o nota-claro, o uno tuyo de themes/
# vault: ~/Notas          # vault que se abre si no pasas ninguno

font:
  # family: JetBrains Mono  # sin esto, la primera monoespaciada instalada
  size: 15                  # entre 4 y 200

editor:
  number: true            # el número de cada línea
  relativenumber: true    # la distancia de cada línea a la del cursor
  scrolloff: 0            # líneas visibles alrededor del cursor
  tabstop: 4              # ancho del tab, al menos 1
  expandtab: true         # Tab inserta espacios
  padding:                # espacio entre el borde del panel y el texto, en píxeles (0–255)
    horizontal: 12        # a la izquierda, antes de los números, y a la derecha
    vertical: 8           # arriba y abajo

panes:
  border_width: 2         # ancho de la línea entre paneles, en píxeles (0–255); el color es ui.pane_border

search:
  ignorecase: false
  smartcase: false        # con ignorecase, una mayúscula vuelve la búsqueda exacta
```

Los números de línea siguen a vim: con solo `number`, el número de cada línea; con solo `relativenumber`, la distancia al cursor; con los dos (el default), el modo híbrido: el número de la línea del cursor y la distancia en las demás. El margen crece con el número de líneas y resalta la del cursor (`ui.gutter_current`). Cada panel tiene los suyos: `:set nu`, `:set rnu`, `:set nonu`, `:set nornu`, `:set nu!` (o `invnu`) cambian solo el panel enfocado, y un split hereda los del panel que se divide. Por ejemplo, solo números absolutos:

```yaml
editor:
  relativenumber: false
```

La indentación marca qué clave va dentro de cuál: dos espacios por nivel, nunca tabs.

Hoy la app usa el tema, la fuente, los números de línea, `editor.padding`, `panes` y `scrolloff`; el resto de los ajustes se irán aplicando a medida que lleguen las funciones que los usan.

### En JSON

El archivo es YAML 1.2, que incluye a JSON: un `config.yaml` escrito en JSON puro, con llaves y comillas, funciona igual. Este equivale a un `config.yaml` con tema claro y tabs de 2:

```json
{
  "theme": "nota-claro",
  "font": { "family": "Iosevka", "size": 14 },
  "editor": { "tabstop": 2, "expandtab": true }
}
```

### Trampas de YAML que nota evita

nota lee YAML 1.2, no 1.1, y además usa el tipo de cada ajuste como esquema:

- **El problema de Noruega.** En YAML 1.1, `NO`, `no`, `yes`, `on`, `off`, `y` y `n` se leen como booleanos aunque quisieras texto: `country: NO` daría `false`. En nota, en un campo de texto son ese texto (`theme: NO` busca un tema llamado `NO`), y en un booleano solo valen `true` y `false`: `expandtab: on` es un error que dice dónde está, nunca un `true` silencioso.
- **Números que parecen otra cosa.** `0755` no es octal y `1.10` en un campo de texto no se vuelve `1.1`: si el campo es texto, se guarda tal como lo escribiste.
- **Claves repetidas.** Escribir dos veces la misma clave en el mismo nivel es un error, en vez de quedarse en silencio con la última.

Lo que sí sigue siendo YAML:

- `#` empieza un comentario, así que un color hex va entre comillas: `"#7aa2f7"`.
- `~` solo es nulo: `vault: ~` es "sin vault", pero `vault: ~/Notas` es la ruta.
- Un texto con `: ` o que empieza con `{`, `[`, `*`, `&`, `!`, `%`, `@` o `` ` `` va entre comillas.

## Temas

Un tema es una **paleta** con nombres y los **colores semánticos** que usa la interfaz. Los temas propios van en `themes/<nombre>.yaml` dentro de la carpeta de configuración y se eligen con `theme: <nombre>`.

Cada color semántico puede ser:

- un hex, entre comillas: `"#7aa2f7"`
- un nombre de la paleta: `blue`
- un mapa con opacidad: `{ color: blue, alpha: 0.5 }`

Con `extends` un tema parte de otro y solo cambia lo que define. Los colores heredados que nombran la paleta usan la paleta final, así que cambiar `blue` cambia todo lo que era azul:

```yaml
# ~/.config/nota/themes/mio.yaml
extends: nota-oscuro

palette:
  blue: "#82aaff"

ui:
  background: "#101216"
  selection: { color: blue, alpha: 0.25 }
```

Para heredar un color, no pongas la clave: una clave vacía o `null` es un error, no una herencia. Un tema sin `extends` tiene que definir todos los colores. Si un tema no carga (no existe, tiene un error, su cadena de `extends` es circular o le faltan colores), se usa `nota-oscuro` y se reporta el problema. Un tema tuyo con el nombre de uno embebido lo reemplaza, y puede extenderlo: `themes/nota-oscuro.yaml` con `extends: nota-oscuro` parte del embebido.

Las secciones y sus claves son las de los temas embebidos, que sirven de plantilla: [`nota-oscuro.yaml`](../crates/nota-config/themes/nota-oscuro.yaml) y [`nota-claro.yaml`](../crates/nota-config/themes/nota-claro.yaml).

- `ui`: `background`, `foreground`, `selection`, `search` (las coincidencias de la búsqueda), `search_current` (la coincidencia donde está el cursor), `current_line`, `gutter`, `gutter_foreground`, `gutter_current` (el número de la línea del cursor), `status_background`, `status_foreground`, `status_unfocused_foreground`, `status_border`, `pane_border`, `modified`, `error`, y los del finder y la paleta: `picker_background`, `picker_border`, `picker_selected` (la fila elegida), `picker_hover` (la fila bajo el ratón), `picker_match` (las letras que coinciden) y `picker_muted` (carpeta, atajo y texto de ayuda).
- `cursor`: `normal`, `insert`, `visual`, `replace`, `unfocused` (el cursor de un panel sin foco).
- `syntax`: `heading`, `bold`, `italic`, `link`, `tag`, `code`, `quote`, `list_marker`, `highlight`, `comment`, `frontmatter`, `markup`.
