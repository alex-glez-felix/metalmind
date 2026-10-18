# Presupuesto de rendimiento

> Decisión del 2026-10-18. Enlazado desde [03 §8](research/03-core-architecture.md#8-hitos-del-mvp). El rendimiento es la prioridad número uno de nota: este documento dice cuánto puede tardar cada operación, cómo se mide y qué impide que empeore sin que nadie se entere.

## De dónde salen los números

- **Un frame**: 16.7 ms a 60 Hz y 4.17 ms a 240 Hz. El spike sostuvo 240 fps con 0.7 ms de CPU por frame ([04](research/04-spike-gpui.md)), así que el render no es lo que limita: lo que pase en el núcleo por cada tecla es lo que puede hacer perder frames.
- **Lo que corre en cada tecla** (escribir, borrar, mover, un comando de Normal) tiene **1 ms** de núcleo. Así queda casi todo el frame para layout y paint, también a 240 Hz y en una máquina más lenta que la de desarrollo.
- **Lo que va a correr mientras se escribe en un prompt** (el finder en `space f`, la búsqueda con `/` si se hace incremental como el `incsearch` de nvim) tiene **8 ms**: la mitad de un frame de 60 Hz, la otra mitad es de la UI. Si no cabe, pasa a un hilo de fondo, que es más complejo; el presupuesto dice cuándo hace falta.
- **Un comando que se lanza una vez** y toca todo el buffer (`:%s`, el undo de un `:%s`, una macro con count) tiene **16 ms** en una nota de 5 k líneas: termina antes del frame siguiente y no se ve ningún salto.
- **Una búsqueda en todo el vault** corre en hilos de fondo y entrega en streaming: **100 ms** con 5 k notas, que se siente inmediato, y **1 s** con 50 k. Lo que sí bloquea la UI es cancelar la anterior al escribir otra letra, y eso tiene los 8 ms de un prompt. Buscar en una nota abierta con cambios sin guardar es en memoria y en el mismo hilo: también 8 ms.
- **Lo que pasa en segundo plano** (indexar el vault) no bloquea la UI, pero el finder y el árbol lo esperan: abrir 5 k notas tiene que estar antes de que la ventana pinte su primer frame (~300 ms).
- **Tamaños**: 5 k líneas es una nota larga; 50 k, el peor caso realista (un log, una nota exportada). 5 k notas es un vault grande de Obsidian; 50 k, uno enorme.

Los objetivos valen para la **máquina de pruebas** de los hitos (la de 04, con GPU). Los números de hoy salen de una máquina de escritorio x86_64 más rápida, así que lo que quede a menos del doble de su objetivo se marca ⚠️: en la máquina de pruebas puede no cumplir.

## Presupuesto y números de hoy

Medido el 2026-10-18 con `divan` en release, mediana de cada benchmark, en una máquina de escritorio x86_64 con Linux. Las notas salen de `nota_perf::note` y los vaults de `nota_perf::vault_paths` (las rutas) y `nota_perf::write_vault` (en disco, notas de ~1.5 KB): texto sintético y determinista, nada de datos reales. La búsqueda de texto se mide con la caché de disco caliente; en frío y el umbral de `tantivy`, en [03 §4](research/03-core-architecture.md).

| Operación | Objetivo | Hoy | | Bench |
|---|---|---|---|---|
| Arranque al primer frame, archivo chico | ≤ 300 ms | 244–297 ms (spike, máquina de pruebas) | ⏳ | sin automatizar ([#39](https://github.com/alex-glez-felix/nota/issues/39)) |
| Tecla en Insert, de la tecla al frame | < 16 ms | — | ⏳ | sin benchmark de la UI ([#39](https://github.com/alex-glez-felix/nota/issues/39)) |
| Render: CPU por frame | ≤ 4 ms (240 Hz) | p99 1.05 ms (spike) | ⏳ | sin benchmark en la app ([#39](https://github.com/alex-glez-felix/nota/issues/39)) |
| Tecla en Insert, núcleo, 5 k / 50 k líneas | ≤ 1 ms | 1.7 / 3.8 µs | ✅ | `nota-vim` `tecla_en_insert` |
| Backspace en Insert, 5 k / 50 k | ≤ 1 ms | 1.1 / 2.6 µs | ✅ | `nota-vim` `backspace_en_insert` |
| `dd` / `ciw` / `.` / `u` en 5 k | ≤ 1 ms | 3.1 / 12.7 / 6.2 / 2.9 µs | ✅ | `nota-vim` |
| Macro de 7 teclas, `100@a`, en 5 k | ≤ 16 ms | 1.8 ms | ✅ | `nota-vim` `macro_100_veces` |
| `:%s/nota/apunte/g` en 5 k (cada `nota` del texto) | ≤ 16 ms | 6.6 ms | ✅ | `nota-vim` `sustituir_todo` |
| `u` de ese `:%s` | ≤ 16 ms | 2.9 ms | ✅ | `nota-vim` `deshacer_sustituir_todo` |
| `:%s` en 50 k | ≤ 160 ms | 84 ms | ⚠️ | `nota-vim` `sustituir_todo` |
| `/` sin coincidencias en 50 k (recorre todo) | ≤ 8 ms | 0.34 ms | ✅ | `nota-vim` `buscar_sin_resultado` |
| `?`, `/\<…\>` y `:%s` sin coincidencias en 50 k | ≤ 8 ms | 0.35 / 0.37 / 0.35 ms | ✅ | `nota-vim` `…_sin_resultado` |
| `/` hasta la siguiente coincidencia en 50 k; `n` | ≤ 1 ms | 36 µs; 2 µs | ✅ | `nota-vim` |
| Scan de `nota-syntax`, nota de 5 k | ≤ 8 ms | 1.3 ms | ✅ | `nota-syntax` `nota` |
| Scan, nota de 50 k | ≤ 50 ms | 11.7 ms | ✅ | `nota-syntax` `nota` |
| Scan patológico: línea de 1 MB / 5 k rachas de `#` / 5 k `[[` sin cerrar / 5 k títulos `(` sin cerrar / rachas de 1 a 2 000 backticks | ≤ 16 ms y lineal | 3.2 / 0.5 / 0.2 / 0.3 / 1.4 ms | ✅ | `nota-syntax` |
| Abrir el vault, 5 k notas (walk, watcher, índices de links y de tags) | ≤ 200 ms | 38.7 ms | ✅ | `nota-vault` `abrir_5k_notas` |
| Abrir el vault, 50 k notas | ≤ 2 s | 479 ms | ✅ | `nota-vault` `abrir_50k_notas` |
| Índice de links de cero, 5 k / 50 k notas (30 k / 300 k links): nombres repetidos en las 100 carpetas; únicos | ≤ 200 ms / ≤ 2 s | 18 / 327 ms; 24 / 336 ms | ✅ | `nota-vault` `indexar_links`, `indexar_links_nombres_unicos` |
| Índice de links, un cambio del watcher: modificar una nota / crear una cuyo nombre usan 300 links | ≤ 16 ms | 23 µs / 0.35 ms | ✅ | `nota-vault` `modificar_una_nota`, `crear_una_nota_que_empata` |
| Publicar el índice de links en cada lote (clonarlo), 5 k / 50 k | ≤ 16 / 160 ms | 5.7 / 88 ms | ⚠️ | `nota-vault` `clonar_el_indice` |
| Un nombre en cada carpeta (`index.md` en 8 000 carpetas): armar el índice / crear otra | lineal | 60 / 6.8 ms | ✅ | test `un_nombre_en_cada_carpeta_es_lineal` |
| `gd` en una nota de 5 k líneas (escanea el buffer) | ≤ 16 ms | 0.93 ms | ✅ | `nota-vault` `seguir_un_link_en_5k_lineas` |
| Autocompletado de `[[`: el destino de una página de 50 notas | ≤ 8 ms | 0.10 ms | ✅ | `nota-vault` `nombrar_50_notas_para_completar` |
| Plan de renombre: una nota / mover una carpeta de 500 notas | ≤ 16 ms | 7.6 µs / 13.4 ms | ⚠️ | `nota-vault` `plan_de_renombrar_una_nota`, `plan_de_mover_una_carpeta` |
| Mover esa carpeta con cada nombre en las 100 carpetas (peor caso) | lineal | 37 ms | ✅ | test `planear_mover_una_carpeta_es_lineal` |
| Abrir con tags: 5 k notas y 200 páginas de tag / 50 k y 2 000 | ≤ 200 ms / ≤ 2 s | 40 / 465 ms | ✅ | `nota-vault` `abrir_con_tags` |
| Consulta de tags `#t1 -#t3`, un tag cuyos descendientes cubren el 99 % de las notas, 5 k / 50 k | ≤ 16 ms | 0.67 / 13 ms | ⚠️ | `nota-vault` `consulta_por_tag_con_descendientes` |
| Panel de tags entero (conteos y orden), 5 k / 50 k | ≤ 16 / 160 ms | 1.15 / 28 ms | ✅ | `nota-vault` `panel_de_tags` |
| Panel de tags en focus (`area-1/ #t1`), 5 k / 50 k | ≤ 16 / 160 ms | 1.35 / 30 ms | ✅ | `nota-vault` `panel_en_focus` |
| Candidatos del autocompletado de `#`, 5 k / 50 k | ≤ 16 / 160 ms | 0.90 / 25 ms | ✅ | `nota-vault` `candidatos_de_autocompletado` |
| Plan de renombrar un tag con página, hijos y alias, 5 k / 50 k | ≤ 16 / 160 ms | 0.05 / 1.0 ms | ✅ | `nota-vault` `plan_de_renombrar_un_tag_con_pagina` |
| Publicar los tags en cada lote (clonarlos), 5 k / 50 k | ≤ 16 / 160 ms | 1.7 / 33 ms | ✅ | `nota-vault` `clonar_los_tags` ([#60](https://github.com/alex-glez-felix/nota/issues/60)) |
| Finder, 5 k rutas, peor consulta (`tema nota`): ordenar todo y las posiciones de 50 | ≤ 8 ms | 0.67 ms | ✅ | `nota-search` |
| Finder, 50 k rutas, peor consulta | ≤ 8 ms | 6.8 ms (en el fondo: no bloquea la UI) | ⚠️ | `nota-search` |
| Finder, una tecla en el picker con 5 k rutas: filtrar todo y preparar las 12 filas visibles, peor consulta (`tema nota`) | ≤ 8 ms | 0.75 ms | ✅ | `nota-ui` `picker` |
| Texto completo en el vault, palabra en 1 de cada 1 000 notas (lee todo), 5 k / 50 k notas | ≤ 100 ms / ≤ 1 s | 6.0 / 33 ms | ✅ | `nota-search` `palabra_rara` |
| Texto completo, palabra en casi todas las notas, 5 k / 50 k | ≤ 100 ms / ≤ 1 s | 11 / 116 ms | ✅ | `nota-search` `palabra_comun` |
| Texto completo, regex sin literal que la acelere, 5 k / 50 k | ≤ 100 ms / ≤ 1 s | 12 / 49 ms | ✅ | `nota-search` `regex_de_fecha` |
| Empezar una búsqueda de texto, recibir la primera nota y cancelarla, 5 k / 50 k | ≤ 8 ms | 1.3 / 1.4 ms | ✅ | `nota-search` `empezar_y_cancelar` |
| Texto completo en una nota abierta de 5 k líneas: palabra frecuente / sin resultado | ≤ 8 ms | 0.95 ms / 18 µs | ✅ | `nota-search` `nota_en_memoria` |
| Buffer: insertar / borrar un carácter en 50 k | ≤ 0.1 ms | 5.2 / 2.8 µs | ✅ | `nota-buffer` |
| Buffer: undo + redo de una edición en 50 k | ≤ 0.1 ms | 2.0 µs | ✅ | `nota-buffer` |
| Buffer: una edición por línea en un lote, 5 k / 50 k | ≤ 16 / 160 ms | 4.4 / 60 ms | ✅ | `nota-buffer` `una_edicion_por_linea` |
| Buffer: undo de ese lote, 5 k / 50 k | ≤ 16 / 160 ms | 2.5 / 34 ms | ✅ | `nota-buffer` |
| Ordenar un lote del watcher con muchos renames | lineal | lineal: 1.4 / 7.8 / 35 ms con 1 k / 5 k / 20 k | ✅ | test `ordenar_muchos_renames_es_lineal` |

Los tags (05 §9), medidos el 2026-10-18 en la misma máquina: el vault de `nota_perf::tagged_vault`, con las rutas de `vault_paths`, una nota de cada diez sin tags y las demás con un tag en `tags:`, dos en el texto y un alias, y una página de tag cada 25 notas en un árbol de `parents` con varios padres. Abrir el vault también escanea los tags de cada nota: en la misma máquina, `main` con solo el índice de links abría 5 k y 50 k notas en 33.9 y 434 ms, y los tags suman 4.8 y 45 ms (un 14 % y un 10 %). El objetivo de la consulta es el de 05 §9 (filtrar `space f` por un tag con descendientes); con 50 k notas, 8 de sus 13 ms son juntar en un conjunto las rutas de 2 800 tags, lo que bajaría con ids de nota (el `NoteId` de M3). La UI no la repite mientras solo cambia la parte difusa de la consulta. El panel y los candidatos se arman una vez por `VaultUpdate`, no por tecla.

⏳ es pendiente: todavía no se puede medir sin abrir una ventana. El arranque se mide a mano con el `--bench` del spike; hacerlo en CI pide un Xvfb, y bajo lavapipe los tiempos de frame no significan nada (04, "smoke sin GPU").

## El finder y la paleta

El picker (`nota-ui/src/picker/`) no filtra en el hilo de la UI: cada tecla manda la búsqueda al executor de fondo de GPUI, de a una a la vez y con la consulta versionada. Lo que vuelve de una versión vieja se descarta, y si se teclea mientras corre una, al terminar se busca solo lo último. Así los 8 ms del prompt son un tope de latencia, no de bloqueo: con 50 k rutas la UI sigue respondiendo aunque la búsqueda tarde. Lo único que corre en el hilo de la UI es la búsqueda de un `enter` que llega antes que los resultados (la que se está esperando) y las posiciones de las letras que coinciden, solo para las filas visibles de la lista virtual (`FuzzyMatcher::positions`). Abrir el vault del finder y leer y guardar las notas recientes van en el fondo.

## Tres capas contra regresiones

### 1. Benchmarks de tiempo (`divan`, local)

`cargo bench -p <crate>` en `nota-buffer`, `nota-vim`, `nota-syntax`, `nota-search` y `nota-vault`. Son los números de la tabla. Se sigue con `divan` (elegido en M2, 03 §6): menos dependencias que `criterion` y sin harness que configurar. Lo que muta el estado (teclas de vim, ediciones) arma una nota nueva antes de cada iteración (`with_inputs`, fuera de la medición) y corre una iteración por muestra.

No sirven para CI: en los runners de GitHub el tiempo de pared varía más que las regresiones que se quieren atrapar.

### 2. Conteo de instrucciones en cada PR (`gungraun`, CI)

`crates/nota-perf/benches/instrucciones.rs` cuenta instrucciones con callgrind para un subconjunto de la tabla: una tecla en Insert, `dd`, `ciw`, `.`, `100@a`, `:%s`, `/` sin resultado, el scan (normal y patológico), el finder, la búsqueda de texto en una nota en memoria y editar y deshacer en el buffer. La búsqueda en el vault no entra: lee el disco con varios hilos y su conteo no es determinista. El conteo es determinista: la misma entrada da el mismo número en cada corrida, haya o no otros procesos.

`gungraun` 0.20 y no `iai-callgrind`: es el mismo proyecto renombrado; `iai-callgrind` se quedó en 0.16.1 (julio de 2025) y `gungraun` sigue publicando (0.20.0, septiembre de 2026).

El job `instrucciones` de `.github/workflows/perf.yml`:

- **La referencia de `main` no se guarda: se mide.** En una PR, `HEAD` es el merge con la base y `HEAD^1` es la base. El job corre los benchmarks en la base (`--save-baseline=base`) y luego en la PR (`--baseline=base`), en el mismo runner. Guardar el número de `main` en un caché o un artefacto no sirve: el conteo depende de las extensiones del CPU, porque `memchr` y `ropey` eligen su implementación SIMD al arrancar, y dos runners de GitHub no siempre tienen el mismo CPU. Medir los dos lados en la misma máquina cuesta una corrida más (~15 s de benchmarks y lo que se recompile) y quita esa fuente de error. La base se compila en un `CARGO_TARGET_DIR` aparte: con el mismo, cargo daba por fresco el binario de la base (mismas rutas relativas, fuentes de la PR más viejas) y comparaba la base consigo misma.
- **Falla si un benchmark sube más del 5 % en instrucciones** (`--callgrind-limits='ir=5%'`). Repetir la corrida en la misma máquina da casi el mismo conteo: la diferencia más grande entre dos corridas fue 0.024 % (la semilla de los `HashMap` cambia por proceso), así que el 5 % es todo margen para regresiones de verdad y deja pasar cambios chicos que mueven unos pocos cientos de instrucciones. Una regresión del 5 % es chica para un usuario, pero 4 PRs de 5 % son un 22 %: el límite es para que cada subida se vea y se discuta cuando aparece.
- **Una regresión justificada** (una función nueva que cuesta lo que cuesta) se acepta poniendo la etiqueta `rendimiento-aceptado` en la PR: el job vuelve a correr, reporta la diferencia y no falla. La justificación va en la descripción de la PR.
- **Solo corre si cambia el núcleo medido** (`nota-buffer`, `nota-vim`, `nota-syntax`, `nota-search`, `nota-perf`, `Cargo.toml`, `Cargo.lock`, el toolchain o el propio workflow), y en `main` con los mismos filtros para dejar el caché de compilación. El tiempo en CI se anota en la PR que lo agrega; si pasa de unos 5 minutos con caché, se recorta el subconjunto. Una PR que solo toca la UI o la documentación no lo paga.
- Si la PR cambia un benchmark, se compara el de la base con el de la PR, que ya no miden lo mismo: la diferencia de ese benchmark no significa nada y la etiqueta es la salida. Si la base no tiene benchmarks (la PR que los agrega), el job solo los corre.

Conteos de hoy (valgrind 3.25, x86_64), la referencia contra la que se comparará la próxima PR:

| Benchmark | Instrucciones |
|---|---|
| Insertar un carácter en el buffer, 5 k / 50 k | 18.8 k / 30.9 k |
| Undo + redo en 50 k | 25.2 k |
| Tecla en Insert, 5 k / 50 k | 21.0 k / 31.4 k |
| `dd` / `ciw` / `.` en 5 k | 41.5 k / 311 k / 132 k |
| `100@a` en 5 k | 32.9 M |
| `:%s` en 5 k | 107.9 M |
| `/` sin resultado en 50 k | 6.0 M |
| Scan 5 k / 50 k | 16.5 M / 165.0 M |
| Scan patológico: `#` / `[[` / títulos / backticks / 1 MB | 8.8 M / 4.2 M / 5.7 M / 29.1 M / 52.7 M |
| Finder 5 k / 50 k (ordenar todas las coincidencias, para la lista virtual) | 18.9 M / 187.8 M |
| Texto completo en una nota de 5 k: palabra frecuente / sin resultado | 12.4 M / 459 k |

Para correrlo en local hacen falta `valgrind` y `gungraun-runner` de la misma versión que `gungraun` (`cargo binstall gungraun-runner@0.20.0`, o el binario de su release):

```sh
cargo bench -p nota-perf --bench instrucciones -- --save-baseline=antes
# cambios…
cargo bench -p nota-perf --bench instrucciones -- --baseline=antes
```

`cargo bench --workspace` incluye este benchmark; sin valgrind, `--exclude nota-perf`.

### 3. Pruebas de complejidad (`cargo nextest`, en cada PR)

Lo que puede volverse cuadrático sin que ningún benchmark de tamaño fijo lo note (el scanner, la búsqueda, `:s`, el índice del vault, el de links y el de tags, los lotes de ediciones) tiene un test `complexity.rs` (o `…_es_lineal` en los tests unitarios de `nota-vault`) que corre la operación con tamaño N y **16N**:

- El paso es ×16 y no ×4 porque con ×4 un `N^1.5` da ×8, justo entre lineal (×4) y cuadrático (×16), y ningún umbral lo separa de lo lineal con margen. No es teórico: las rachas de backticks sin pareja de largos 1…k miden k² bytes y el código anterior al arreglo costaba k³, o sea `N^1.5` en bytes, y con ×4 pasaba.
- `nota_perf::assert_linear` falla si 16N tarda más de **×40** lo que N. Lineal da ×16; lo peor que se vio en verde, con la suite completa en paralelo, fue ×22 (ordenar el borrado de una carpeta grande del vault, un `N log N` con la caché en contra). `N^1.5` da ×64 y cuadrático ×256: ×40 deja casi el doble de margen hacia los dos lados.
- `nota_perf::assert_flat` es para lo que no debería depender del tamaño, como una tecla en Insert: falla con más de **×4** (constante ×1, lineal ×16). En instrucciones, que no ven la caché, una tecla en 50 k líneas cuesta ×1.5 lo que en 5 k.
- Mide tiempo, no instrucciones: así corre en `cargo nextest` sin valgrind. Para que el ruido no lo tumbe, alterna hasta 11 corridas de cada tamaño y se queda con la más rápida de cada uno (el ruido solo suma tiempo), y exige que la corrida con N dure al menos 1 ms. Si tras 3 rondas lo grande ya gastó 2 s, deja de medir; y nextest corta a los 60 s cualquier test (`.config/nextest.toml`), porque con el código roto hasta una sola corrida grande puede tardar minutos.

Comprobado revirtiendo los dos arreglos cuadráticos que encontró la revisión en `nota-syntax`: sin el de los títulos `(` sin cerrar, ese test da ×253; sin el de las rachas de backticks fallan tres (rachas ×60, línea de 1 MB ×106, títulos ×256). Un caso que hoy no cumple queda con `#[ignore = "…#issue"]` hasta que se arregle, y el `ignore` se quita en el PR del arreglo.

## Al cambiar algo

- Operación nueva en el núcleo que corra en cada tecla o sobre todo el buffer: su fila en la tabla, su benchmark `divan`, y si recorre algo que crece, su prueba de complejidad. Si está en el camino de cada tecla, también en `instrucciones.rs`.
- Al tocar un número de la tabla, se actualiza con la fecha y el tipo de máquina en términos genéricos.
