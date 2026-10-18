# Configuración

`nota` funciona sin configurar nada. Para cambiar algo, crea `config.toml` en la carpeta de configuración:

| Sistema | Carpeta |
|---|---|
| Linux | `$XDG_CONFIG_HOME/nota/`, por defecto `~/.config/nota/` |
| macOS | `~/Library/Application Support/nota/` |
| Windows | `%APPDATA%\nota\` |

Si el archivo tiene un error, `nota` arranca igual con los valores por defecto y reporta el error con archivo, línea y columna. Una clave que no existe (por ejemplo un typo) no tumba el archivo: se avisa y el resto se aplica.

## `config.toml`

Todas las claves son opcionales; estos son los valores por defecto:

```toml
theme = "nota-oscuro"     # o "nota-claro", o uno tuyo de themes/
# vault = "~/Notas"       # vault que se abre si no pasas ninguno

[font]
# family = "JetBrains Mono"   # sin esto, la primera monoespaciada instalada
size = 15                     # entre 4 y 200

[editor]
scrolloff = 0             # líneas visibles alrededor del cursor
tabstop = 4               # ancho del tab, al menos 1
expandtab = true          # Tab inserta espacios

[search]
ignorecase = false
smartcase = false         # con ignorecase, una mayúscula vuelve la búsqueda exacta
```

Hoy la app usa el tema y la fuente; el resto de los ajustes se irán aplicando a medida que lleguen las funciones que los usan.

## Temas

Un tema es una **paleta** con nombres y los **colores semánticos** que usa la interfaz. Los temas propios van en `themes/<nombre>.toml` dentro de la carpeta de configuración y se eligen con `theme = "<nombre>"`.

Cada color semántico puede ser:

- un hex: `"#7aa2f7"`
- un nombre de la paleta: `"blue"`
- una tabla con opacidad: `{ color = "blue", alpha = 0.5 }`

Con `extends` un tema parte de otro y solo cambia lo que define. Los colores heredados que nombran la paleta usan la paleta final, así que cambiar `blue` cambia todo lo que era azul:

```toml
# ~/.config/nota/themes/mio.toml
extends = "nota-oscuro"

[palette]
blue = "#82aaff"

[ui]
background = "#101216"
selection = { color = "blue", alpha = 0.25 }
```

Un tema sin `extends` tiene que definir todos los colores. Si un tema no carga (no existe, tiene un error, su cadena de `extends` es circular o le faltan colores), se usa `nota-oscuro` y se reporta el problema. Un tema tuyo con el nombre de uno embebido lo reemplaza.

Las secciones y sus claves son las de los temas embebidos, que sirven de plantilla: [`nota-oscuro.toml`](../crates/nota-config/themes/nota-oscuro.toml) y [`nota-claro.toml`](../crates/nota-config/themes/nota-claro.toml).

- `[ui]`: `background`, `foreground`, `selection`, `current_line`, `gutter`, `gutter_foreground`, `status_background`, `status_foreground`, `status_border`, `modified`, `error`.
- `[cursor]`: `normal`, `insert`, `visual`, `replace`.
- `[syntax]`: `heading`, `bold`, `italic`, `link`, `tag`, `code`, `quote`, `list_marker`, `highlight`, `comment`, `frontmatter`, `markup`.
