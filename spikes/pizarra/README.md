# Spike de la pizarra

Proyecto aparte, fuera del workspace (como `spikes/gpui` y `spikes/math`): no toca el editor ni el binario. Prueba una pizarra para pensar (cajas, flechas, texto, tarjetas de notas y trazo libre) pintada con GPUI, sin webview ni JavaScript. Los resultados y la recomendación están en [`docs/research/09-pizarra.md`](../../docs/research/09-pizarra.md).

| Módulo | Qué hace |
|---|---|
| `src/formato.rs` | El formato en disco: JSON Canvas 1.0 con cuatro extensiones, un elemento por línea, claves en orden fijo y las claves desconocidas conservadas |
| `src/trazo.rs` | Los puntos del lápiz como texto compacto: deltas en décimas de píxel |
| `src/geometria.rs` | Por dónde pasa cada flecha, calculado de los nodos que une; acierto de clic |
| `src/escena.rs` | Lo que se pinta de cada elemento: mallas de lyon en coordenadas de la pizarra y textos; estilo limpio o a mano (roughr) |
| `src/excalidraw.rs` | Un `.excalidraw` en memoria, reescrito byte a byte como `JSON.stringify(datos, null, 2)` |
| `src/mapeo.rs` | Esbozo del mapeo `.excalidraw` ↔ pizarra |
| `src/generar.rs` | El ejemplo de `ejemplos/` y tableros deterministas de N elementos |
| `src/bin/medir.rs` | Bytes, líneas y diffs de los dos formatos; tiempo y memoria de la escena |
| `src/bin/convertir.rs` | El convertidor de línea de comandos, fuera de la app |
| `examples/pizarra/` | El visor GPUI: pan, zoom, arrastrar cajas, crear con `r`/`o`, nivel de detalle y benchmark |
| `examples/vacio.rs` | Una ventana GPUI vacía, la base para medir el binario |

La biblioteca no depende de GPUI (solo los ejemplos), como el núcleo de Metalmind. `lyon` va en la versión que ya trae GPUI.

## Correr

```sh
cargo run --release --example pizarra -- ejemplos/arquitectura.canvas
cargo run --release --example pizarra -- ejemplos/arquitectura.excalidraw   # pasa por el importador
cargo run --release --example pizarra -- --tablero 2000 --mano --claro
cargo run --release --bin medir                                            # 09 §2 y §3
cargo run --release --bin convertir -- ejemplos/arquitectura.canvas salida.excalidraw
```

En el visor: rueda para mover, `ctrl`+rueda para el zoom, arrastrar una caja la mueve (sus flechas se re-enrutan) y arrastrar el fondo mueve la pizarra; `h j k l`, `+`, `-` y `0` (ver todo); `r` u `o` y un clic crean una caja o una elipse.

## Medir

```sh
./medir-frames.sh gamescope 1920x1080   # GPU real sin pantalla: gamescope --backend headless
./medir-frames.sh xvfb 1920x1080        # sin GPU: Xvfb + lavapipe (VK_DRIVER_FILES)
DETALLE=no ./medir-frames.sh gamescope  # sin nivel de detalle
./medir-binario.sh                      # cuánto suma al binario, sin símbolos
./capturar.sh salida.png 1280x720 ejemplos/arquitectura.canvas --claro
```

`--banco` recorre un guion fijo (calentar, toda la pizarra a la vista, zoom continuo, pan al 100 % y arrastrar una caja), un paso por frame, y escribe el intervalo entre frames y el tiempo de CPU del `render` al último `paint`.

## Tests

`cargo test --release` (y `--examples` para el guion del benchmark). No abren ventana.
