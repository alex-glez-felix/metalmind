#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "en tests, un unwrap que falla es un test que falla"
)]

//! Capturas de cada pantalla en un Xvfb propio, en los dos temas y en
//! ventanas de 1920x1080 y 1280x720, sobre un vault que estresa el layout
//! (`capturas/vault.rs`). Con `METALMIND_CAPTURAS_ANTES` compara cada una con
//! la del mismo nombre en esa carpeta y falla si cambia algún píxel.
//!
//!   scripts/capturas.sh [salida] [--antes carpeta]
//!
//! Cómo y cuándo usarlas: `docs/design.md`, "Proceso".

#[path = "capturas/image.rs"]
mod image;
#[path = "capturas/saltos.rs"]
mod saltos;
#[allow(dead_code, reason = "las capturas usan solo una parte del harness")]
mod support;
#[path = "capturas/vault.rs"]
mod vault;

use std::env;
use std::fmt::Write as _;
use std::fs;
use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use image::Image;
use metalmind_config::{Paths, Themes};
use rstest::rstest;
use support::wait::{self, TIMEOUT};
use support::{Smoke, Window};

/// Sin frames nuevos durante este tiempo, la ventana ya pintó lo último.
const QUIET: Duration = Duration::from_millis(400);

/// Donde caen las capturas; por defecto, en `target/tmp/capturas`.
const OUT_VAR: &str = "METALMIND_CAPTURAS";
/// Una carpeta con capturas de antes para comparar.
const BEFORE_VAR: &str = "METALMIND_CAPTURAS_ANTES";

