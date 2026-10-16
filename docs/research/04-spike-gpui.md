# 04 — Spike de GPUI: resultados

> Plantilla preparada el 2026-10-15; los resultados se llenan en pc-oma siguiendo la checklist de [`spikes/gpui/README.md`](../../spikes/gpui/README.md). Cada prueba lleva el mismo número que en la checklist. Lo que no se pudo probar se marca **(no probado)** con el motivo.

## Qué se probó

| | |
|---|---|
| Fecha | |
| Commit del spike | `git rev-parse --short HEAD` |
| Stack | `gpui-kit =0.7.3` → `gpui-component 0.7.3` + `gpui-pre =0.3.9` (snapshot de Zed `93d7456`), wgpu 29 |
| Máquina | pc-oma: Ryzen 7 8700F, RTX 4070 |
| Sistema | Omarchy (Arch), kernel `uname -r` |
| Compositor | Hyprland `hyprctl version \| head -1` |
| Driver NVIDIA | `nvidia-smi --query-gpu=driver_version --format=csv,noheader` |
| Monitor | resolución, Hz, escala de Hyprland |
| Layout de teclado | p. ej. `latam`, `es`, `us(intl)` |
| IME | Fcitx5 sí/no, versión |
| Primera compilación en pc-oma | minutos |

## Veredicto

**GPUI: sí / sí con condiciones / no.** Una o dos líneas con el porqué.

Condiciones o bloqueos, si los hay:

-

## Resultados por prueba

Marca cada fila con ✅ pasa, ⚠️ pasa con reservas o ❌ falla, y anota lo observado.

### A. Arranque y GPU

| # | Prueba | Resultado | Notas |
|---|---|---|---|
| 1 | GPU elegida por wgpu | | línea `Selected GPU` del log |
| 2 | Arranque con archivo chico | | primer frame: ms |
| 3 | Arranque con archivo de 50 MB | | carga: ms, primer frame: ms |

### B. Archivo grande (vista vim)

| # | Prueba | Resultado | Notas |
|---|---|---|---|
| 4 | Scroll manual con rueda o touchpad | | fps del overlay, tirones |
| 5 | Saltos `G` y `gg` | | CPU del frame del salto |
| 6 | Benchmark de scroll | | pegar reporte abajo |
| 7 | Repetición de tecla (`j` sostenida) | | |

Reporte de `./run.sh vim grande --bench`:

```
(pegar aquí el contenido de resultados/…-vim-grande-bench.md)
```

### C. Teclado con nota-vim

| # | Prueba | Resultado | Notas |
|---|---|---|---|
| 8 | Normal e Insert básicos | | |
| 9 | Teclas muertas en Insert (`´`+`a`, `~`+`n`) | | qué muestra el overlay |
| 10 | Tecla muerta en Normal (`"` en us-intl) | | |
| 11 | AltGr (`@`, `{`, `[`…) | | |
| 12 | `ctrl-r`, `ctrl-c`, `tab` llegan al motor | | |
| 13 | IME (Fcitx5) | | preedit, ventana de candidatos, commit |

Teclas que no se tradujeron bien (copiar las líneas del overlay o del `.log`):

```
```

### D. Portapapeles

| # | Prueba | Resultado | Notas |
|---|---|---|---|
| 14 | Copiar de nota a otra app (`ctrl-shift-c`) | | |
| 15 | Pegar de otra app en nota (`ctrl-shift-v`) | | en Normal y en Insert |
| 16 | Primaria: de nota a otra app (clic medio afuera) | | |
| 17 | Primaria: de otra app a nota (clic medio en nota) | | |

### E. Editor de gpui-component (comparación)

| # | Prueba | Resultado | Notas |
|---|---|---|---|
| 18 | `kit chico`: scroll, teclas muertas, copiar y pegar | | |
| 19 | `kit grande`: abre y hace scroll | | primer frame: ms |
| 20 | Benchmark `kit grande --bench` | | pegar reporte abajo |

```
(pegar aquí el reporte de kit)
```

### F. X11 / XWayland (opcional)

| # | Prueba | Resultado | Notas |
|---|---|---|---|
| 21 | Repetir 4, 9 y 16 con `WAYLAND_DISPLAY=` | | |

## Comparación de números

| Métrica | vim (propia) | kit (gpui-component) |
|---|---|---|
| Carga de 50 MB | | |
| Arranque → primer frame | | |
| Intervalo p50 / p99 en benchmark | | |
| fps sostenidos | | |
| CPU por frame p50 / p99 | | |

## Riesgos que ya se vieron al integrar (antes de correrlo)

