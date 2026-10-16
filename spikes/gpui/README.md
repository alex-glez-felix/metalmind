# Spike de GPUI

Responde una pregunta: **¿GPUI aguanta lo que nota necesita en Linux/Wayland?** Archivo de 50 MB con scroll fluido, teclado real (teclas muertas, IME, atajos) traducido a `nota-vim`, portapapeles con selección primaria, y números medidos en vez de impresiones.

Es un proyecto aparte, fuera del workspace: no entra al CI ni a las dependencias del núcleo. Usa `gpui-kit =0.7.3` (gpui-component sobre `gpui-pre =0.3.9`), la variante que recomienda `docs/research/02-gui-stack.md`; el porqué está en `docs/research/04-spike-gpui.md`.

Tiene dos vistas:

- **vim**: la que importa. Nuestro `nota-buffer` + `nota-vim` con un render básico hecho a mano: pinta solo las líneas visibles.
- **kit**: el editor de gpui-component, para comparar.

## Preparar la máquina de pruebas (una sola vez)

```sh
# Rust, si no está
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Librerías que GPUI necesita para compilar y abrir ventanas
sudo pacman -S --needed base-devel fontconfig freetype2 libxkbcommon libxkbcommon-x11 wayland libxcb vulkan-icd-loader
```

Con el driver de NVIDIA instalado, si `vulkaninfo --summary` (paquete `vulkan-tools`) muestra la GPU NVIDIA, está listo.

## Correr

Desde la raíz del repo:

```sh
git pull && ./spikes/gpui/run.sh
```

La primera vez compila todo GPUI (en una máquina sin GPU y con `-j3` tardó 27 min; en un escritorio actual debería ser varias veces más rápido) y genera `spikes/gpui/fixtures/grande.md` de 50 MB. Las siguientes corridas arrancan al toque.

Otras formas:

| Comando | Qué abre |
|---|---|
| `./spikes/gpui/run.sh` | vista vim con el archivo de 50 MB |
| `./spikes/gpui/run.sh vim chico` | vista vim con uno de 2 MB |
| `./spikes/gpui/run.sh vim grande --bench` | scroll automático 10 s, imprime el reporte y sale |
| `./spikes/gpui/run.sh kit chico` | editor de gpui-component, 2 MB |
| `./spikes/gpui/run.sh kit grande` | editor de gpui-component, 50 MB |
| `./spikes/gpui/run.sh kit grande --bench` | benchmark con el editor de gpui-component |

Para salir: `ctrl-q` o cerrar la ventana. Al salir se imprime un **reporte** en markdown con los tiempos, y queda guardado en `spikes/gpui/resultados/` junto con el log (`.log`) de la corrida.

## Qué vas a ver

Arriba, el texto. Abajo, una franja gris (el overlay) con:

1. **Línea 1**: modo de vim (`NORMAL`, `INSERT`, `OPERADOR`), línea y columna del cursor, cuánto tardó en cargar el archivo y cuánto en aparecer el primer frame.
2. **Línea 2**: frames dibujados, **intervalo entre frames** (p50 y p99 de los últimos 240, con los fps que implica), **CPU por frame** (p50, p99 y el último).
3. **`preedit`**: el texto provisional mientras escribes con una tecla muerta o el IME.
4. **Eventos**: las últimas 8 teclas, del más nuevo al más viejo. Cada una dice qué entregó GPUI y qué hizo nota-vim con ella:
   - `a Some("a") → Char('a') → vim::insert_after`: GPUI dio `a`, nota-vim ejecutó esa acción.
   - `a Some("a") → texto, va por el input handler` y luego `texto "a" (input handler) → insertado`: en Insert el texto entra por el camino del IME, que es el mismo que usan las teclas muertas.
   - `... → sin equivalente en nota-vim`: tecla que el motor no conoce (flechas, F1…). No es error.
   - Las líneas en rojo empiezan con `ERROR` y son bugs: anótalas.

Cómo leer los números:

- **Intervalo entre frames**: mientras haces scroll, cuánto pasa entre un frame y el siguiente. Lo ideal es `1000 / Hz` de tu monitor: 6.94 ms a 144 Hz, 6.06 ms a 165 Hz, 16.7 ms a 60 Hz. Las pausas de más de 250 ms (no estás tocando nada) no cuentan.
- **CPU por frame**: cuánto trabaja la app para armar un frame, sin contar la GPU. Tiene que ser bastante menor que el intervalo; si se le acerca, se van a perder frames.
- **p50 / p99**: el valor típico y el del 1 % peor. Un p99 mucho mayor que el p50 son tirones.

## Teclas de la vista vim

Es nota-vim tal cual está en `main`: `h j k l`, `w W e E b B`, `0 ^ $`, `gg G`, `x`, `d c y` con motions y `dd cc yy`, `iw aw i( a( i" a"`, `p P`, `u`, `ctrl-r`, `i a I A o O`, `Esc`. No hay modo visual ni `:`.