/// Lo que se hace antes de capturar una pantalla.
enum Step {
    Keys(&'static str),
    /// Clic derecho en la fila `n` del árbol.
    RightClickRow(i16),
    /// Clic derecho en la última fila del árbol, cerca de su borde derecho:
    /// el menú no cabe hacia abajo.
    RightClickBottomRow,
    /// Escribe [`vault::SEARCHED`].
    Search,
    /// Cambia algo en el disco del vault, como otro programa.
    Files(fn(&Path)),
    /// Espera a que la ventana pinte lo último que cambió y se calme, como
    /// las filas que agrega el watcher, antes de hacer clic donde estaban.
    Settle,
    /// Deja el ratón sobre una mark hasta que sale su tooltip.
    HoverMark(MarkAt),
    /// Borra lo escrito y escribe [`vault::SEARCHED_AFTER_CJK`].
    SearchAfterCjk,
    /// Lleva la línea de fórmulas rotas de [`vault::LONG_FORMULAS`] al
    /// fondo de la ventana y el cursor a la fórmula de la zona de prueba.
    FormulasZone,
    /// Pone el ratón sobre la última fórmula rota que se ve: su mensaje
    /// queda junto a los bordes derecho e inferior.
    HoverBrokenFormula,
}

/// Sobre qué mark se deja el ratón.
#[derive(Clone, Copy)]
enum MarkAt {
    /// En el panel de la izquierda, la línea `n` desde arriba, sin headings
    /// antes.
    Line(i16),
    /// La más baja del panel de la izquierda: el tooltip junto al borde de
    /// abajo.
    Lowest,
    /// La primera línea del último de cuatro paneles lado a lado: el tooltip
    /// junto al borde derecho.
    RightmostPane,
}

/// Cada pantalla parte de cómo dejó la ventana la anterior.
const SCENES: &[(&str, &[Step])] = &[
    ("editor", &[]),
    ("editor-final", &[Step::Keys("G")]),
    ("arbol", &[Step::Keys("gg<Space>ejjj")]),
    ("menu", &[Step::RightClickRow(2)]),
    (
        "menu-borde",
        &[Step::Keys("<Esc>"), Step::RightClickBottomRow],
    ),
    ("split", &[Step::Keys("<Esc><C-w>l<C-w>v")]),
    // La línea de `:` es un pop-up como la paleta (#193): lo largo se parte
    // en filas; debajo van los nombres que empiezan con lo escrito, con el
    // que corre primero, y con la línea vacía, el historial.
    (
        "linea-de-comandos",
        &[Step::Keys(
            "<C-w>o:una linea de comandos larga, que sigue mas alla del ancho del pop-up y no se corta: se parte en filas, sin_espacios_en/una/ruta/muy/larga/que/tampoco/cabe.md",
        )],
    ),
    ("linea-de-comandos-nombres", &[Step::Keys("<Esc>:s")]),
    (
        "linea-de-comandos-historial",
        &[Step::Keys("<Esc>:noh<CR>:focus #rust<CR>:unfocus<CR>:")],
    ),
    ("finder", &[Step::Keys("<Esc><Space>f")]),
    ("finder-filtrado", &[Step::Keys("idea")]),
    ("finder-ruta-larga", &[Step::Keys("<C-u>honda")]),
    ("finder-vacio", &[Step::Keys("<C-u>zzqqxx")]),
    // Los tags estrechan antes del fuzzy (05 §4); lo que no encuentra nada
    // se marca.
    ("finder-tags", &[Step::Keys("<C-u>#rust -#archivado idea")]),
    (
        "finder-tags-inexistentes",
        &[Step::Keys("<C-u>#rust -#nada #2024/10 idea")],
    ),
    ("finder-buzon", &[Step::Keys("<C-u>-#")]),
    ("buscar", &[Step::Keys("<Esc><Space>/"), Step::Search]),
    ("buscar-vacio", &[Step::Keys("<C-u>zzqqxx")]),
    (
        "buscar-escrito-largo",
        &[Step::Keys(
            "<C-u>zeta y un patron de busqueda mucho mas largo que el ancho del picker, para ver que el cursor sigue a la vista",
        )],
    ),
    ("buscar-cjk", &[Step::SearchAfterCjk]),
    ("paleta", &[Step::Keys("<Esc><C-S-p>")]),
    // `ctrl-p` desde la primera fila da la vuelta: el final de la lista.
    ("paleta-final", &[Step::Keys("<C-p>")]),
    // Filtrada, para ver cada comando con su atajo, el de la ventana
    // (`ctrl-shift-p`) o el de vim (`space f`, `ctrl-w l`).
    ("paleta-save", &[Step::Keys("save")]),
    ("paleta-focus", &[Step::Keys("<C-u>focus")]),
    ("paleta-workspace", &[Step::Keys("<C-u>workspace")]),
    ("paleta-pane", &[Step::Keys("<C-u>pane:")]),
    ("paleta-panel", &[Step::Keys("<C-u>panel:")]),
    ("paleta-editor", &[Step::Keys("<C-u>editor:")]),
    // Al final, para que las marks no cambien las demás pantallas: tres en
    // una línea (la `b` y un `+`), una sobre un heading y el tooltip.
    (
        "marks",
        &[
            Step::Keys("<Esc>ggmajmbmcmBjjmA"),
            Step::HoverMark(MarkAt::Line(1)),
        ],
    ),
    // La nota larga: marks en líneas de cuatro cifras, la última con once
    // (`a`–`h` y `A`–`C`: un tooltip largo) y, con `zb`, el tooltip junto al
    // borde de abajo. La última línea es más larga que la ventana.
    (
        "marks-nota-larga",
        &[
            Step::Keys("GmambmcmdmemfmgmhmAmBmCkkkmzkkkkkmyGzb"),
            Step::HoverMark(MarkAt::Lowest),
        ],
    ),
    (
        "marks-borde-derecho",
        &[
            Step::Keys("<C-w>v<C-w>v<C-w>vGzt"),
            Step::HoverMark(MarkAt::RightmostPane),
        ],
    ),
    // Listas en una nota nueva: `Enter` las continúa (la casilla sin
    // marcar, el número siguiente) y en un ítem vacío la termina; después
    // la paleta marca la primera casilla.
    (
        "listas",
        &[Step::Keys(
            "<Esc><C-w>o:e listas.md<CR>:set rnu<CR>i- [ ] comprar pan<CR>leer el capitulo 3<CR><CR>1. uno<CR>dos<CR>tres<Esc>",
        )],
    ),
    (
        "listas-casilla",
        &[Step::Keys("gg<C-S-p>toggle checkbox<CR>")],
    ),
    ("formulas", &[Step::Keys("<Esc><C-w>o:e formulas.md<CR>")]),
    ("formulas-bloque", &[Step::Keys("Gk")]),
    (
        "formulas-larga",
        &[
            // La línea de fórmulas rotas tiene que pasar del borde: sin wrap,
            // que desde #182 viene puesto.
            Step::Keys(":e formulas-larga.md<CR>:set nornu nowrap<CR>"),
            Step::FormulasZone,
            Step::HoverBrokenFormula,
        ],
    ),
    // El indicador del autoguardado, con la nota de la ruta más larga.
    (
        "autoguardado-guardado",
        &[Step::Keys("<Esc><C-w>o<Space>ffondo<CR>")],
    ),
    (
        "autoguardado-sin-guardar",
        &[
            Step::Files(read_only),
            Step::Keys("iAgregar al acta: <Esc>"),
        ],
    ),
    (
        "autoguardado-cambio-en-disco",
        &[Step::Files(changed_outside)],
    ),
    // La vista de tags del panel, al final para no cambiar las demás: las
    // raíces, `#Rust` abierto con `#diseño` (su hijo, con ícono) abierto
    // dentro, el buzón al final y el menú de una nota junto al borde.
    ("tags", &[Step::Keys("<Esc><Space>t")]),
    ("tags-abierto", &[Step::Keys("jjjjjlll")]),
    ("tags-buzon", &[Step::Keys("GlG")]),
    ("tags-menu-borde", &[Step::RightClickBottomRow]),
    // El autocompletado de `#`: la lista, por un alias, eligiendo con
    // `ctrl-n`, aceptada con `tab` y junto al borde de abajo, donde se abre
    // encima del tag.
    (
        "autocompletado",
        &[Step::Keys("<Esc>q:e listas.md<CR>Go#r")],
    ),
    ("autocompletado-alias", &[Step::Keys("s")]),
    ("autocompletado-elegir", &[Step::Keys("<BS><C-n>")]),
    ("autocompletado-aceptado", &[Step::Keys("<Tab>")]),
    (
        "autocompletado-borde",
        &[Step::Keys("<Esc>:e formulas-larga.md<CR>GA #un")],
    ),
    // Un alias largo junto al borde derecho: la lista no sale de la
    // ventana, se recortan el nombre y el alias.
    (
        "autocompletado-alias-largo",
        &[Step::Keys(concat!(
            "<Esc>:e listas.md<CR>Go",
            "Un texto muy largo que lleva el tag hasta el borde derecho de la ventana, ",
            "aunque la ventana sea ancha y el panel lateral siga abierto, para que la ",
            "lista se abra pegada al borde y se vea que no sale de la ventana #arquitectura",
        ))],
    ),
    // En un panel bajito no caben todas las filas ni arriba ni abajo: van
    // las que caben, del lado con más lugar.
    (
        "autocompletado-panel-bajo",
        &[Step::Keys("<Esc><C-w>o<C-w>s<C-w>s<C-w>s<C-w>sGo#r")],
    ),
    // Renombrar y fusionar tags (05 §5): el menú de un tag y su pregunta en
    // el panel, la vista previa con muchas filas y rutas largas, una fila
    // quitada, el final de la lista (la página y el `parents` de otra), una
    // fusión bloqueada y los avisos al aplicar y deshacer.
    (
        "tags-menu-tag",
        &[
            Step::Keys("<Esc><Esc><C-w>o<Space>tgg"),
            Step::RightClickRow(0),
        ],
    ),
    // Renombrar es la séptima entrada del menú de un tag.
    ("tags-renombrar-pregunta", &[Step::Keys("jjjjjj<CR>")]),
    (
        "renombrar-tag",
        &[Step::Keys(
            "<Esc><C-w>l:tagrename #rust #lenguaje-de-programacion-con-un-nombre-larguisimo-que-no-cabe<CR>",
        )],
    ),
    ("renombrar-tag-quitado", &[Step::Keys("jjx")]),
    ("renombrar-tag-final", &[Step::Keys("kkk")]),
    // `#diseño` por su alias largo: XTest no teclea la `ñ`.
    (
        "fusionar-tag-bloqueado",
        &[Step::Keys(concat!(
            "<Esc>:tagmerge #rs #arquitectura-de-la-interfaz-y-sus-principios-a-largo-plazo-",
            "con-un-alias-que-no-cabe-ni-en-la-ventana-mas-ancha-que-hay-porque-sigue-y-sigue-",
            "sin-parar-por-mucho-que-uno-quiera-leerlo-entero-de-una-sola-vez-en-la-lista-de-",
            "tags-del-autocompletado<CR>",
        ))],
    ),
    (
        "renombrar-tag-aplicado",
        &[Step::Keys("<Esc>:tagrename #archivado #guardado<CR><CR>")],
    ),
    ("renombrar-tag-deshecho", &[Step::Keys(":vaultundo<CR>")]),
    // Notas nuevas desde plantillas (06 §11). Las plantillas llegan aquí,
    // como de otro programa, para no cambiar las pantallas de antes: el menú
    // de un tag con su nota nueva, la pregunta del nombre desde el tag, el
    // picker de plantillas con los tags que las piden, uno vacío, el nombre
    // tomado, una ruta larga, la nota creada y la nota del día.
    (
        "tags-menu-nueva-nota",
        &[
            Step::Files(vault::write_templates),
            Step::Settle,
            Step::Keys("<Esc><Esc><C-w>o<C-w>h"),
            // `#Rust`, cuya página pide `rust/capitulo`.
            Step::RightClickRow(9),
        ],
    ),
    ("tags-nueva-nota", &[Step::Keys("<CR>")]),
    (
        "tags-nueva-nota-creada",
        &[Step::Keys(
            "notas de rust/un borrador del capitulo sobre el borrow checker y sus reglas<CR>",
        )],
    ),
    ("plantillas", &[Step::Keys("<Esc><Space>n")]),
    ("plantillas-filtradas", &[Step::Keys("entre")]),
    ("plantillas-vacio", &[Step::Keys("<C-u>zzqqxx")]),
    ("plantilla-nombre", &[Step::Keys("<C-u>entrevista<CR>")]),
    ("plantilla-nombre-tomado", &[Step::Keys("otra")]),
    (
        "plantilla-nombre-largo",
        &[Step::Keys(
            "<C-u>entrevistas/2026/octubre/una entrevista con un nombre larguisimo para ver como se recorta la ruta en la fila del picker",
        )],
    ),
    // El input se corre hasta el cursor (#147): al final de lo escrito y,
    // con `ctrl-a`, de vuelta al principio.
    ("plantilla-nombre-largo-inicio", &[Step::Keys("<C-a>")]),
    ("plantilla-creada", &[Step::Keys("<CR>")]),
    ("nota-del-dia", &[Step::Keys("<Esc>:today<CR>")]),
    // El resto del menú de un tag (05 §3): el menú entero sobre `#Rust`, la
    // pregunta de sus alias con los de ahora y una lista larga, su vista
    // previa y aplicada, un ícono que no vale y la página abierta.
    (
        "tags-menu-completo",
        &[Step::Keys("<C-w>h"), Step::RightClickRow(9)],
    ),
    ("tags-alias-pregunta", &[Step::Keys("<Esc>e")]),
    (
        "tags-alias-largos",
        &[Step::Keys(
            ", rustlang, ferris-el-cangrejo, el-lenguaje-de-sistemas-con-un-alias-que-no-cabe-en-el-panel",
        )],
    ),
    // El campo deja ver el cursor (#149): al final y, con `home`, al principio.
    ("tags-alias-largos-inicio", &[Step::Keys("<Home>")]),
    ("tags-alias-vista-previa", &[Step::Keys("<CR>")]),
    ("tags-alias-aplicado", &[Step::Keys("<CR>")]),
    ("tags-icono-invalido", &[Step::Keys("<C-w>hi<BS>libro<CR>")]),
    ("tags-pagina", &[Step::Keys("<Esc>P")]),
    // El autocompletado de `[[`: con nada escrito (las recientes primero),
    // muchas notas que coinciden, dos `plan.md` que empatan (una en una
    // carpeta larguísima), la más honda aceptada (lleva su carpeta: la otra
    // gana el `[[plan]]` por menos honda), junto al borde de abajo y
    // una ruta larga junto al borde derecho. Las dos `plan.md` llegan como
    // de otro programa, para no cambiar las pantallas de antes.
    (
        "links-autocompletado",
        &[
            Step::Files(vault::write_link_notes),
            Step::Settle,
            Step::Keys("<Esc><Esc><C-w>o:e listas.md<CR>Go[["),
        ],
    ),
    ("links-autocompletado-filtrado", &[Step::Keys("idea 1")]),
    ("links-autocompletado-empate", &[Step::Keys("<Esc>o[[plan")]),
    ("links-autocompletado-aceptado", &[Step::Keys("<Tab>")]),
    (
        "links-autocompletado-borde",
        &[Step::Keys("<Esc>:e formulas-larga.md<CR>GA [[idea")],
    ),
    (
        "links-autocompletado-ruta-larga",
        &[Step::Keys(concat!(
            "<Esc>:e listas.md<CR>Go",
            "Un texto muy largo que lleva el link hasta el borde derecho de la ventana, ",
            "aunque la ventana sea ancha y el panel lateral siga abierto, para que la ",
            "lista se abra pegada al borde y se vea que no sale de [[tampoco cabe",
        ))],
    ),
    // Tablas (#151): una llenada a mano, `editor::format_table` en la
    // paleta, sin tecla, y la tabla alineada con el cursor en su celda; al
    // final, `:tableformat` en un registro de más de mil filas. La nota llega
    // aquí, como de otro programa, para no cambiar las pantallas de antes.
    (
        "tabla-desalineada",
        &[
            Step::Files(vault::write_tables),
            Step::Settle,
            Step::Keys("<Esc><Esc><C-w>o:e tablas.md<CR>:set nornu<CR>/Comunic<CR>"),
        ],
    ),
    ("tabla-paleta", &[Step::Keys("<C-S-p>format table")]),
    ("tabla-alineada", &[Step::Keys("<CR>")]),
    (
        "tabla-larga-alineada",
        &[Step::Keys("/evento-final<CR>:tableformat<CR>")],
    ),
    // El autocompletado de `:focus` en el pop-up de `:` (#156, #193): muchas
    // carpetas con rutas hondas, los tags tras aceptar una, y la carpeta
    // larga con espacios, que el indicador muestra escapada (#157).
    (
        "focus-completar-carpetas",
        &[Step::Keys("<Esc><Esc>:focus pro")],
    ),
    ("focus-completar-tags", &[Step::Keys("<Tab> #")]),
    ("focus-completar-espacios", &[Step::Keys("<Esc>:focus Una")]),
    ("focus-carpeta-con-espacios", &[Step::Keys("<Tab><CR>")]),
    // El modo focus (05 §3), al salir de las tablas: el panel en sus dos vistas, el menú de una
    // carpeta, el finder y la búsqueda con el alcance, uno largo, uno que
    // no deja nada, y el que pone y quita `F` desde el panel. La fila del
    // focus bajo el encabezado corre las del árbol una más abajo.
    (
        "focus-tags",
        &[Step::Keys(
            "<Esc><Esc><C-w>l:focus #rust -#archivado<CR><C-w>h",
        )],
    ),
    ("focus-carpetas", &[Step::Keys("<Space>egg")]),
    ("focus-menu", &[Step::RightClickRow(1)]),
    ("focus-finder", &[Step::Keys("<Esc><C-w>l<Space>f")]),
    ("focus-buscar", &[Step::Keys("<Esc><Space>/idea 0")]),
    (
        "focus-largo",
        &[Step::Keys(
            "<Esc>:focus Ideas/ #rust -#archivado -#un-tag-que-no-existe-con-un-nombre-muy-largo-para-ver-el-corte<CR>",
        )],
    ),
    // En el finder, un focus largo se recorta y deja ver lo escrito.
    ("focus-largo-finder", &[Step::Keys("<Space>fidea")]),
    ("focus-vacio", &[Step::Keys("<Esc>:focus #nada<CR>")]),
    ("focus-finder-vacio", &[Step::Keys("<Space>f")]),
    (
        "focus-desde-el-panel",
        &[Step::Keys("<Esc>:unfocus<CR><C-w>hTggF")],
    ),
    ("focus-menu-salir", &[Step::RightClickRow(1)]),
    ("sin-focus", &[Step::Keys("<Esc>F")]),
    // El panel de backlinks: muchas notas con rutas hondas, líneas largas y
    // multibyte que enlazan a `Metalmind/plan` (llegan como de otro
    // programa), el final de la lista, una nota cerrada, el menú de la
    // última fila junto al borde y la nota abierta en su link, que no tiene
    // backlinks.
    (
        "backlinks",
        &[
            Step::Files(vault::write_backlinks),
            Step::Settle,
            Step::Keys("<Esc><Esc><C-w>l<C-w>o:e Proyectos/Metalmind/plan.md<CR><Space>b"),
        ],
    ),
    ("backlinks-final", &[Step::Keys("G")]),
    ("backlinks-plegado", &[Step::Keys("ggjhh")]),
    ("backlinks-menu-borde", &[Step::RightClickBottomRow]),
    ("backlinks-abierto", &[Step::Keys("<Esc>gg<CR>")]),
    // Los links sin destino (`space u`): muchos, con nombres más largos que
    // el picker, multibyte y emoji (llegan como de otro programa); filtrados,
    // sin resultados y el elegido abierto en el editor, donde se ve con el
    // color de los links sin destino.
    (
        "sin-destino",
        &[
            Step::Files(vault::write_unresolved),
            Step::Settle,
            Step::Keys("<Space>u"),
        ],
    ),
    ("sin-destino-filtrado", &[Step::Keys("tarea 1")]),
    ("sin-destino-vacio", &[Step::Keys("<C-u>zzqqxx")]),
    ("sin-destino-abierto", &[Step::Keys("<C-u>informe<CR>")]),
    // Listas angostas, de `#` y de `[[`: la tecla de la fila elegida tiene
    // su columna y no se pega al conteo ni al detalle.
    (
        "autocompletado-angosto",
        &[Step::Keys(
            "<Esc><Esc><C-w>l:e formulas-larga.md<CR>GA #mark",
        )],
    ),
    (
        "links-autocompletado-angosto",
        &[Step::Keys("<Esc>A [[idea 0")],
    ),
    // Las líneas largas (#175): con `wrap`, la nota partida en filas, con la
    // búsqueda y una selección que cruzan filas; sin él, la vista corrida a
    // lo ancho hasta el cursor, al final de una línea y de una palabra sin
    // espacios más larga que la ventana.
    (
        "wrap-seleccion",
        &[Step::Keys(
            "<Esc><Esc><C-w>l<C-w>o<Space>fbastante largo<CR>/sigue y sigue<CR>v3gj",
        )],
    ),
    ("sin-wrap", &[Step::Keys("<Esc>:set nowrap<CR>$")]),
    ("sin-wrap-palabra", &[Step::Keys("j$")]),
    // La lista de notas (#164), al final porque etiqueta una nota. Las notas
    // vuelven a fechas fijas para que la lista salga igual en cada corrida:
    // el buzón, el picker de fuentes, un tag con sus descendientes, el filtro
    // en su lugar y vacío, una nota abierta desde la lista y la nota larga
    // con la lista abierta, plegada y otra vez abierta (no se mueve), y `t`.
    (
        "lista-buzon",
        &[
            Step::Files(vault::age_notes),
            Step::Settle,
            Step::Keys("<Esc><Esc><C-w>o<C-w>hq<Space>l"),
        ],
    ),
    ("lista-fuente", &[Step::Keys("<Space>v")]),
    ("lista-tag", &[Step::Keys("#rust<CR>")]),
    ("lista-filtro", &[Step::Keys("<Space>f-#archivado idea")]),
    ("lista-filtro-vacio", &[Step::Keys("<C-u>zzqqxx")]),
    ("lista-nota", &[Step::Keys("<Esc><Esc>jj<CR>")]),
    // Con `wrap`, el de fábrica: plegar cambia el ancho y las filas
    // partidas, pero la nota sigue en la misma línea y con el cursor.
    (
        "lista-nota-larga",
        &[Step::Keys("<Space>festr<CR>:set wrap<CR>1000Gzz")],
    ),
    ("lista-plegada", &[Step::Keys("<Space>l")]),
    ("lista-desplegada", &[Step::Keys("<Space>l")]),
    (
        "lista-etiquetar",
        &[Step::Keys("<Space>vbuz<CR>jt#ideas rust")],
    ),
    ("lista-etiquetada", &[Step::Keys("<CR>")]),
    // El árbol sin selección (#189): `esc` la quita y lo nuevo va a la raíz.
    (
        "arbol-sin-seleccion",
        &[Step::Keys("<Esc><Esc><C-w>l<Space>egg<Esc>")],
    ),
    ("arbol-nueva-carpeta-raiz", &[Step::Keys("A")]),
    // Los avisos en toasts (#194): `esc` cierra los que quedan; tres
    // errores, que no se cierran solos, uno tan largo que se parte en
    // líneas; el historial de toda la corrida, que los tapa, filtrado, y el
    // elegido otra vez como el último.
    (
        "toasts",
        &[Step::Keys(concat!(
            "<Esc><Esc><C-w>l<C-w>l<Esc>:nada<CR>/zzqqxx<CR>",
            ":estecomandonoexiste y lleva un argumento muy, muy largo para que el aviso ",
            "no quepa en el toast y se parta en varias filas<CR>",
        ))],
    ),
    ("toasts-historial", &[Step::Keys(":messages<CR>")]),
    ("toasts-historial-filtrado", &[Step::Keys("E486")]),
    ("toasts-de-nuevo", &[Step::Keys("<CR>")]),
    // Renombrar desde el árbol una nota con muchos links (las reuniones y
    // `listas.md` enlazan a `Metalmind/plan`; con el filtro, es la última
    // fila del árbol): la vista previa con el renombre primero y elegido (su
    // ruta nueva entera abajo), una línea quitada, `x` sobre el renombre,
    // que no se quita, lo aplicado, deshecho con `:vaultundo` y otro
    // `:vaultundo` sin nada que deshacer.
    (
        "renombrar-nota",
        &[Step::Keys(concat!(
            "<Esc><Space>e/Metalmind/plan<CR>Gr<BS><BS><BS><BS>",
            "plan del proyecto con un nombre largo para ver como se recorta<CR>",
        ))],
    ),
    ("renombrar-nota-quitado", &[Step::Keys("jx")]),
    ("renombrar-nota-fijo", &[Step::Keys("kx")]),
    ("renombrar-nota-aplicado", &[Step::Keys("<CR>")]),
    (
        "renombrar-nota-deshecho",
        &[Step::Keys("<C-w>l:vaultundo<CR>")],
    ),
    ("nada-que-deshacer", &[Step::Keys(":vaultundo<CR>")]),
    // El ancho de lectura (#172) sin `wrap`: la línea 12 pasa de la columna,
    // el bloque de código y la selección `V` llegan a su borde, y con `$`
    // la vista se corre a lo ancho dentro de ella.
    (
        "ancho-de-lectura-sin-wrap",
        &[Step::Keys(
            "<Esc><Esc><C-w>o<Space>flayout bastante<CR>:set nowrap<CR>16GVj",
        )],
    ),
    ("ancho-de-lectura-a-lo-ancho", &[Step::Keys("<Esc>12G$")]),
];

/// Sin permiso de escribir en su carpeta, el autoguardado de la nota falla.
fn read_only(notes: &Path) {
    set_writable(&notes.join(vault::DEEP_FOLDER_2), false);
}

/// Otro programa cambia la nota, que tiene cambios sin guardar. La carpeta
/// sigue sin permiso de escritura hasta el final del caso: un reintento del
/// autoguardado que leyó el disco justo antes no llega a escribir encima.
fn changed_outside(notes: &Path) {
    let note = notes.join(vault::DEEP_FOLDER_2).join(vault::DEEP_NOTE);
    fs::write(note, "Editada desde otro equipo.\n").unwrap();
}

fn set_writable(folder: &Path, writable: bool) {
    let mode = if writable { 0o755 } else { 0o555 };
    fs::set_permissions(folder, fs::Permissions::from_mode(mode)).unwrap();
}

/// Alto de una fila del árbol y del encabezado, y dónde empieza el panel,
/// con la fuente por defecto: para hacer clic sobre una fila.
const TREE_ROW: i16 = 26;
const TREE_LEFT: i16 = 36;
const TREE_WIDTH: i16 = 260;
/// Con `editor.padding` por defecto (12 a los lados, 8 arriba) y líneas de
/// texto de 21: la columna de las marks y el centro de cada línea sin heading.
const EDITOR_LEFT: i16 = TREE_LEFT + TREE_WIDTH;
const MARK_X: i16 = EDITOR_LEFT + 12 + 4;
const TEXT_TOP: i16 = 8;
const TEXT_LINE: i16 = 21;

#[rstest]
#[case::oscuro_1920("oscuro", 1920, 1080)]
#[case::oscuro_1280("oscuro", 1280, 720)]
#[case::claro_1920("claro", 1920, 1080)]
#[case::claro_1280("claro", 1280, 720)]
#[ignore = "necesita Xvfb y Vulkan: ver docs/testing.md"]
fn capturas(#[case] theme: &str, #[case] width: u16, #[case] height: u16) {
    let out = out_dir();
    let smoke = Smoke::with_screen_at("1920x1080", &format!("metalmind-capturas-{theme}-{width}"));
    prepare(&smoke, theme);
    let logs = tempfile::tempdir().unwrap();
    let frames = logs.path().join("frames.log");
    let frame_log = frames.to_str().unwrap();
    let mut metalmind = smoke.launch(&["--frame-log", frame_log, vault::NOTE]);
    let window = metalmind.window();
    let rest = (
        i16::try_from(width).unwrap() - 2,
        i16::try_from(height).unwrap() - 2,
    );
    let mut seen = frame_count(&frames);
    let mark_color = gutter_mark_color(theme);
    window.resize(width.into(), height.into());
    let mut report = Vec::new();
    let mut previous = None;
    let mut titles = String::new();
    for (scene, steps) in SCENES {
        for step in *steps {
            match step {
                Step::Keys(keys) => window.type_keys(keys),
                Step::Search => window.type_keys(vault::SEARCHED),
                Step::SearchAfterCjk => {
                    window.type_keys(&format!("<C-u>{}", vault::SEARCHED_AFTER_CJK));
                }
                Step::RightClickRow(row) => {
                    window.right_click(TREE_LEFT + 80, TREE_ROW * (row + 1) + TREE_ROW / 2);
                }
                Step::RightClickBottomRow => {
                    window.right_click(TREE_LEFT + TREE_WIDTH - 8, rest.1 - TREE_ROW / 2);
                }
                Step::HoverMark(at) => {
                    // El tooltip sale tras una demora con el ratón quieto, más
                    // larga que la quietud de `settle`: se espera a verlo.
                    let still = settle(&window, &frames, seen, previous.as_ref());
                    seen = frame_count(&frames);
                    let (x, y) = mark_position(*at, &still, mark_color);
                    window.move_pointer(x, y);
                    wait::until("el tooltip de la mark", TIMEOUT, || {
                        if window.pixels().2 == still.rgb {
                            Err("sin tooltip".to_owned())
                        } else {
                            Ok(())
                        }
                    });
                }
                Step::FormulasZone => {
                    window.type_keys(&format!(":{}<CR>zb?aqui<CR>", vault::FORMULAS_BROKEN));
                }
                Step::HoverBrokenFormula => {
                    let still = settle(&window, &frames, seen, previous.as_ref());
                    if hover_broken_formula(&window, &still, theme, rest) {
                        seen = frame_count(&frames);
                        wait::until("el tooltip de la fórmula rota", TIMEOUT, || {
                            if window.pixels().2 == still.rgb {
                                Err("sin tooltip".to_owned())
                            } else {
                                Ok(())
                            }
                        });
                    }
                }
                Step::Files(change) => change(&smoke.notes()),
                Step::Settle => {
                    // Los archivos que llegan pueden no cambiar lo que se ve:
                    // aquí no hace falta que difiera de la escena anterior.
                    settle(&window, &frames, seen, None);
                    seen = frame_count(&frames);
                }
            }
        }
        if !matches!(
            steps.last(),
            Some(
                Step::RightClickRow(_)
                    | Step::RightClickBottomRow
                    | Step::HoverMark(_)
                    | Step::HoverBrokenFormula
            )
        ) {
            window.move_pointer(rest.0, rest.1);
        }
        let image = settle(&window, &frames, seen, previous.as_ref());
        seen = frame_count(&frames);
        assert_eq!((image.width, image.height), (width, height), "{scene}");
        let name = format!("{scene}-{theme}-{width}");
        image.save(&out.join(format!("{name}.png")));
        report.push(compare(&out, &name, &image));
        writeln!(titles, "{name}: {}", window.title()).unwrap();
        previous = Some(image);
    }
    drop(metalmind);
    // Para que la carpeta del caso se pueda borrar.
    set_writable(&smoke.notes().join(vault::DEEP_FOLDER_2), true);
    // El título no sale en la captura: sin gestor de ventanas no hay barra de título.
    fs::write(out.join(format!("titulos-{theme}-{width}.txt")), titles).unwrap();
    finish(&out, &format!("{theme}-{width}"), &report);
}

/// Dónde dejar el ratón para ver el tooltip de una mark en `image`.
fn mark_position(at: MarkAt, image: &Image, mark_color: [u8; 3]) -> (i16, i16) {
    match at {
        MarkAt::Line(line) => (MARK_X, TEXT_TOP + TEXT_LINE * line + TEXT_LINE / 2),
        MarkAt::Lowest => (MARK_X, lowest_mark(image, MARK_X, mark_color)),
        MarkAt::RightmostPane => {
            let editors = i16::try_from(image.width).unwrap() - EDITOR_LEFT;
            let pane_left = EDITOR_LEFT + editors * 3 / 4;
            (pane_left + MARK_X - EDITOR_LEFT, TEXT_TOP + TEXT_LINE / 2)
        }
    }
}

/// La fila más baja con la letra de una mark en la columna de `x`: se busca
/// su color en la imagen, porque dónde termina el texto depende de la ventana.
fn lowest_mark(image: &Image, x: i16, mark_color: [u8; 3]) -> i16 {
    let width = usize::from(image.width);
    let columns = usize::try_from(x - 4).unwrap()..usize::try_from(x + 6).unwrap();
    (0..usize::from(image.height))
        .rev()
        .find(|&row| {
            columns.clone().any(|column| {
                let at = (row * width + column) * 3;
                image.rgb[at..at + 3] == mark_color
            })
        })
        .map(|row| i16::try_from(row).unwrap() - 4)
        .expect("ninguna mark en la columna")
}

/// El color de `ui.gutter_mark` en el tema de fábrica `theme`.
fn gutter_mark_color(theme: &str) -> [u8; 3] {
    let dir = tempfile::tempdir().unwrap();
    let loaded = Themes::new(dir.path().to_owned())
        .load(&format!("metalmind-{theme}"))
        .unwrap();
    let [_, red, green, blue] = loaded.value.ui.gutter_mark.rgb.to_be_bytes();
    [red, green, blue]
}

/// Distancia en píxeles entre dos tramos del color de error que ya no son
/// la misma fórmula rota: entre una y otra va la palabra `texto`.
const FORMULA_GAP: usize = 24;

/// Pone el ratón sobre la última fórmula rota que se ve entera en `image`
/// (la de más a la derecha puede estar cortada por el borde, y una cortada
/// no dice nada). Devuelve si había alguna: en las capturas de antes, sin
/// fórmulas compuestas, no hay nada que señalar.
fn hover_broken_formula(window: &Window<'_>, image: &Image, theme: &str, rest: (i16, i16)) -> bool {
    let error = Themes::builtin()
        .load(&format!("metalmind-{theme}"))
        .unwrap()
        .value
        .ui
        .error
        .rgb;
    let [_, red, green, blue] = error.to_be_bytes();
    let width = usize::from(image.width);
    let pixels: Vec<(usize, usize)> = image
        .rgb
        .as_chunks::<3>()
        .0
        .iter()
        .enumerate()
        .filter(|(_, pixel)| **pixel == [red, green, blue])
        .map(|(index, _)| (index % width, index / width))
        .collect();
    let Some(&(_, lowest)) = pixels.iter().max_by_key(|(_, y)| *y) else {
        return false;
    };
    // La última fila de la línea de las fórmulas rotas, de izquierda a
    // derecha: partida, las de arriba tienen fórmulas en otras columnas.
    let row = usize::try_from(TEXT_LINE / 2).unwrap();
    let mut columns: Vec<usize> = pixels
        .iter()
        .filter(|(_, y)| lowest - y < row)
        .map(|(x, _)| *x)
        .collect();
    columns.sort_unstable();
    columns.dedup();
    let mut formulas: Vec<(usize, usize)> = Vec::new();
    for x in columns {
        match formulas.last_mut() {
            Some((_, right)) if x - *right < FORMULA_GAP => *right = x,
            _ => formulas.push((x, x)),
        }
    }
    let &(left, right) = formulas
        .iter()
        .rev()
        .find(|(_, right)| right + FORMULA_GAP < width)
        .expect("una fórmula rota que se vea entera");
    window.move_pointer(rest.0, rest.1);
    let at = |value: usize| i16::try_from(value).unwrap();
    window.move_pointer(at(usize::midpoint(left, right)), at(lowest) - 4);
    true
}

fn out_dir() -> PathBuf {
    let out = env::var_os(OUT_VAR).map_or_else(
        || Path::new(env!("CARGO_TARGET_TMPDIR")).join("capturas"),
        PathBuf::from,
    );
    fs::create_dir_all(&out).unwrap();
    out
}

/// El vault, el tema y las carpetas del árbol abiertas.
fn prepare(smoke: &Smoke, theme: &str) {
    vault::write(&smoke.notes());
    let home = smoke.home();
    let config = home.join(".config/metalmind");
    fs::create_dir_all(&config).unwrap();
    fs::write(
        config.join("config.yaml"),
        // Un delay corto: el autoguardado actúa antes de que la ventana se calme.
        format!(
            "theme: metalmind-{theme}\neditor:\n  autosave_delay_ms: 100\ndaily:\n  enabled: true\n  format: '[hoy]'\n  template: diario\n"
        ),
    )
    .unwrap();
    let paths = Paths::from_xdg(|_| None, &home);
    let state = paths.vault_state(&smoke.notes().canonicalize().unwrap());
    fs::create_dir_all(&state).unwrap();
    let mut expanded = String::new();
    for folder in vault::EXPANDED {
        writeln!(expanded, "    - {folder}").unwrap();
    }
    fs::write(
        state.join("ui.yaml"),
        // La lista empieza plegada: las pantallas de antes no la tienen.
        format!(
            "panel:\n  visible: true\n  view: folders\n  expanded:\n{expanded}list:\n  visible: false\n"
        ),
    )
    .unwrap();
}

fn frame_count(frames: &Path) -> usize {
    fs::read_to_string(frames).map_or(0, |log| log.lines().count())
}

/// Espera a que la ventana pinte algo después de `before` frames y deje de
/// pintar, y devuelve lo que se ve. Que el registro de frames esté quieto no
/// basta: con lavapipe, un frame registrado puede no verse aún (#116). Por
/// eso la imagen tampoco puede ser `previous`, la escena anterior.
fn settle(window: &Window<'_>, frames: &Path, before: usize, previous: Option<&Image>) -> Image {
    wait::until("un frame nuevo", TIMEOUT, || {
        let count = frame_count(frames);
        if count > before {
            Ok(())
        } else {
            Err(format!("{count} frames"))
        }
    });
    let mut last = (frame_count(frames), Instant::now());
    wait::until("que la ventana pinte y deje de pintar", TIMEOUT, || {
        let count = frame_count(frames);
        if count != last.0 {
            last = (count, Instant::now());
        }
        if last.1.elapsed() < QUIET {
            return Err(format!("{count} frames"));
        }
        let (width, height, rgb) = window.pixels();
        let image = Image { width, height, rgb };
        not_yet_painted(&image, previous).map_or(Ok(image), |why| Err(why.to_owned()))
    })
}

/// Por qué `image` todavía no puede ser la escena que sigue a `previous`.
fn not_yet_painted(image: &Image, previous: Option<&Image>) -> Option<&'static str> {
    if support::one_color(&image.rgb) {
        Some("de un solo color")
    } else if previous == Some(image) {
        Some("igual a la escena anterior")
    } else {
        None
    }
}