Detectados al escribir el spike en sage, leyendo el código de `gpui-pre 0.3.9` y `gpui-component 0.7.3`:

1. **El `Root` de gpui-kit se queda con `tab`, `shift-tab` y `ctrl-c`.** Los liga en su contexto y GPUI despacha los bindings antes que los listeners de tecla, así que un editor modal dentro de `Root` nunca los ve. El spike abre la vista vim sin `Root`. Para nota: o la raíz es propia, o hay que sobreescribir esos bindings en el contexto del editor.
2. **El editor de gpui-component no está pensado para 50 MB.** Su propia documentación de `EditorState` dice "handles large text up to about 50K lines"; el archivo de prueba tiene 512K. En el smoke test de sage, con un archivo de solo 2 MB (21K líneas), tardó 35 s en mostrar el primer frame, contra 0.7 s de la vista propia con 50 MB. Es render por CPU en un A8, pero la diferencia de dos órdenes de magnitud apunta a su modelo de layout (display map y wrap de todo el documento), no a GPUI. Las pruebas 18 a 20 dicen cuánto pesa en hardware real.
3. **gpui-component no implementa la selección primaria.** Ni `gpui-base` ni `gpui-component` llaman a `write_to_primary`/`read_from_primary`; la primaria solo funciona en la vista propia. GPUI sí la soporta en Wayland (`zwp_primary_selection_v1`) y X11.
4. **Teclas muertas e IME llegan por dos caminos.** En Wayland, xkb compone `´`+`a` y GPUI lo entrega como `KeyDown` con `key_char = "á"`; el preedit de la tecla muerta llega por el input handler. Las commits de IME de un byte llegan como `KeyDown`, las multibyte (p. ej. `ñ` desde Fcitx5) directo al input handler. El spike unifica ambos: en Insert el texto siempre entra por el input handler; en Normal la tecla va directa al motor.
5. **GPUI no implementa `delete_surrounding_text` de `text-input-v3`.** Solo maneja commit, preedit y done. Un IME que reemplace texto alrededor del cursor no va a funcionar; el spike lo registra en el overlay si pasa.
6. **Scroll con `f32`.** Con un millón de líneas la altura total pasa de 2²⁴ px y un `f32` ya no distingue un píxel. La vista propia guarda el scroll en líneas (`f64`); cualquier lista de GPUI que use offsets absolutos en píxeles va a temblar al final de un archivo enorme.
7. **No hay un hook público "después del present".** El tiempo de CPU por frame que mide el spike va del `render` de la raíz al último `paint` y no incluye el envío a la GPU. Como contraste, GPUI imprime su propia duración de frame (draw + present) con `ZED_MEASUREMENTS=1`.
8. **GPUI baja a 30 fps las ventanas sin foco** (`WindowOptions::inactive_frame_interval`, 33 ms por defecto). Es buena política de energía, pero nota tiene que decidirla a propósito; el benchmark del spike la desactiva para medir el render.
9. **Ecosistema:** `gpui` en crates.io sigue en 0.2.2 (2025-10-22); el camino viable es `gpui-pre` vía gpui-kit, que publica cada semana. Fijar con `=` es obligatorio: gpui-kit 0.7.2 y 0.7.3 salieron el mismo día.

## Smoke test en sage (sin GPU)

Para comprobar que el spike arranca, pinta y sale con el reporte antes de llevarlo a pc-oma, se corrió en sage bajo `xvfb-run` con lavapipe (Vulkan por CPU, X11). Los tiempos de frame no significan nada: Xvfb reporta 0 Hz de refresco y GPUI cae a un intervalo fijo de ~80 ms. Lo útil son los tiempos de carga y de primer frame, y que no haya pánicos ni errores.

| Corrida | Carga | Primer frame | CPU por frame p50 / p99 |
|---|---|---|---|
| `vim chico --bench` (2 MB) | 20 ms | 1082 ms | 2.26 / 11.33 ms |
| `vim grande --bench` (50 MB) | 338 ms | 710 ms | 2.35 / 13.45 ms |
| `kit chico --bench` (2 MB) | 24 ms | 35 104 ms | 4.65 / 13.80 ms |

El primer frame de la corrida chica incluye inicializar lavapipe la primera vez. wgpu eligió `llvmpipe (Vulkan)` como único adaptador, que es lo esperado en sage; en pc-oma la prueba 1 tiene que mostrar la RTX 4070.

## Decisiones que salen del spike

Llenar al cerrar el spike, y actualizar `02-gui-stack.md` §3 y `03-core-architecture.md` si algo cambia.

-