Además, propias del spike:

| Tecla | Acción |
|---|---|
| arrastrar con el ratón | selecciona y copia a la **selección primaria** |
| clic | mueve el cursor |
| clic medio | pega la selección primaria donde hiciste clic |
| `ctrl-shift-c` | copia la selección del ratón (o la línea del cursor) al portapapeles |
| `ctrl-shift-v` | pega el portapapeles en el cursor |
| `ctrl-q` | sale e imprime el reporte |

## Checklist

Hazla en orden; anota cada resultado en `docs/research/04-spike-gpui.md` (mismos números). Si algo no se puede probar, anota por qué y sigue.

### A. Arranque y GPU

1. **GPU correcta.** Corre `./spikes/gpui/run.sh vim chico` y cierra. Busca en la terminal (o en el `.log`) la línea `Selected GPU`.
   - Pasa: dice `NVIDIA GeForce RTX` con backend `Vulkan`.
   - Falla: dice `llvmpipe`, `Software` o no aparece. Anota todas las líneas `Found … GPU adapter(s)` que salgan.
2. **Arranque con archivo chico.** En el reporte de esa misma corrida, mira `arranque → primer frame`.
   - Pasa: menos de 300 ms. Aceptable hasta 600 ms. Falla: más.
3. **Arranque con 50 MB.** `./spikes/gpui/run.sh` y cierra apenas aparezca el texto. Anota `carga del archivo` y `arranque → primer frame`.
   - Pasa: primer frame en menos de 1 s, y la mayor parte es `carga del archivo` (leer el disco y armar el rope, que no es GPUI).
   - Falla: más de 2 s, o la ventana queda en blanco un rato visible.

### B. Archivo grande (vista vim, 50 MB)

4. **Scroll manual.** Con `./spikes/gpui/run.sh`, haz scroll con la rueda o el touchpad durante unos 20 s, rápido y lento. Mira la línea 2 del overlay mientras lo haces.
   - Pasa: los fps del intervalo están cerca de los Hz del monitor, el p99 no pasa del doble del p50 y no ves saltos ni texto que tiembla.
   - Falla: p50 por encima de 16.7 ms, tirones visibles, o texto que se corre medio píxel al llegar al final.
5. **Saltos.** En Normal, `G` (al final del archivo) y `gg` (al principio), varias veces. Mira `CPU … último` justo después de cada salto. Al final del archivo, haz scroll un poco: el texto tiene que verse nítido y quieto.
   - Pasa: el salto se ve instantáneo y el último frame cuesta menos de 16 ms.
   - Falla: el salto se nota, o el texto tiembla al final.
6. **Benchmark.** `./spikes/gpui/run.sh vim grande --bench`. No toques nada: hace scroll solo 10 s y sale. Pega el reporte en 04. (En este modo GPUI no baja a 30 fps aunque la ventana pierda el foco.)
   - Pasa: `fps sostenidos` igual a los Hz del monitor (±5 %) y CPU p99 menor que la mitad del intervalo.
   - Falla: fps sostenidos por debajo de los Hz del monitor.
7. **Repetición de tecla.** Mantén `j` presionada unos segundos, luego `k`.
   - Pasa: el cursor baja parejo, sin trabarse, y al soltar se detiene en el acto.
   - Falla: se traba, acelera a saltos o sigue moviéndose después de soltar.

### C. Teclado con nota-vim

Esta es la parte central: que lo que escribes llegue bien traducido al motor vim. Usa `./spikes/gpui/run.sh vim chico`.

8. **Normal e Insert.** Prueba `w`, `b`, `$`, `0`, `dd`, `u`, `ctrl-r`, `yy` + `p`, `o` + texto + `Esc`, `ciw` + texto + `Esc`.
   - Pasa: se comporta como en nvim y el overlay muestra acciones `vim::…`.
   - Falla: alguna tecla hace otra cosa o no aparece. Copia la línea del overlay.
9. **Teclas muertas en Insert.** Entra a Insert (`i`) y escribe `´` + `a`, `´` + `e`, `~` + `n` (o la `ñ` directa si tu layout la tiene), y `´` + `k` (combinación que no existe).
   - Pasa: aparecen `á`, `é`, `ñ` y `´k`. Al tocar `´` el overlay muestra `preedit "´"`, y al completar, `texto "á" (input handler) → insertado`.
   - Falla: sale `´a`, no sale nada, o sale doble.
10. **Tecla muerta en Normal.** Solo si usas un layout con `"` muerta (`us(intl)`): en una línea con `"comillas"`, pon el cursor adentro y escribe `ci"` (en us-intl, `"` + espacio da `"`).
    - Pasa: borra lo de adentro y entra a Insert.
    - Falla: queda en `OPERADOR` o hace otra cosa. Si tu layout no tiene `"` muerta, anota "no aplica".
