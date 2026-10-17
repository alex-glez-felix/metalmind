# 04 — Spike de GPUI: resultados

> Plantilla preparada el 2026-10-15 y llenada el 2026-10-17 con las corridas en la máquina de pruebas, siguiendo la checklist de [`spikes/gpui/README.md`](../../spikes/gpui/README.md). Cada prueba lleva el mismo número que en la checklist. Lo que no se pudo probar se marca **(no probado)** con el motivo.

## Qué se probó

| | |
|---|---|
| Fecha | 2026-10-17 |
| Commit del spike | `8a61a1d` |
| Stack | `gpui-kit =0.7.3` → `gpui-component 0.7.3` + `gpui-pre =0.3.9` (snapshot de Zed `93d7456`), wgpu 29 |
| Máquina | escritorio con GPU NVIDIA |
| Sistema | Arch Linux con Hyprland |
| Compositor | Hyprland 0.56.2 (Wayland) |
| Driver NVIDIA | 610.57.04 |
| Monitor | Samsung Odyssey G9 (G93SC), 5120×1440 a 240 Hz, escala 1 |
| Layout de teclado | `us` (sin teclas muertas); `us(intl)` puesto a mano para la prueba 9 |
| IME | Fcitx5 5.1.22, solo `keyboard-us` (sin pinyin ni mozc) |
| Primera compilación en la máquina de pruebas | 1 min 43 s (`cargo build --release`, 16 hilos) |

## Veredicto

**GPUI: sí.** La vista propia (nota-buffer + nota-vim con render a mano) abre 50 MB en menos de 300 ms, sostiene 240 fps clavados en el benchmark con 0.7 ms de CPU por frame, y teclas muertas, IME y las dos selecciones del portapapeles funcionan sin un solo `ERROR` en seis corridas. **El editor de gpui-component no sirve como base**: tardó 108 s en mostrar el primer frame con 50 MB y 4.5 s con 2 MB, bloqueando el hilo de la UI en un solo frame.

Condiciones:

- El editor se escribe nosotros, sobre GPUI directo. gpui-component queda, como mucho, para widgets que no sean el editor (listas, paneles, diálogos), y sin su `Root` (riesgo 1).
- Queda pendiente probar un IME que dibuje el preedit dentro de la app (pinyin, mozc) y X11 (prueba 21). Ninguno es bloqueante para el uso diario.

## Resultados por prueba

### A. Arranque y GPU

| # | Prueba | Resultado | Notas |
|---|---|---|---|
| 1 | GPU elegida por wgpu | ✅ | `Selected GPU (passed configuration test): NVIDIA GeForce RTX (Vulkan)`. wgpu también ve la misma GPU por GL y la descarta |
| 2 | Arranque con archivo chico | ⚠️ | primer frame 432 ms en la primera corrida (aceptable, < 600); 244 ms en una corrida posterior. La carga son 2.3 ms: casi todo es levantar ventana y Vulkan |
| 3 | Arranque con archivo de 50 MB | ✅ | carga 71–73 ms, primer frame 293–297 ms |

### B. Archivo grande (vista vim)

| # | Prueba | Resultado | Notas |
|---|---|---|---|
| 4 | Scroll manual con rueda o touchpad | ✅ | 120 fps con la rueda a 240 Hz. **No es límite del render**: GPUI solo dibuja cuando llega un evento y la rueda entrega ~120 eventos/s (el benchmark da 240). CPU p50 0.76 ms. Sin tirones visibles |
| 5 | Saltos `G` y `gg` | ✅ | instantáneos a ojo (`vim::end_of_document` / `start_of_document` en el log); peor frame de toda la corrida 18 ms |
| 6 | Benchmark de scroll | ✅ | **240 fps sostenidos**, intervalo p50 4.16 / p99 4.29 ms, CPU p99 1.05 ms (presupuesto 4.17 ms) |
| 7 | Repetición de tecla (`j` sostenida) | ✅ | baja parejo y para al soltar. Con la tecla sostenida el intervalo mide la repetición del teclado (~40/s), no el render |

Reporte de `./run.sh vim grande --bench`:

```
| métrica | valor |
|---|---|
| vista | vim |
| archivo | fixtures/grande.md (50.0 MB, 511878 líneas) |
| corrida | benchmark de scroll automático |
| carga del archivo | 71.33 ms |
| arranque → primer frame | 292.52 ms |
| frames dibujados | 2402 |
| intervalo entre frames p50 / p99 / máx | 4.16 ms / 4.29 ms / 5.70 ms (2401 muestras) |
| fps sostenidos (1000 / p50) | 240 |
| CPU por frame p50 / p99 / máx | 0.68 ms / 1.05 ms / 2.74 ms (2402 muestras) |
| XDG_SESSION_TYPE | wayland |
| XDG_CURRENT_DESKTOP | Hyprland |
| WAYLAND_DISPLAY | wayland-1 |
| DISPLAY | :0 |
```

### C. Teclado con nota-vim

| # | Prueba | Resultado | Notas |
|---|---|---|---|
| 8 | Normal e Insert básicos | ✅ | `w b $ 0 dd u ctrl-r yy p o ciw` se comportan como en nvim |
| 9 | Teclas muertas en Insert (`´`+`a`, `~`+`n`) | ✅ | con `us(intl)`: `preedit "´"` y luego `texto "á" (input handler) → insertado`; igual `~`+`n` → `ñ`. `'`+`k` da `ḱ` (U+1E31, sí existe en la tabla de composición; Telegram hace lo mismo) y `` ` ``+`k` da `` `k ``. Sin dobles ni perdidas. Con `us` a secas no hay teclas muertas y `` ` ``+`a` da `` `a `` en cualquier app: no es fallo |
| 10 | Tecla muerta en Normal (`"` en us-intl) | (no probado) | se volvió a `us` antes de llegar a ella |
| 11 | AltGr (`@`, `{`, `[`…) | (no aplica) | `us` no usa AltGr para esos caracteres |
| 12 | `ctrl-r`, `ctrl-c`, `tab` llegan al motor | ⚠️ | `ctrl-r` ✅ (`Ctrl('r') → vim::redo`, 12 veces). `tab` y `ctrl-c` en Insert no aparecen en ningún log: se tomaron como probados, pero no hay evidencia. Repetir |
| 13 | IME (Fcitx5) | ✅ | selector de Unicode de Fcitx5 (`ctrl-alt-shift-u`, "heart"): `texto "❤️" (input handler) → insertado` una sola vez, y la ventana de candidatos sale pegada al cursor (el rectángulo del cursor llega al compositor). No hay `preedit` en la app porque ese selector dibuja su texto provisional en su propio panel. Falta un IME que haga preedit dentro de la app |

Teclas que no se tradujeron bien: ninguna. Las únicas `sin equivalente en nota-vim` fueron teclas que el motor no conoce (`scroll_lock`).

### D. Portapapeles

| # | Prueba | Resultado | Notas |
|---|---|---|---|
| 14 | Copiar de nota a otra app (`ctrl-shift-c`) | ✅ | `portapapeles ← 1579 bytes` |
| 15 | Pegar de otra app en nota (`ctrl-shift-v`) | ✅ | `portapapeles → 101 bytes pegados` |
| 16 | Primaria: de nota a otra app (clic medio afuera) | ✅ | `primaria ← …` al arrastrar |
| 17 | Primaria: de otra app a nota (clic medio en nota) | ✅ | `primaria → 101 bytes pegados` |

### E. Editor de gpui-component (comparación)

| # | Prueba | Resultado | Notas |
|---|---|---|---|
| 18 | `kit chico`: scroll, teclas muertas, copiar y pegar | ⚠️ | fluido una vez abierto (121 fps con rueda, CPU p50 1.02 ms), pero **primer frame 4452 ms** con un solo frame de 4235 ms. Una vez, el selector de Unicode de Fcitx5 apareció arriba a la izquierda en vez de junto al cursor; las siguientes salió bien. En la vista propia nunca pasó |
| 19 | `kit grande`: abre y hace scroll | ❌ | **primer frame 107 816 ms** (un frame de 107 519 ms). Escala lineal con el tamaño (~25× el de 2 MB). No usable |
| 20 | Benchmark `kit grande --bench` | (no probado) | la 19 no fue usable |

`kit chico`:

```
| métrica | valor |
|---|---|
| vista | kit |
| archivo | fixtures/chico.md (2.0 MB, 21063 líneas) |
| corrida | manual |
| carga del archivo | 3.74 ms |
| arranque → primer frame | 4452.13 ms |
| frames dibujados | 3963 |
| intervalo entre frames p50 / p99 / máx | 8.29 ms / 204.07 ms / 249.62 ms (3769 muestras) |
| fps sostenidos (1000 / p50) | 121 |
| CPU por frame p50 / p99 / máx | 1.02 ms / 1.54 ms / 4235.19 ms (3963 muestras) |
| XDG_SESSION_TYPE | wayland |
| XDG_CURRENT_DESKTOP | Hyprland |
| WAYLAND_DISPLAY | wayland-1 |
| DISPLAY | :0 |
```

`kit grande`:

```
| métrica | valor |
|---|---|
| vista | kit |
| archivo | fixtures/grande.md (50.0 MB, 511878 líneas) |
| corrida | manual |
| carga del archivo | 86.33 ms |
| arranque → primer frame | 107815.78 ms |
| frames dibujados | 331 |
| intervalo entre frames p50 / p99 / máx | 8.68 ms / 65.59 ms / 65.59 ms (12 muestras) |
| fps sostenidos (1000 / p50) | 115 |
| CPU por frame p50 / p99 / máx | 1.47 ms / 5.04 ms / 107519.05 ms (331 muestras) |
| XDG_SESSION_TYPE | wayland |
| XDG_CURRENT_DESKTOP | Hyprland |
| WAYLAND_DISPLAY | wayland-1 |
| DISPLAY | :0 |
```

### F. X11 / XWayland (opcional)

| # | Prueba | Resultado | Notas |
|---|---|---|---|
| 21 | Repetir 4, 9 y 16 con `WAYLAND_DISPLAY=` | (no probado) | opcional |

## Comparación de números

| Métrica | vim (propia) | kit (gpui-component) |
|---|---|---|
| Carga de 50 MB | 71 ms | 86 ms |
| Arranque → primer frame, 50 MB | 293 ms | 107 816 ms |
| Arranque → primer frame, 2 MB | 432 ms | 4 452 ms |
| Intervalo p50 / p99 en benchmark | 4.16 / 4.29 ms | (no corrido) |
| fps sostenidos | 240 (benchmark) | 121 con rueda, 2 MB |
| CPU por frame p50 / p99 | 0.68 / 1.05 ms | 1.02 / 1.54 ms (2 MB) |
| Peor frame | 18 ms | 107 519 ms |

## Riesgos que ya se vieron al integrar (antes de correrlo)

Detectados al escribir el spike en la máquina sin GPU, leyendo el código de `gpui-pre 0.3.9` y `gpui-component 0.7.3`:

1. **El `Root` de gpui-kit se queda con `tab`, `shift-tab` y `ctrl-c`.** Los liga en su contexto y GPUI despacha los bindings antes que los listeners de tecla, así que un editor modal dentro de `Root` nunca los ve. El spike abre la vista vim sin `Root`. Para nota: o la raíz es propia, o hay que sobreescribir esos bindings en el contexto del editor.
2. **El editor de gpui-component no está pensado para 50 MB.** Su propia documentación de `EditorState` dice "handles large text up to about 50K lines"; el archivo de prueba tiene 512K. En el smoke test de la máquina sin GPU, con un archivo de solo 2 MB (21K líneas), tardó 35 s en mostrar el primer frame, contra 0.7 s de la vista propia con 50 MB. Es render por CPU en una máquina modesta, pero la diferencia de dos órdenes de magnitud apunta a su modelo de layout (display map y wrap de todo el documento), no a GPUI. Las pruebas 18 a 20 dicen cuánto pesa en hardware real.
3. **gpui-component no implementa la selección primaria.** Ni `gpui-base` ni `gpui-component` llaman a `write_to_primary`/`read_from_primary`; la primaria solo funciona en la vista propia. GPUI sí la soporta en Wayland (`zwp_primary_selection_v1`) y X11.
4. **Teclas muertas e IME llegan por dos caminos.** En Wayland, xkb compone `´`+`a` y GPUI lo entrega como `KeyDown` con `key_char = "á"`; el preedit de la tecla muerta llega por el input handler. Las commits de IME de un byte llegan como `KeyDown`, las multibyte (p. ej. `ñ` desde Fcitx5) directo al input handler. El spike unifica ambos: en Insert el texto siempre entra por el input handler; en Normal la tecla va directa al motor.
5. **GPUI no implementa `delete_surrounding_text` de `text-input-v3`.** Solo maneja commit, preedit y done. Un IME que reemplace texto alrededor del cursor no va a funcionar; el spike lo registra en el overlay si pasa.
6. **Scroll con `f32`.** Con un millón de líneas la altura total pasa de 2²⁴ px y un `f32` ya no distingue un píxel. La vista propia guarda el scroll en líneas (`f64`); cualquier lista de GPUI que use offsets absolutos en píxeles va a temblar al final de un archivo enorme.
7. **No hay un hook público "después del present".** El tiempo de CPU por frame que mide el spike va del `render` de la raíz al último `paint` y no incluye el envío a la GPU. Como contraste, GPUI imprime su propia duración de frame (draw + present) con `ZED_MEASUREMENTS=1`.
8. **GPUI baja a 30 fps las ventanas sin foco** (`WindowOptions::inactive_frame_interval`, 33 ms por defecto). Es buena política de energía, pero nota tiene que decidirla a propósito; el benchmark del spike la desactiva para medir el render.
9. **Ecosistema:** `gpui` en crates.io sigue en 0.2.2 (2025-10-22); el camino viable es `gpui-pre` vía gpui-kit, que publica cada semana. Fijar con `=` es obligatorio: gpui-kit 0.7.2 y 0.7.3 salieron el mismo día.

## Smoke test sin GPU

Para comprobar que el spike arranca, pinta y sale con el reporte antes de llevarlo a una máquina con GPU, se corrió en la máquina sin GPU bajo `xvfb-run` con lavapipe (Vulkan por CPU, X11). Los tiempos de frame no significan nada: Xvfb reporta 0 Hz de refresco y GPUI cae a un intervalo fijo de ~80 ms. Lo útil son los tiempos de carga y de primer frame, y que no haya pánicos ni errores.

| Corrida | Carga | Primer frame | CPU por frame p50 / p99 |
|---|---|---|---|
| `vim chico --bench` (2 MB) | 20 ms | 1082 ms | 2.26 / 11.33 ms |
| `vim grande --bench` (50 MB) | 338 ms | 710 ms | 2.35 / 13.45 ms |
| `kit chico --bench` (2 MB) | 24 ms | 35 104 ms | 4.65 / 13.80 ms |

El primer frame de la corrida chica incluye inicializar lavapipe la primera vez. wgpu eligió `llvmpipe (Vulkan)` como único adaptador, que es lo esperado en la máquina sin GPU; en la máquina de pruebas la prueba 1 tiene que mostrar la GPU NVIDIA.

## Decisiones que salen del spike

- **GPUI se queda como stack de GUI** (`gpui-pre` vía `gpui-kit`, fijado con `=`).
- **El editor es propio**: nota-buffer + nota-vim + un elemento que pinta solo las líneas visibles, como la vista vim del spike. El `EditorState` de gpui-component se descarta: su costo de abrir crece lineal con el documento y bloquea el hilo de la UI (108 s con 50 MB).
- **Nada de `Root` de gpui-kit alrededor del editor** (riesgo 1). Si se usan widgets de gpui-component, en una raíz propia o con esos bindings sobreescritos.
- **En Insert el texto entra siempre por el input handler** (riesgo 4): con esto funcionaron teclas muertas e IME sin casos especiales.
- **Scroll guardado en líneas, no en píxeles** (riesgo 6), como en el spike.
- **Pendientes que no bloquean M1**: un IME con preedit dentro de la app, `tab`/`ctrl-c` en Insert (prueba 12), la 10 con `us(intl)` y X11 (21).
- Nota aparte: la rueda del mouse entrega ~120 eventos/s; en monitores de 240 Hz el scroll con rueda se verá a 120 aunque el render dé más. Si molesta, la solución es scroll suave interpolado (animar entre eventos), no optimizar el render.
