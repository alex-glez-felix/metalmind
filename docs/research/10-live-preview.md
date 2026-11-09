# 10 — Live preview: ocultar la sintaxis fuera de la línea del cursor

> **Estado: pendiente de revisión.** Diseño del 2026-11-09 para [#277](https://github.com/alex-glez-felix/metalmind/issues/277) (M4, [03](03-core-architecture.md) §8). No es una decisión hasta que el autor conteste las preguntas del §8: mientras diga "pendiente de revisión", no se implementa nada de aquí. Sin código de producción.
> Construye sobre el plan del paso 2 de [03](03-core-architecture.md) §2, [06](06-decisiones-de-fondo.md) §14, las fórmulas ([08](08-matematicas.md), [#219](https://github.com/alex-glez-felix/metalmind/issues/219), [#267](https://github.com/alex-glez-felix/metalmind/issues/267), [#268](https://github.com/alex-glez-felix/metalmind/issues/268)) y la revisión de arquitectura del [2026-11-02](../arquitectura/2026-11-02.md) (hallazgo 7). Lo que se comprobó y lo que no está en el §9.

## TL;DR

- **Las fórmulas ya son el live preview**, para un solo caso. Tienen las tres piezas: qué líneas muestran la fuente (`SourceLines`), en qué se parte una línea que no la muestra (`parts`, `editor/replaced.rs`) y el mapa de bytes a píxeles (`LaidRow::pieces`). El diseño las generaliza; no agrega un camino aparte.
- **El motor de vim no cambia y no puede enterarse.** Trabaja sobre la fuente en bytes y las filas salen de la fuente (03 §3, #219). Las líneas del cursor muestran la fuente, así que el cursor nunca está sobre texto oculto. Un test impide que `metalmind-vim` nombre el tipo nuevo.
- **Qué muestra la fuente**: la línea del cursor de cada panel (la línea del buffer entera, con todas sus filas), las de Visual y el bloque cercado o `$$` donde está el cursor. No depende del foco ni de la línea de comandos. Mientras un botón del ratón está abajo, no cambia.
- **Cuánto se mueve la nota: 0 px en vertical, siempre.** A lo ancho solo se mueve el texto de la línea que entra o sale, dentro de su fila, lo que miden los marcadores ocultos a su izquierda. Viñetas, casillas, barras de cita y reglas ocupan el ancho de su fuente: esas líneas no se mueven nada.
- **Dónde vive**: un `DisplayLine` puro en `metalmind-syntax` decide qué se ve, qué se oculta y qué se reemplaza. La UI lo pinta cortando la forma de la línea que ya tiene (la del motor, en la caché de GPUI): ocultar no le da forma a nada nuevo.
- **Cuatro PRs**: (1) `DisplayLine` con lo que ya hay, sin cambio visible, y los benchmarks que serán la base; (2) énfasis y headings; (3) links; (4) listas, casillas, citas y reglas.
- **Se apaga** con `editor.live_preview` y `editor::toggle_live_preview` (paleta), para toda la ventana.
- **Comprobado en nvim 0.12.6** (spike, §9): con `conceal`, nvim parte las filas por la fuente, deja filas en blanco donde todo quedó oculto, y los movimientos dan el mismo cursor que sin ocultar.
- **Seis preguntas para el autor** (§8). La que más cambia el resultado es la 1: qué se hace con un tramo oculto que quedó partido entre dos filas.

Dos palabras que se usan abajo: una línea **en fuente** se pinta como hoy, con la sintaxis atenuada; una línea **compuesta** se pinta con lo ocultable oculto o reemplazado.

---

## 0. Lo que ya está decidido y aquí solo se aplica

| Decisión | Dónde |
|---|---|
| Sin panel de vista previa; el estilo vive en el editor y lo ocultable ya va marcado en el modelo | 06 §14 |
| El motor no sabe del ocultamiento: motions, operadores, búsqueda y `.` van sobre la fuente en bytes | 03 §2, "Plan del paso 2" |
| `j`/`k` conservan la columna medida sobre la fuente (`Goal::Column`) | 03 §2 |
| Las filas de una línea salen siempre de su fuente, con el cursor encima o no; vale también para el paso 2 | 03 §3, "Líneas partidas", confirmado en #219 |
| El alto de una línea no depende de dónde esté el cursor | 08 §10 |
| Lo que decide `metalmind-syntax` no lo vuelve a calcular la UI | #267 |
| Nada brinca | [`design.md`](../design.md), principio 2 |

## 1. Qué líneas muestran la fuente

Hoy lo decide `PaneView::source_lines` (`metalmind-ui/src/workspace/source_lines.rs`), puro y sin GPUI, para las fórmulas. La regla queda igual y se le suma el bloque cercado:

> De cada panel: la línea del cursor; en Visual, todas las líneas entre el ancla y el cursor; y si un extremo cae dentro de un bloque `$$` de varias líneas o de un bloque cercado, el bloque entero. Las demás líneas van compuestas.

| Caso | Qué pasa | Por qué |
|---|---|---|
| **`wrap`** | La línea del buffer entera, con todas sus filas | `0`, `$`, `f`, `t` y los conteos de columna miden la línea, no la fila. Si solo la fila del cursor estuviera en fuente, `$` llevaría el cursor a una fila compuesta |
| **Visual** (`v`, `V`, `ctrl-v`) | Todas las líneas de la selección | Lo que se ve seleccionado es lo que recibe el operador. Es lo que hace nvim (§9) |
| **Insert, operador pendiente** | La línea del cursor | Igual que en Normal |
| **Línea de comandos** (`:`, `/`, `?`) | La línea del cursor sigue en fuente | `:s` actúa sobre esa línea y hay que verla. Componerla al abrir `:` y volver a la fuente al cerrarlo movería el texto dos veces por nada. Es lo que hace nvim con `concealcursor` vacío (§9). Tras `:` desde Visual el modo pasa a `Mode::CommandLine` y queda solo la del cursor; vim también quita el resaltado de la selección ahí |
| **Otro overlay con el foco** (finder, paleta, panel lateral) | No cambia nada | La regla mira la selección y el modo del panel, nunca el foco de GPUI |
| **Varios paneles sobre el mismo buffer** | Cada panel tiene las suyas, con su cursor | El mismo texto puede verse en fuente en un panel y compuesto en otro. No cuesta nada: el alto y las filas no dependen de esto (§3), así que los dos comparten `Styles`, `Rows` y la forma de cada línea |
| **Panel sin foco** | Su línea del cursor sigue en fuente | Es lo que hacen hoy las fórmulas y lo que dice 03 §2. nvim hace otra cosa: **pregunta 6** |
| **Cursor dentro de un bloque cercado** | Las dos cercas se ven | 03 §2. Como las líneas de en medio son código y no tienen nada que ocultar, basta con que `SourceLines` tome el bloque entero, igual que el `$$`. Sigue siendo un rango de líneas seguidas |
| **Ratón** | Mientras un botón está abajo, el panel pinta con las líneas en fuente que tenía al pulsar | §2, "El ratón" |
| **`editor.live_preview: false`** | Todas en fuente | §6 |

**Descartado:**

- *Solo la fila del cursor*, o *solo la construcción bajo el cursor* (el link, la negrita). Se mueve menos texto, pero el cursor queda en una línea con partes ocultas: todo lo del §2 dejaría de ser gratis.
- *Componer la línea del cursor en Normal y mostrar la fuente solo en Insert* (`concealcursor=n` de vim). Es el modo "lectura" de la **pregunta 3**: necesita el cursor sobre texto oculto.
- *Esperar un momento antes de mostrar la fuente al llegar a una línea.* Con `j` sostenida evitaría el vaivén de cada línea, pero es una animación, y todo responde al instante (`design.md`).

## 2. Cómo se mueve vim sobre texto oculto

**No se mueve sobre texto oculto.** Es una invariante con dos mitades:

1. **El motor no recibe nada que dependa de qué se oculta.** `Vim::feed` recibe un `Target` (buffer, selecciones, `DocState`, `PaneState`). De ahí sale todo lo que mide: bytes de la fuente, columnas de la fuente (`Goal::Column`, con el `tabstop`) y filas de `ScreenLine`, que se parte por la fuente. `SourceLines` vive en el `Workspace` de `metalmind-ui` y el `DisplayLine` nuevo (§4) no llega al motor.
2. **Donde está el cursor, lo que se ve es la fuente** (§1). Lo que el motor calcula y lo que el usuario ve en esa línea coinciden.

Por eso el resultado es el de vim sobre la fuente, por construcción:

| Movimiento | Qué hace | Nota |
|---|---|---|
| `h` `l`, `0` `^` `$`, `f` `t` `F` `T` `;` `,`, `N\|` | No salen de la línea del cursor, que está en fuente | Sin cambios |
| `w` `b` `e` `W` `B` `E`, text objects | Pueden cruzar de línea. El destino se calcula sobre la fuente y su línea pasa a fuente | `w` desde el final de una línea cae en el `**` con que empieza la siguiente, como en vim |
| `j` `k` con columna deseada | La columna es la de la fuente | La línea de llegada pasa a fuente y el cursor cae donde caería sin ocultar nada. No queda bajo el mismo carácter que se veía antes de entrar: es el precio que 03 §2 ya aceptó. `jk` vuelve al mismo lugar |
| `gj` `gk` `g0` `g$` `gm` | Filas de `ScreenLine`, que son las de la fuente | Las filas no cambian al entrar (§3) |
| `/` `?` `n` `N` `*` `#` | Buscan en la fuente | La línea de la coincidencia pasa a fuente al llegar. El resaltado en las líneas compuestas va por el mapa de píxeles: se pinta la parte visible, y una coincidencia que cae entera en texto oculto (buscar `](`) no se ve hasta que llega el cursor, como dice 03 §2 y como hace vim |
| `H` `M` `L`, `zt` `zz` `zb`, `ctrl-d` `ctrl-u` `ctrl-f` `ctrl-b` | Miden altos y filas | No dependen de qué línea está en fuente (§3) |
| Operadores, `.`, macros, `:s`, `:g`, marks, jumplist | Sobre la fuente | Sin cambios |
| Visual por bloque | Columnas de la fuente | Todas sus líneas están en fuente |

**Descartado: que `j`/`k` conserven la posición que se ve** (pasar de la x en pantalla al byte de la línea compuesta). El resultado ya no sería el de vim, el oráculo fallaría, y el motor tendría que conocer el `DisplayLine` de cada panel.

### Cómo lo verifica el oráculo

Que el motor no reciba nada del ocultamiento hace que los tests de hoy sigan valiendo, pero no prueba nada nuevo. Las pruebas nuevas van en el `Workspace` (modelo sin GPUI), que es donde se juntan el motor, `SourceLines` y el `Screen` de cada panel:

- **Tabla contra nvim** (`workspace/live_preview_oracle.rs`, como `panes_oracle.rs`): cada caso es una nota con marcado y unas teclas. Corre en el `Workspace` con el live preview prendido y un `Screen` que parte las filas, y en `nvim --clean` 0.12.6 sobre la misma fuente. Se comparan texto, cursor y modo. Los casos se eligen para que el cursor cruce o caiga en lo oculto: `l` sobre `**`, `w` y `e` sobre `[[nota|alias]]`, `f]` y `t)`, `$` en una línea que acaba en `**`, `7|`, `j` con columna deseada desde una línea larga a una con un link, `/\[\[` y `n`, `*` sobre el texto de un link, `gj` en una línea partida, `dw` y `ci[` sobre un link, `V` más `j` y un operador.
- **Proptest**: con teclas al azar sobre notas con marcado, el `Workspace` con `live_preview` prendido y apagado da el mismo texto, cursor, registros, primera línea visible (`Viewport::top`) y filas saltadas (`Viewport::skip`). Que la ventana coincida es la mitad vertical del §3.
- **Guarda**: un test de `metalmind-vim` falla si su código nombra `DisplayLine`, como `tests/tokens.rs` vigila los colores en `metalmind-ui`. `metalmind-vim` depende de `metalmind-syntax`, así que el compilador no lo impide solo.

### El ratón

Un clic en una línea compuesta va del punto al byte de la fuente con el mapa de la fila (`LaidRow::closest_index_for_x`, que ya existe): sobre texto, el carácter que se ve; sobre un reemplazo (viñeta, casilla, fórmula), el principio de lo que reemplaza.

Hay un problema que hoy no se nota, porque casi ninguna línea tiene fórmulas. `on_left_down` mueve el cursor, la línea pasa a fuente y su texto se corre a la derecha; el siguiente `on_mouse_move`, con el puntero quieto, calcula el byte con el layout nuevo, da otro byte y empieza una selección que nadie pidió.

**Regla: mientras un botón del ratón está abajo, el panel pinta con las `SourceLines` que tenía al pulsar.** El arrastre selecciona sobre lo que se ve, con el mapa de siempre, y al soltar las líneas de la selección pasan a fuente, una sola vez. Es estado de la vista (`Editor`, junto a `drag_anchor`), no del modelo. Así el error no existe: lo que hay bajo un puntero con el botón abajo no se mueve.

**Un clic en una casilla la alterna sin mover el cursor** (PR 4). Hoy `Workspace::press` lleva el cursor a la línea y `editor::toggle_checkbox` actúa ahí; con el live preview, la línea pasaría a fuente y la casilla recién marcada se volvería `[x]`. Cambia el comportamiento de hoy, en el que el cursor sí se mueve.

## 3. Qué pasa con el layout al entrar y salir de una línea

**En vertical: nada. 0 px, siempre.**

- El alto de una línea sale de `Rows::line_rows` (`metalmind-syntax`), que recibe el estilo de la línea y sus filas. No recibe `SourceLines` ni el `DisplayLine`, y sigue así.
- Las filas salen de `ScreenLine`, que se parte por la fuente (03 §3). Una línea compuesta tiene las mismas filas que en fuente; alguna queda más corta.
- Por eso la ventana (`Viewport`) no depende de dónde esté el cursor, y dos paneles con el mismo documento comparten medidas.

Lo prueban el proptest del §2 y uno más chico: mover el cursor a cualquier línea no cambia el alto de ninguna (`LineHeights::height` del `Layout`).

**A lo ancho, el tope es este:**

- Solo cambian las líneas que entran o salen de `SourceLines`: dos con `j` o `k`, las de la selección al entrar o salir de Visual.
- En cada una, solo se mueve el texto a la derecha del primer marcador `Hide` o `Link` de cada fila, y solo dentro de esa fila. Se corre lo que miden los marcadores ocultos a su izquierda: 3 columnas en `## Título`, 2 tras cada `**`, el destino entero en `[[destino|alias]]`.
- **Los reemplazos ocupan exactamente el ancho de su fuente.** La viñeta va en la celda del `-`, la casilla en las tres de `[ ]`, la barra en la del `>` y la regla sobre sus guiones. Una línea que solo tiene reemplazos no se mueve nada al entrar el cursor: cambia el glifo y el texto se queda donde estaba.
- La rueda lleva el cursor consigo (es un `ctrl-e`/`ctrl-y`), así que al rodar la línea del borde entra y sale de la fuente. Es el mismo movimiento a lo ancho, en una línea.
- Sin `wrap`, cada línea es una fila y lo corrido a lo ancho (`Viewport::left`) se mide sobre la fuente de la línea del cursor, como hoy.

**Lo que cuesta partir por la fuente.** Las filas no se rehacen al ocultar, así que una fila puede quedar corta por la derecha. Con énfasis y headings son 2 a 7 columnas y no se nota. Con un destino largo sí: un `[texto](https://…)` con una URL de 170 caracteres en una columna de 80 ocupa tres filas en fuente y, compuesto, deja una **fila entera en blanco** en medio del párrafo. Es exactamente lo que hace nvim con `conceallevel=2` y `wrap` (§9):

```
uno dos tres [liga]
                                          <- fila en blanco: ahí está la URL
 cuatro cinco seis siete ocho nueve diez
```

No pasa con cualquier link: `ScreenLine` parte después de un espacio, y `texto](url)` no tiene espacios, así que una URL que cabe en una fila baja entera a la siguiente y solo deja corta la de arriba. Pasa cuando la palabra que la contiene mide más que una fila, y con el destino de un wikilink con alias que tiene espacios (`[[Nota de nombre largo|alias]]`).

Qué se hace con ese tramo es la **pregunta 1**. Recomiendo seguir a las fórmulas (#219: "una fórmula larga entre texto se sigue viendo como fuente, porque ahí quedarían huecos"): un tramo ocultable que quedó partido entre filas, o que dejaría su fila sin nada visible, **se ve**, atenuado como hoy. El resto del documento supone esa respuesta y dice dónde cambia si es otra.

**Descartado:**

- *Volver a partir la línea compuesta* (lo que hace Obsidian). Una línea cambiaría de filas al entrar el cursor y todo lo de abajo saltaría. Además el motor tendría que saber qué línea está en fuente en cada panel para medir, que es lo que #219 descartó.
- *Dejar el hueco de lo oculto* para que nada se mueva a lo ancho: un heading empezaría tres columnas adentro y cada negrita tendría dos espacios a cada lado. Ya no se vería el resultado.
- *Colgar los `#` de un heading en el margen* para que el título no se mueva: `editor.padding.horizontal` no alcanza para seis `#`, y los números de línea están ahí.

## 4. Dónde vive cada pieza

| Pieza | Dónde | Estado | Por qué ahí |
|---|---|---|---|
| Qué es ocultable (`Span::conceal`) | `metalmind-syntax`, `style/` | Existe; `Hide` se separa en tres | Sale del scanner, el mismo del índice |
| Filas y altos (`ScreenLine`, `Rows`, `LineRows`) | `metalmind-syntax`, `style/` | No cambia | El motor mueve la ventana con ellos. No saben qué se oculta |
| Qué líneas van en fuente (`SourceLines`) | `metalmind-ui`, `Workspace` | Existe; suma el bloque cercado | Necesita el cursor y el modo de cada panel. Es modelo, sin GPUI |
| **Qué se ve de una línea (`DisplayLine`)** | `metalmind-syntax`, `style/display.rs` | **Nuevo**; reemplaza a `parts` y a `Line::shows_image` | Abajo |
| Píxeles: dónde cae cada trozo (`LaidRow::pieces`) | `metalmind-ui`, `editor/layout.rs` | No cambia | Los anchos son de las formas de GPUI. Ya da `x_for_index`, `x_for_end` y `closest_index_for_x`, y un tramo oculto es un hueco entre dos trozos: sus bytes caen en el borde izquierdo del trozo siguiente |
| Pintar los reemplazos (viñeta, casilla, barra, regla) | `metalmind-ui`, `editor/` | Nuevo (PR 4) | Colores y medidas del tema |

### `DisplayLine`

Hoy la decisión de qué se pinta está repartida en la UI. `parts` (`editor/replaced.rs`) parte la línea en texto y fórmulas, y `Line::shows_image` (`editor/lines/mod.rs`) decide si una fórmula partida se ve como imagen: junta lo que decidió `metalmind-syntax` (`LineRows::split`) con un ancho que solo sabe la UI. Con el ocultamiento, esa decisión crece mucho (qué variante se oculta, qué quedó partido, qué manda dentro de una fórmula), y la usan el editor y la pizarra (§7). Va en un solo lugar:

```rust
// metalmind-syntax, style/display.rs
pub struct DisplayLine { segments: Vec<Segment> }

/// Un tramo de la fuente y qué se ve en su lugar.
pub struct Segment { pub source: Range<usize>, pub show: Show }

pub enum Show {
    Text,
    /// No se pinta ni ocupa ancho.
    Hidden,
    Replaced(Replacement),
}

pub enum Replacement {
    Bullet,
    Checkbox { checked: bool },
    QuoteBar,
    Rule,
    /// El índice en `LineStyle::math`.
    Formula(usize),
}

impl DisplayLine {
    /// La línea en fuente: un solo tramo de texto.
    pub fn source(len: usize) -> Self;
    /// La línea compuesta. `formula_width` da lo que mide la imagen de cada
    /// fórmula, o `None` si no compone.
    pub fn composed(
        len: usize,
        style: &LineStyle,
        screen: Option<&ScreenLine>,
        width: f32,
        formula_width: impl Fn(usize) -> Option<f32>,
    ) -> Self;
    pub fn segments(&self) -> &[Segment];
}
```

- Los tramos van en orden y cubren la línea entera, sin huecos ni solapes. Solo hay rangos de la fuente: **no hay un "texto en pantalla"**. La vista ya le da forma a la línea entera una vez, con los mismos tramos que el motor usó para partirla, y la corta en trozos (`ShapedSlicer`, #213). Ocultar es no pintar un trozo. Esto cambia el plan de 03 §2, que hablaba de un texto a pintar y de rangos en pantalla: es anterior a `wrap`.
- La UI pasa lo único que sabe: el ancho del área y lo que mide cada fórmula. Lo demás lo decide `composed`, en este orden: dentro del rango de una fórmula manda la fórmula (si no compone, su fuente se ve entera, con sus `$`); después cada `Span::conceal` según su variante; y al final la regla de la pregunta 1.
- Si un rango quedó partido entre filas es una sola función privada de `style/`, la que hoy usa `Rows::line_rows` para las fórmulas (`splits`).
- `Line::lay` y el `Builder` recorren `segments()` donde hoy recorren `Part`. `editor/replaced.rs` y `Line::shows_image` se borran.

### `Conceal::Hide` se separa por construcción

Hoy `Hide` marca por igual los `**`, los `#` de un heading, los `[[ ]]`, el destino de un link y los delimitadores de un embed. Para que el `DisplayLine` decida con un `match`, el scanner dice de qué es cada uno:

| Variante | Qué marca | Compuesta |
|---|---|---|
| `Hide` | Énfasis, `#` de un heading, backticks del código inline, `\` de un escape, cercas, `$` de una fórmula | Oculto (PR 2) |
| `Link` | Lo que no es el texto de un link: `[[`, `]]`, `destino\|`, `[`, `](url)` | Oculto (PR 3). Con la respuesta B a la pregunta 1 lleva el número del link en su línea, para que se oculte o se vea entero |
| `Embed` | Los delimitadores de `![[…]]` y `![…](…)` | Se ve, hasta que M5 lo reemplace por la imagen o la nota |
| `Bullet`, `Checkbox`, `QuoteBar`, `Rule` | Como hoy | Reemplazo (PR 4) |
| `Math` | Como hoy | Reemplazo (ya) |

Cada PR prende una variante. La variante existe desde la PR 1 y, hasta su PR, el `match` de `composed` la deja como texto.

### Red flags revisados

- **Fuga de información**: la decisión de qué se ve deja de vivir en `replaced.rs` y `lines/mod.rs` y queda en `DisplayLine`. La UI no vuelve a mirar `Span::conceal` para pintar.
- **La misma abstracción en dos capas**: `Part` desaparece; no queda un `Segment` en la UI. `Piece` no lo repite: son píxeles y formas de GPUI.
- **Capas que solo pasan argumentos**: `SourceLines` sigue llegando a la vista dentro de lo que ya recibe (hoy `MathFrame::source`, que cambia de nombre porque deja de ser solo de fórmulas).
- **Errores que podrían dejar de existir**: la regla del ratón (§2) y los reemplazos del ancho de su fuente (§3).
- **Difícil de describir en una frase** (hallazgo 7 del 2026-11-02): `metalmind-syntax` ya tiene píxeles en `style/`, y el `DisplayLine` suma unas 300 líneas ahí. La revisión pidió volver a mirarlo en este momento: **pregunta 5**. En cualquier caso, el comentario del crate se actualiza en la PR 1.

## 5. Las PRs de implementación

Cada una cumple `CLAUDE.md` (clippy, nextest, red flags en la descripción) y, si cambia algo visible, lleva las capturas de `scripts/capturas.sh` en las cuatro combinaciones, con antes, después y diff.

### PR 1 — `DisplayLine` con lo que ya se compone (sin cambio visible)

- `style/display.rs` con `Show::Text`, `Hidden` y `Replaced(Formula)`. Reemplaza a `parts` y a `Line::shows_image`.
- El scanner separa `Hide` en `Hide`, `Link` y `Embed`. Todo se sigue viendo atenuado.
- El test de `metalmind-vim` que prohíbe nombrar `DisplayLine`, y el comentario de crate de `metalmind-syntax`.
- Los benchmarks del §6, **antes de ocultar nada**: así las PRs 2 a 4 se comparan contra una base que ya los tiene.

**Hecho cuando:** `scripts/capturas.sh … --antes` da 0 píxeles de diferencia en las cuatro combinaciones, y ningún benchmark `editor_*` se mueve más que el ruido (0.7 %, `performance.md`).

**Tests:** los de `replaced.rs`, pasados a tabla sobre `DisplayLine`; un proptest de que los tramos cubren la línea en orden, sin huecos ni solapes, y de que una línea sin nada ocultable da un solo `Text`; la tabla del scanner con las variantes nuevas.

### PR 2 — énfasis y headings

- `Hide` se oculta: `**` y `__`, `*` y `_`, `~~`, `==`, los `#` de un heading y los de cierre. También los backticks del código inline, el `\` de un escape y las cercas: el issue no los nombra, pero son el mismo `Hide` y dejarlos fuera pediría una excepción. Una cerca compuesta deja ver su lenguaje (` ```rust ` se ve `rust`) y la de cierre queda vacía, con el fondo del bloque.
- `SourceLines` suma el bloque cercado.
- La regla del ratón (§2).
- El resaltado de la búsqueda y la selección sobre líneas compuestas.
- `editor.live_preview` y `editor::toggle_live_preview` (§6), con [`config.md`](../config.md).
- La regla de la pregunta 1 para un `Hide` partido (casi nunca pasa: hace falta una palabra más larga que la fila).

**Hecho cuando:** con el cursor fuera de una línea no se ven sus marcadores de énfasis ni de heading; al entrar se ve la fuente; ninguna línea cambia de alto ni de filas; el oráculo y los proptests del §2 pasan; los benchmarks cumplen el §6.

**Tests:** la tabla de `DisplayLine` con cada construcción, anidadas (un link dentro de una negrita dentro de un heading) y de varias líneas (cada marcador se oculta según su línea, 03 §2); el oráculo y los dos proptests del §2; con una ventana de pruebas, que para cada byte visible `closest_index_for_x(x_for_index(b))` devuelve `b`, y que con el botón abajo las líneas en fuente no cambian.

**Capturas:** escenas nuevas con el cursor dentro y fuera, Visual sobre varias líneas, dos paneles con la misma nota, un heading que se parte en filas y la búsqueda resaltada sobre líneas compuestas.

### PR 3 — links

- `Link` se oculta: `[[nota]]` se ve `nota`; `[[nota|alias]]`, `alias`; `[[nota#Sección]]`, `nota#Sección` (solo se oculta, no se reescribe); `[texto](url)`, `texto`. El color del que no resuelve sigue igual.
- La regla de la pregunta 1 por link entero: se oculta o se ve completo, nunca `texto](https://…` a medias.
- Con el ratón sobre un link compuesto, un tooltip con su destino y la tecla que lo sigue (`gd`), el mismo `Tooltip` de las marks y las fórmulas, que no se sale del área del editor. Es lo que pide `design.md`: lo que se oculta se puede ver sin teclado, y la acción muestra su tecla.
- El clic sobre un link, según la **pregunta 4**.
- Los embeds siguen en fuente (M5).

**Hecho cuando:** un link compuesto se ve como su texto; su destino se puede ver con el ratón; un destino más largo que la fila se comporta como decida la pregunta 1; los movimientos sobre links dan lo de nvim.

**Tests:** la tabla de `DisplayLine` (alias, heading, bloque, URL, con y sin título, destino con espacios, link partido, dos links en una fila); casos del oráculo sobre links (`f]`, `t)`, `ci[`, `*`, `gd` desde una línea que acaba de pasar a fuente); la complejidad de `composed` con miles de links en una línea.

**Capturas:** URLs más largas que la fila en 1280x720, alias, un link sin resolver, y el tooltip junto a los bordes derecho e inferior.

### PR 4 — listas, casillas, citas y reglas

- `Bullet`, `Checkbox`, `QuoteBar` y `Rule` se reemplazan, cada uno en el ancho de su fuente (§3). Se pintan con quads, no con glifos de la letra (`☐` depende de la fuente instalada), con los colores que ya tienen sus roles en `syntax.*`. Si hace falta una medida nueva (el lado de la casilla, el grosor de la barra), es un token de `layout`.
- Una tarea se ve como su casilla; su `-` queda en blanco, con su ancho. Los números de una lista ordenada (`1.`) no se tocan.
- El clic en la casilla la alterna sin mover el cursor (§2).
- Cierra M4: 03 §8 y `docs/vim.md`.

**Hecho cuando:** listas, tareas, citas y reglas se ven compuestas fuera del cursor; al entrar y salir, el texto de esas líneas no se mueve ni un píxel.

**Tests:** con una ventana de pruebas, que en una línea que solo tiene reemplazos cada byte visible tiene la misma x compuesta que en fuente; la tabla de `DisplayLine` con listas anidadas, una tarea dentro de una cita y una regla dentro de una cita; el clic en una casilla de una línea compuesta la alterna y deja el cursor y `SourceLines` como estaban.

**Capturas:** listas anidadas en cuatro niveles, tareas marcadas y sin marcar, citas anidadas con una lista dentro, y una regla, en la nota larga.

## 6. Rendimiento y config

### Por qué no debería costar

- Una línea compuesta no se vuelve a medir: se corta la forma de la fuente, que el motor ya pidió en ese frame y está en la caché de GPUI (#213). Se pintan menos glifos y más trozos.
- `DisplayLine::composed` recorre los tramos de una línea una vez, como `runs` y `parts` hoy, y solo para las líneas visibles. No se guarda entre frames mientras no se mida que hace falta.
- Mover el cursor con `j` cambia dos líneas. Ninguna medida del documento se invalida.

Es una expectativa, no una medición (§9).

### Cómo se mide

Con las tres capas de [`performance.md`](../performance.md):

1. **Instrucciones en cada PR** (`metalmind-ui/benches/instrucciones.rs`, límite del 5 % en el CI). Los `editor_*` de hoy se comparan solos. Se suma una **nota densa en marcado** (cada línea con énfasis, links e ítems, más densa que cualquier nota real): primer frame, `j` (dos líneas cambian) y `ctrl-d`. Entra en la PR 1.
2. **Tiempo** (`divan`, `metalmind-syntax/benches/style.rs`): `DisplayLine::composed` para las líneas de una ventana de esa nota. Presupuesto: **≤ 100 µs**, una décima del milisegundo de núcleo de cada tecla. Su fila va en la tabla.
3. **Complejidad**: `composed` es lineal en los tramos de la línea (`assert_linear`, una línea con N negritas y 16N). El test que ya hay (una tecla cuesta lo mismo con 5 k que con 80 k líneas) sigue cubriendo el render.
4. **Benchmark de la UI** (`--frame-log`, antes y después en la misma máquina) en las PRs 2 y 4: tecla en Insert, `j` y `ctrl-d` sobre la nota de 50 k líneas y sobre la densa.

**Criterio de "sin regresión":** ningún benchmark `editor_*` sube más del 5 % (el límite del CI) y el CPU por frame sigue bajo 4 ms. Si una PR necesita la etiqueta `rendimiento-aceptado`, lleva los números y la causa, como dice `performance.md`.

### Se puede apagar

- `editor.live_preview: true` en `config.yaml`, prendido por defecto desde la PR 2.
- `editor::toggle_live_preview`, en la paleta, que lo cambia y lo guarda como `editor::toggle_wrap` ([`config.md`](../config.md), "Lo que se cambia desde la app").
- Apagado, todas las líneas van en fuente, **también las fórmulas**, como anunció 08 §7 ("las fórmulas lo siguen"): las líneas con fórmulas miden como texto.
- **Es de la ventana, no de cada panel.** `wrap` sí tiene `:set wrap!` por panel, pero aquí apagar cambia los altos (los de las fórmulas), y `Rows` es de cada documento: dos paneles con la misma nota y distinto modo necesitarían dos juegos de medidas. Por eso no hay `:set conceallevel`.

El roadmap (03 §8) dice "Toggle source/live/lectura". Aquí hay dos estados; el tercero es la **pregunta 3**.

## 7. Qué reusa #256 (markdown en los nodos de la pizarra)

Hoy el texto de una caja se compone con `shape_text` de GPUI y se parte a su ancho (`board/label.rs`); una caja abierta la edita el motor de vim y se pinta igual, para que no brinque al abrirla (`board/open_text.rs`).

| Pieza | ¿La reusa? |
|---|---|
| `Styles` sobre el texto del nodo (roles, énfasis, links) | Sí, tal cual. Es `metalmind-syntax`; `metalmind-board` sigue sin depender de nada |
| `DisplayLine::composed` | Sí. Una caja cerrada compone todas sus líneas |
| `SourceLines` | La regla sí, el tipo no: es de `PaneView`. Una caja abierta tiene su cursor, y la línea donde está va en fuente. Para compartir la regla, se saca de `PaneView` una función sobre (cursor, ancla, modo, `Styles`) |
| De tramos a `TextRun` (`editor/markdown.rs`, `editor/text.rs`, `Faces`) | Sí; `Faces` ya recibe letra y tamaño, que en la pizarra cambian con el zoom |
| Armar una línea (`Line::lay`, `Builder`, `ShapedSlicer`) | Sí, si no conoce al editor. Hoy `Line::lay` no recibe `PaneView` ni `Viewport`. Las PRs 1 a 4 lo mantienen: es su criterio para #256 |
| Partir por la fuente (`ScreenLine`) | Tiene que adoptarlo. Si la caja cerrada partiera el texto compuesto y la abierta la fuente, el texto brincaría al abrirla, que es lo que `label.rs` evita hoy |
| Viewport, margen, scroll, búsqueda | No |
| Fórmulas en los nodos | No hace falta: `formula_width` puede dar siempre `None`, y se ven como fuente |

Dos cosas que #256 tiene que decidir por su cuenta: el texto de las cajas va **centrado**, así que una fila más corta se vuelve a centrar al componerse (en el editor se queda a la izquierda); y las cajas son angostas, así que el costo de partir por la fuente (§3) se nota más.

## 8. Preguntas para Alex

Ninguna está escrita en otro documento. Con cada respuesta se actualiza este documento y el estado del encabezado.

1. **Un tramo ocultable que quedó partido entre dos filas** (una URL más larga que la fila, el destino con espacios de un wikilink con alias). Ver §3.
   - A: Se oculta igual, como vim. Puede quedar una fila en blanco, o varias, en medio de un párrafo.
   - B: Ese tramo se ve, atenuado como hoy, y el link va entero: o se oculta todo o se ve todo. Tampoco se oculta lo que dejaría su fila sin nada visible. Es la regla de las fórmulas partidas (#219).
   - C: Volver a partir la línea compuesta, como Obsidian. La nota salta al entrar y salir de esas líneas, y el motor tiene que saber qué línea está en fuente.
   - **Recomiendo B.** Una URL larga se ve como hoy, que no es peor que ahora, y no aparecen huecos que parecen cortes de párrafo. C rompe "nada brinca" y lo que confirmó #219. El costo de B: que un link se oculte depende del ancho del panel.

2. **Las líneas de un bloque `$$` de varias líneas** (08 §7 lo dejó para este paso). Hoy se ve su fuente y la fórmula debajo.
   - A: Dejarlo así. La fórmula ya se ve, y la fuente también.
   - B: Líneas de alto cero cuando el cursor está fuera (`conceal_lines` de nvim). Al entrar al bloque, lo de abajo baja tantas líneas como tenga.
   - C: Un mapa de filas a líneas, como los pliegues.
   - **Recomiendo A**, y que no entre en M4. B y C rompen la invariante del §3 (0 px en vertical), que es lo que mantiene el scroll igual al del oráculo. Si un día molesta, es su propio issue, junto con los pliegues.

3. **El modo "lectura"** del roadmap (03 §8: "Toggle source/live/lectura"): todo compuesto, también la línea del cursor.
   - A: M4 tiene dos estados, fuente y live. "Lectura" pasa a su propio issue.
   - B: Los tres en M4.
   - **Recomiendo A.** En "lectura" el cursor está sobre texto oculto, y es justo lo que este diseño evita para que vim dé lo mismo que el oráculo. Hacerlo pide otro diseño: el cursor sobre el mapa, y qué hacen `l` y `x` encima de un marcador que no se ve.

4. **Un clic sobre un link compuesto.** Hoy un clic solo pone el cursor; `gd` y `enter` siguen el link. 03 §8 dice "links como texto clicable".
   - A: El clic sigue el link, como Obsidian. Para editarlo se llega con el teclado o con un clic en otra parte de la línea.
   - B: `ctrl`+clic lo sigue, como en Zed; el clic pone el cursor, como en cualquier otro texto. El tooltip muestra el destino, `gd` y `ctrl`+clic.
   - C: Como hoy: el clic pone el cursor y solo `gd` o `enter` siguen.
   - **Recomiendo B.** Un clic que a veces mueve el cursor y a veces cambia de nota sorprende. Con B el ratón puede seguir links sin quitarle al clic lo que hace en todo el texto.

5. **Dónde vive el `DisplayLine`** (hallazgo 7 del 2026-11-02: `style/` ya tiene píxeles, y había que volver a mirarlo al llegar aquí).
   - A: En `metalmind-syntax`, `style/display.rs`, junto a `ScreenLine` y `LineRows`.
   - B: Un crate nuevo `metalmind-layout` con `Advances`, `ScreenLine`, `Rows`, `LineRows` y `DisplayLine`. Cambia 03 §6.
   - C: En `metalmind-ui`, como `replaced.rs` hoy.
   - **Recomiendo A.** Se arma solo con datos de `metalmind-syntax` (estilo y filas), y C repetiría lo de #267. B es un cambio de nombres que se puede hacer cuando haya una razón: `metalmind-vim` necesita `Rows` y `ScreenLine`, así que dependería del crate nuevo igual, y la guarda del §2 haría falta de todos modos.

6. **La línea del cursor de un panel sin foco.**
   - A: Sigue en fuente, como hoy con las fórmulas y como dice 03 §2.
   - B: Compuesta: solo el panel con el foco muestra fuente. Es lo que hace nvim (§9). Con la misma nota en dos paneles, el otro queda como una vista compuesta completa.
   - **Recomiendo A.** Con B, cada `ctrl-w w` mueve el texto de dos líneas. La ventaja de B es real (un split de la misma nota sirve de vista de lectura, sin una línea suelta en fuente). El cambio es chico en cualquier dirección: una condición en `source_lines`.

## 9. Qué se comprobó y qué no

**Comprobado**, en un spike fuera del repo con `nvim --clean --headless` 0.12.6 (la versión del oráculo), `conceallevel=2`, `concealcursor` vacío y `matchadd('Conceal', …)` sobre `**`, `[[ ]]` y `(https://…)`:

- **Las filas salen de la fuente.** Una línea de 239 caracteres con una URL de 170, en una ventana de 80 columnas, ocupa 3 filas con `conceallevel` 0 y con 2. Con 2, la fila de en medio queda en blanco (el dibujo del §3 es la pantalla de nvim).
- **La línea del cursor muestra la fuente** y las demás no.
- **Los movimientos dan el mismo cursor** con `conceallevel` 0 y 2 en 15 secuencias (`l`, `w`, `e`, `b`, `f]`, `t)`, `$`, `7|`, `j` con columna deseada, `gj`, `gk`, `/`, `n` y Visual con `y`).
- **En Visual, todas las líneas de la selección muestran la fuente.**
- **En la línea de comandos, la línea del cursor sigue en fuente.**
- **En una ventana que no es la actual, la línea de su cursor va oculta.** Aquí el código de hoy hace otra cosa (pregunta 6).

Y leyendo el código de `main` en `b4a5fae`: lo que este documento dice de `SourceLines`, `parts`, `Line::shows_image`, `LaidRow`, `ScreenLine`, `Rows`, las marcas del scanner, el ratón y `board/label.rs`.

**No comprobado:**

- **El costo.** No se escribió `DisplayLine` ni se midió nada: el §6 es una expectativa razonada sobre el código que hay. El riesgo que veo es el número de trozos por fila en una línea muy densa (cada trozo visible es un corte de la forma y una llamada a pintar); el benchmark de la nota densa de la PR 1 es el que lo dirá.
- **Cómo se ve.** No hay capturas ni prototipo. Lo que quizá cambie al verlo: la cerca compuesta (su lenguaje solo, la de cierre vacía), la casilla sin su viñeta, y qué tan seguido aparece el caso de la pregunta 1 en notas reales.
- **El problema del ratón del §2** sale de leer `on_left_down` y `on_mouse_move`; no se reprodujo en una ventana.
- **Que `Line::lay` sirva tal cual para la pizarra** (§7): se leyó su firma, no se probó desde `board/`.
- **Los tamaños**: "unas 300 líneas" del `DisplayLine` es una estimación.