11. **AltGr.** En Insert, escribe los caracteres que en tu layout van con AltGr (en latam: `@` `{` `}` `[` `]` `\` `|`).
    - Pasa: todos aparecen tal cual.
    - Falla: alguno no sale o el overlay dice `sin equivalente en nota-vim`.
12. **Atajos con modificadores.** En Normal: `u` y luego `ctrl-r`. En Insert: `tab` y `ctrl-c`.
    - Pasa: `ctrl-r` rehace; `tab` inserta un tabulador; `ctrl-c` aparece en el overlay como `Ctrl('c') → sin binding` (nota-vim aún no lo usa, pero llegó al motor).
    - Falla: `tab` mueve el foco o no hace nada, o `ctrl-c` no aparece en el overlay.
13. **IME (si tienes Fcitx5).** Actívalo (normalmente `ctrl-space`), entra a Insert y escribe algo que pase por el IME (pinyin, mozc, o el selector de emoji de Fcitx5).
    - Pasa: el texto provisional se ve subrayado en el cursor, la ventana de candidatos aparece junto al cursor y al confirmar queda en el texto.
    - Falla: no aparece el subrayado, la ventana de candidatos sale en una esquina, o el texto confirmado no llega. Si no tienes Fcitx5, anota "no probado".

### D. Portapapeles

Con `./spikes/gpui/run.sh vim chico` y un navegador o terminal al lado.

14. **De nota hacia afuera.** Selecciona una frase con acentos arrastrando el ratón y `ctrl-shift-c`. Pega en el navegador con `ctrl-v`.
    - Pasa: llega igual, con acentos. Falla: no llega o llega con caracteres raros.
15. **De afuera hacia nota.** Copia texto en el navegador (`ctrl-c`) y pégalo en nota con `ctrl-shift-v`, una vez en Normal y otra en Insert.
    - Pasa: aparece en el cursor; con `u` (después de `Esc` si estabas en Insert) se deshace. Falla: no aparece, o el overlay dice `portapapeles vacío`.
16. **Primaria hacia afuera.** Selecciona con el ratón en nota (sin copiar) y haz clic medio en la terminal.
    - Pasa: se pega lo seleccionado. Falla: no se pega nada o se pega otra cosa.
17. **Primaria hacia adentro.** Selecciona texto en la terminal o el navegador y haz clic medio en una línea de nota.
    - Pasa: se pega donde hiciste clic. Falla: no pasa nada o el overlay dice `primaria vacío`.

### E. Editor de gpui-component (para comparar)

18. **Kit con archivo chico.** `./spikes/gpui/run.sh kit chico`. Haz scroll, escribe con teclas muertas, copia con `ctrl-c` y pega con `ctrl-v`.
    - Pasa: todo funciona y se siente igual o mejor que la vista vim. Anota lo que no.
19. **Kit con 50 MB.** `./spikes/gpui/run.sh kit grande`. Espera a que aparezca el texto (dale hasta un minuto) y haz scroll.
    - Anota el primer frame y si el scroll es usable. La documentación de gpui-component dice que su editor aguanta "unas 50K líneas", y en la máquina sin GPU (render por CPU) el archivo de 2 MB ya tardó 35 s en mostrar el primer frame, así que es esperable que sufra; lo que interesa es cuánto. Si a los 3 minutos no aparece nada, ciérralo y anota "no abre".
20. **Benchmark de kit.** `./spikes/gpui/run.sh kit grande --bench` (si la 19 fue usable). Pega el reporte en 04.

### F. X11 / XWayland (opcional)

21. Repite 4, 9 y 16 forzando X11: `WAYLAND_DISPLAY= ./spikes/gpui/run.sh vim chico`.

### Contraste opcional de tiempos

GPUI puede imprimir su propia duración de cada frame (dibujo + envío a la GPU). Para comparar con el overlay:

```sh
ZED_MEASUREMENTS=1 ./spikes/gpui/run.sh vim grande --bench
```

## Cuando termines

En `spikes/gpui/resultados/` queda un `.md` (reporte) y un `.log` por corrida. Llena `docs/research/04-spike-gpui.md` y haz commit en la rama `spike/gpui`, o pásale la carpeta `resultados/` y tus notas a Claude para que lo llene.

## Código

| Archivo | Qué hace |
|---|---|
| `src/keys.rs` | traduce teclas de GPUI a `nota_vim::Key` y decide si van al motor o al input handler |
| `src/editor/` | la vista vim: elemento que pinta las líneas visibles, input handler del IME, ratón y portapapeles |
| `src/document.rs` | `nota-buffer` + `nota-vim` sin GUI: teclas, texto compuesto, pegar, selección |
| `src/metrics.rs`, `src/report.rs` | tiempos de frame y el reporte final |
| `src/shell.rs`, `src/hud.rs` | raíz de la ventana, medición por frame, overlay y benchmark |
| `src/kit.rs` | la vista con el editor de gpui-component |
| `src/fixture.rs` | generador determinista del markdown de prueba |

Tests: `cargo test` dentro de `spikes/gpui/` (no abren ventana).