/// Una línea del resumen, y la imagen de lo que cambió si cambió algo.
fn compare(out: &Path, name: &str, image: &Image) -> (String, usize) {
    let Some(before) = env::var_os(BEFORE_VAR).map(PathBuf::from) else {
        return (format!("| {name} | sin comparar |"), 0);
    };
    let path = before.join(format!("{name}.png"));
    if !path.exists() {
        // Sin la de antes no hay comparación: cuenta como si cambiara todo.
        let pixels = usize::from(image.width) * usize::from(image.height);
        return (format!("| {name} | no hay captura de antes |"), pixels);
    }
    match Image::load(&path).diff(image) {
        None => (format!("| {name} | 0 |"), 0),
        Some((changed, marked)) => {
            marked.save(&out.join(format!("diff-{name}.png")));
            (
                format!("| {name} | {changed} (`diff-{name}.png`) |"),
                changed,
            )
        }
    }
}

/// Escribe las filas del resumen del caso (el script les pone el
/// encabezado) y falla si alguna captura cambió.
fn finish(out: &Path, case: &str, report: &[(String, usize)]) {
    let mut summary = String::new();
    for (line, _) in report {
        writeln!(summary, "{line}").unwrap();
    }
    fs::write(out.join(format!("resumen-{case}.md")), &summary).unwrap();
    let changed: Vec<&str> = report
        .iter()
        .filter(|(_, changed)| *changed > 0)
        .map(|(line, _)| line.as_str())
        .collect();
    assert!(
        changed.is_empty(),
        "cambiaron píxeles:\n{}",
        changed.join("\n")
    );
}

#[rstest]
#[case::de_un_solo_color([[0, 0, 0], [0, 0, 0]], None, Some("de un solo color"))]
#[case::la_primera([[0, 0, 0], [9, 9, 9]], None, None)]
#[case::igual_a_la_anterior([[0, 0, 0], [9, 9, 9]], Some([[0, 0, 0], [9, 9, 9]]), Some("igual a la escena anterior"))]
#[case::distinta_de_la_anterior([[0, 0, 0], [9, 9, 9]], Some([[0, 0, 0], [7, 7, 7]]), None)]
fn una_captura_es_de_la_escena_nueva_solo_si_ya_pinto(
    #[case] pixels: [[u8; 3]; 2],
    #[case] previous: Option<[[u8; 3]; 2]>,
    #[case] expected: Option<&str>,
) {
    let image = |pixels: [[u8; 3]; 2]| Image {
        width: 2,
        height: 1,
        rgb: pixels.as_flattened().to_vec(),
    };
    let previous = previous.map(image);
    assert_eq!(not_yet_painted(&image(pixels), previous.as_ref()), expected);
}
