# Presupuesto de rendimiento

> Decisión del 2026-10-18. Enlazado desde [03 §8](research/03-core-architecture.md#8-hitos-del-mvp). El rendimiento es la prioridad número uno de Metalmind: este documento dice cuánto puede tardar cada operación, cómo se mide y qué impide que empeore sin que nadie se entere.

## De dónde salen los números

- **Un frame**: 16.7 ms a 60 Hz y 4.17 ms a 240 Hz. El spike sostuvo 240 fps con 0.7 ms de CPU por frame ([04](research/04-spike-gpui.md)), así que el render no es lo que limita: lo que pase en el núcleo por cada tecla es lo que puede hacer perder frames.
- **Lo que corre en cada tecla** (escribir, borrar, mover, un comando de Normal) tiene **1 ms** de núcleo. Así queda casi todo el frame para layout y paint, también a 240 Hz y en una máquina más lenta que la de desarrollo.
- **Lo que va a correr mientras se escribe en un prompt** (el finder en `space f`, la búsqueda con `/` si se hace incremental como el `incsearch` de nvim) tiene **8 ms**: la mitad de un frame de 60 Hz, la otra mitad es de la UI. Si no cabe, pasa a un hilo de fondo, que es más complejo; el presupuesto dice cuándo hace falta.
- **Un comando que se lanza una vez** y toca todo el buffer (`:%s`, el undo de un `:%s`, una macro con count) tiene **16 ms** en una nota de 5 k líneas: termina antes del frame siguiente y no se ve ningún salto.
- **Una búsqueda en todo el vault** corre en hilos de fondo y entrega en streaming: **100 ms** con 5 k notas, que se siente inmediato, y **1 s** con 50 k. Lo que sí bloquea la UI es cancelar la anterior al escribir otra letra, y eso tiene los 8 ms de un prompt. Buscar en una nota abierta con cambios sin guardar es en memoria y en el mismo hilo: también 8 ms.
- **Lo que pasa en segundo plano** (indexar el vault) no bloquea la UI, pero el finder y el árbol lo esperan: abrir 5 k notas tiene que estar antes de que la ventana pinte su primer frame (~300 ms). Registrar la versión de una nota en el historial también va en segundo plano: el guardado no la espera, porque comprimir la primera versión de una nota de 50 k líneas pasa de 160 ms.
- **Tamaños**: 5 k líneas es una nota larga; 50 k, el peor caso realista (un log, una nota exportada). 5 k notas es un vault grande de Obsidian; 50 k, uno enorme.

Los objetivos valen para la **máquina de pruebas** de los hitos (la de 04, con GPU). Los números de hoy salen de una máquina de escritorio x86_64 más rápida, así que lo que quede a menos del doble de su objetivo se marca ⚠️: en la máquina de pruebas puede no cumplir.

## Presupuesto y números de hoy

Medido el 2026-10-18 con `divan` en release, mediana de cada benchmark, en una máquina de escritorio x86_64 con Linux. Las notas salen de `metalmind_perf::note` y los vaults de `metalmind_perf::vault_paths` (las rutas) y `metalmind_perf::write_vault` (en disco, notas de ~1.5 KB): texto sintético y determinista, nada de datos reales. La búsqueda de texto se mide con la caché de disco caliente; en frío y el umbral de `tantivy`, en [03 §4](research/03-core-architecture.md). Lo que escribe en disco (guardar, el historial) se midió en un sistema de archivos en disco y no en tmpfs, donde `fsync` no cuesta nada.

| Operación | Objetivo | Hoy | | Bench |
|---|---|---|---|---|
| Arranque al primer frame, archivo chico / 50 k líneas / en un vault de 5 k notas (abre el vault antes de la ventana) | ≤ 300 ms | p50 109–166 / 162–233 / 187–192 ms (lavapipe); 244–297 ms el spike en la máquina de pruebas | ⚠️ | `metalmind` `ui` ([abajo](#benchmark-de-la-ui)) |
| `metalmind add` / `metalmind search` de punta a punta (proceso entero, vault de 500 notas, build de debug) | ≤ 500 ms | 4 / 6 ms | ✅ | test `add_y_search_terminan_rapido` (`crates/metalmind/tests/commands.rs`) |
| Primer `space f` en un vault de 5 k notas, hasta abrir la nota elegida | ≤ 300 ms (el vault, antes del primer frame) | p50 31–41 ms, p95 ≤ 42 ms (lavapipe) | ✅ | `metalmind` `ui` |
| Tecla en Insert / Normal / `ctrl-d`, 50 k líneas, de la tecla al frame | < 16 ms | p50 0.95–1.02 / 0.86–0.89 / 1.49–1.57 ms; p95 ≤ 1.8 ms (lavapipe) | ✅ | `metalmind` `ui` |
| Render: CPU por frame (`render` de la raíz → último `paint`), mismas teclas y ráfaga de `ctrl-d` | ≤ 4 ms (240 Hz) | p50 0.72–1.42 ms, p95 ≤ 1.7 ms (lavapipe); p99 1.05 ms el spike | ✅ | `metalmind` `ui` |
| Ráfaga de 200 `ctrl-d` sin esperar cada frame, 50 k líneas | un frame por tecla | 200 frames, 1.15–1.19 ms por tecla (lavapipe) | ✅ | `metalmind` `ui` |
| Tecla en Insert, núcleo, 5 k / 50 k líneas | ≤ 1 ms | 1.7 / 3.8 µs | ✅ | `metalmind-vim` `tecla_en_insert` |
| Backspace en Insert, 5 k / 50 k | ≤ 1 ms | 1.1 / 2.6 µs | ✅ | `metalmind-vim` `backspace_en_insert` |
| `dd` / `ciw` / `.` / `u` en 5 k | ≤ 1 ms | 3.1 / 12.7 / 6.2 / 2.9 µs | ✅ | `metalmind-vim` |
| `]]` con el estilo ya calculado (en el editor), 5 k / 50 k | ≤ 1 ms | 6.6 / 98 µs | ✅ | `metalmind-vim` `siguiente_heading` |
| `Enter` al final de un ítem de lista (lo continúa), 5 k / 50 k | ≤ 1 ms | 33 / 184 µs | ✅ | `metalmind-vim` `enter_en_un_item` |
| `]]` en un motor sin editor (calcula el estilo de cero), 5 k / 50 k | ≤ 16 / 160 ms | 5.4 / 58 ms | ✅ | `metalmind-vim` `siguiente_heading_sin_estilo` |
| Macro de 7 teclas, `100@a`, en 5 k | ≤ 16 ms | 1.8 ms | ✅ | `metalmind-vim` `macro_100_veces` |
| `:%s/nota/apunte/g` en 5 k (cada `nota` del texto) | ≤ 16 ms | 6.6 ms | ✅ | `metalmind-vim` `sustituir_todo` |
| `u` de ese `:%s` | ≤ 16 ms | 2.9 ms | ✅ | `metalmind-vim` `deshacer_sustituir_todo` |
| `:%s` en 50 k | ≤ 160 ms | 84 ms | ⚠️ | `metalmind-vim` `sustituir_todo` |
| `:sort` en 5 k / 50 k | ≤ 16 / 160 ms | 0.71 / 11 ms | ✅ | `metalmind-vim` `ordenar` |
| `:g/nota/d` en 5 k / 50 k; su `u` en 5 k | ≤ 16 / 160 ms; ≤ 16 ms | 11.1 / 126 ms; 1.5 ms | ⚠️ | `metalmind-vim` `global_borrar`, `deshacer_global_borrar` |
| `:g/nota/s/nota/apunte/` en 5 k / 50 k | ≤ 16 / 160 ms | 11.3 / 144 ms | ⚠️ | `metalmind-vim` `global_sustituir` |
| `:g/^/m0` y `:%norm Ax` en 5 k | ≤ 16 ms | 14.3 / 13.4 ms | ⚠️ | `metalmind-vim` `global_mover_arriba`, `normal_en_cada_linea` |
| `:g/^/m0` y `:%norm Ax` en 50 k | ≤ 160 ms | 167 / 165 ms | ❌ | ídem ([#66](https://github.com/alex-glez-felix/metalmind/issues/66)) |
| `/` sin coincidencias en 50 k (recorre todo) | ≤ 8 ms | 0.34 ms | ✅ | `metalmind-vim` `buscar_sin_resultado` |
| `?`, `/\<…\>` y `:%s` sin coincidencias en 50 k | ≤ 8 ms | 0.35 / 0.37 / 0.35 ms | ✅ | `metalmind-vim` `…_sin_resultado` |
| `/` hasta la siguiente coincidencia en 50 k; `n` | ≤ 1 ms | 36 µs; 2 µs | ✅ | `metalmind-vim` |
| Scan de `metalmind-syntax`, nota de 5 k | ≤ 8 ms | 1.3 ms | ✅ | `metalmind-syntax` `nota` |
| Scan, nota de 50 k | ≤ 50 ms | 11.7 ms | ✅ | `metalmind-syntax` `nota` |
| Scan patológico: línea de 1 MB / 5 k rachas de `#` / 5 k `[[` sin cerrar / 5 k títulos `(` sin cerrar / rachas de 1 a 2 000 backticks | ≤ 16 ms y lineal | 3.2 / 0.5 / 0.2 / 0.3 / 1.4 ms | ✅ | `metalmind-syntax` |
| Scan de una nota con 200 fórmulas / una línea con 5 k `$` sin cerrar (precios) | ≤ 8 ms y lineal | 25 / 65 µs | ✅ | `metalmind-syntax` `nota_con_200_formulas`, `dolares_sin_cerrar` |
| Estilo de las líneas de una nota con 200 fórmulas, de cero | ≤ 16 ms | 0.17 ms | ✅ | `metalmind-syntax` `estilo_de_200_formulas` |
| Estilo de las líneas tras una tecla en un párrafo (repintar el tramo y la ventana), nota de 5 k / 50 k | ≤ 1 ms | 8.8 / 38 µs | ✅ | `metalmind-syntax` `tecla_en_un_parrafo` |
| Estilo tras abrir una cerca sin cierre arriba (todo lo de abajo pasa a ser código; se pinta la ventana), 5 k / 50 k | ≤ 1 ms | 22 / 375 µs | ✅ | `metalmind-syntax` `abrir_una_cerca_arriba` |
| Estilo de todas las líneas de cero (abrir e ir al final), 5 k / 50 k | ≤ 16 / 160 ms | 3.9 / 39 ms | ✅ | `metalmind-syntax` `estilo_de_cero` |
| Estilo tras una tecla en un párrafo de 5 k líneas sin ninguna en blanco (repinta el párrafo) | ≤ 16 ms y lineal | 5.9 ms | ✅ | `metalmind-syntax` `tecla_en_un_parrafo_de_5k` |
| Layout de las 200 fórmulas de esa nota en el hilo de la UI: caché fría (cada una nueva) / caliente | ≤ 16 ms / ≤ 1 ms | 6.6 ms / 6 µs | ✅ | `metalmind-ui` `medir_200_en_frio`, `medir_200_en_caliente` |
| Pintar las 22 fórmulas del spike, escala 1 / 2 (en el fondo) | segundo plano | 2.6 / 5.5 ms (0.12 / 0.25 ms cada una) | ✅ | `metalmind-ui` `pintar_las_22` |
| Nota con 200 fórmulas: primer frame / hasta pintar las visibles (cachés frías) | ≤ 300 ms | p50 111 / 134 ms (lavapipe) | ✅ | `metalmind` `ui` |
| Nota con 200 fórmulas: `ctrl-d` con caché fría / caliente, tecla en Insert junto a fórmulas / dentro de un bloque, de la tecla al frame | < 16 ms | p50 0.94 / 0.73 / 0.82 / 0.73 ms; p95 ≤ 1.6 ms (lavapipe) | ✅ | `metalmind` `ui` |
| Abrir el vault, 5 k notas (walk, watcher, índices de links y de tags) | ≤ 200 ms | 38.7 ms | ✅ | `metalmind-vault` `abrir_5k_notas` |
| Abrir el vault, 50 k notas | ≤ 2 s | 479 ms | ✅ | `metalmind-vault` `abrir_50k_notas` |
| Índice de links de cero, 5 k / 50 k notas (30 k / 300 k links): nombres repetidos en las 100 carpetas; únicos | ≤ 200 ms / ≤ 2 s | 18 / 327 ms; 24 / 336 ms | ✅ | `metalmind-vault` `indexar_links`, `indexar_links_nombres_unicos` |
| Índice de links, un cambio del watcher: modificar una nota / crear una cuyo nombre usan 300 links | ≤ 16 ms | 23 µs / 0.35 ms | ✅ | `metalmind-vault` `modificar_una_nota`, `crear_una_nota_que_empata` |
| Publicar el índice de links en cada lote (clonarlo), 5 k / 50 k | ≤ 16 / 160 ms | 5.7 / 88 ms | ⚠️ | `metalmind-vault` `clonar_el_indice` |
| Un nombre en cada carpeta (`index.md` en 8 000 carpetas): armar el índice / crear otra | lineal | 60 / 6.8 ms | ✅ | test `un_nombre_en_cada_carpeta_es_lineal` |
| `gd` en una nota de 5 k líneas (escanea el buffer) | ≤ 16 ms | 0.93 ms | ✅ | `metalmind-vault` `seguir_un_link_en_5k_lineas` |
| Autocompletado de `[[`: el destino de una página de 50 notas | ≤ 8 ms | 0.10 ms | ✅ | `metalmind-vault` `nombrar_50_notas_para_completar` |
| Pegar una imagen de 200 KB (escribirla, `fsync`, comprobar que el vault la ve y armar su embed), vault de 5 k / 50 k notas, en SSD | ≤ 16 ms | 2.8 / 3.1 ms (el `fsync` llega a ~100 ms: va en el fondo, como guardar) | ✅ | `metalmind-vault` `pegar_una_imagen` |
| Adjuntos huérfanos: 1 000 en la carpeta, 5 k / 50 k notas (del índice de links, sin leer notas) | ≤ 16 ms | 0.17 / 0.17 ms | ✅ | `metalmind-vault` `huerfanos` |
| Plan de renombre: una nota / mover una carpeta de 500 notas | ≤ 16 ms | 7.6 µs / 13.4 ms | ⚠️ | `metalmind-vault` `plan_de_renombrar_una_nota`, `plan_de_mover_una_carpeta` |
| Mover esa carpeta con cada nombre en las 100 carpetas (peor caso) | lineal | 37 ms | ✅ | test `planear_mover_una_carpeta_es_lineal` |
| Abrir con tags: 5 k notas y 200 páginas de tag / 50 k y 2 000 | ≤ 200 ms / ≤ 2 s | 40 / 465 ms | ✅ | `metalmind-vault` `abrir_con_tags` |
| Consulta de tags `#t1 -#t3`, un tag cuyos descendientes cubren el 99 % de las notas, 5 k / 50 k | ≤ 16 ms | 0.67 / 13 ms | ⚠️ | `metalmind-vault` `consulta_por_tag_con_descendientes` |
| Panel de tags entero (conteos y orden), 5 k / 50 k | ≤ 16 / 160 ms | 1.15 / 28 ms | ✅ | `metalmind-vault` `panel_de_tags` |
| Panel de tags en focus (`area-1/ #t1`), 5 k / 50 k | ≤ 16 / 160 ms | 1.35 / 30 ms | ✅ | `metalmind-vault` `panel_en_focus` |
| Candidatos del autocompletado de `#`, 5 k / 50 k | ≤ 16 / 160 ms | 0.90 / 25 ms | ✅ | `metalmind-vault` `candidatos_de_autocompletado` |
| Plan de renombrar un tag con página, hijos y alias, 5 k / 50 k | ≤ 16 / 160 ms | 0.05 / 1.0 ms | ✅ | `metalmind-vault` `plan_de_renombrar_un_tag_con_pagina` |
| Publicar los tags en cada lote (clonarlos), 5 k / 50 k | ≤ 16 / 160 ms | 1.7 / 33 ms | ✅ | `metalmind-vault` `clonar_los_tags` ([#60](https://github.com/alex-glez-felix/metalmind/issues/60)) |
| Árbol del panel, todo abierto: armarlo, 5 k / 50 k notas | ≤ 16 ms | 0.11 / 1.3 ms | ✅ | `metalmind-ui` `construir_todo_abierto` |
| Árbol: abrir o cerrar una carpeta, 5 k / 50 k | ≤ 1 ms | 0.13 / 1.6 ms | ❌ | `metalmind-ui` `expandir_y_colapsar` ([#59](https://github.com/alex-glez-felix/metalmind/issues/59)) |
| Árbol: bajar una fila | ≤ 1 ms | 20 ns | ✅ | `metalmind-ui` `bajar_una_fila` |
| Árbol: filtrar con `/`, peor consulta (`tema nota`), 5 k / 50 k | ≤ 8 ms | 2.6 / 35 ms | ❌ | `metalmind-ui` `filtrar` ([#59](https://github.com/alex-glez-felix/metalmind/issues/59)) |
| Árbol: aplicar el rename de las 10 carpetas de arriba que manda el watcher, 5 k / 50 k | ≤ 16 ms | 0.19 / 1.8 ms | ✅ | `metalmind-ui` `aplicar_un_rename` |
| Guardar una nota, 5 k / 50 k líneas (`Vault::save`: escritura atómica con `fsync`, lo que espera la UI) | ≤ 16 ms | 3.0 / 4.8 ms | ✅ | `metalmind-vault` `guardar_desde_el_vault` |
| Historial: versión con una línea cambiada, 5 k / 50 k (en el hilo del vault) | segundo plano y lineal | 1.5 / 5.1 ms | ✅ | `metalmind-vault` `version_con_una_linea_cambiada`, test `una_version_con_una_linea_cambiada_es_lineal` |
| Historial: primera versión / todas las líneas cambiadas, 5 k / 50 k (en el hilo del vault) | segundo plano | 18 / 171 ms; 33 / 342 ms | ✅ | `metalmind-vault` `primera_version`, `version_con_todo_cambiado` |
| Finder, 5 k rutas, peor consulta (`tema nota`): ordenar todo y las posiciones de 50 | ≤ 8 ms | 0.67 ms | ✅ | `metalmind-search` |
| Finder, 50 k rutas, peor consulta | ≤ 8 ms | 6.8 ms (en el fondo: no bloquea la UI) | ⚠️ | `metalmind-search` |
| Finder, una tecla en el picker con 5 k rutas: filtrar todo y preparar las 12 filas visibles, peor consulta (`tema nota`) | ≤ 8 ms | 0.75 ms | ✅ | `metalmind-ui` `picker` |
| Texto completo en el vault, palabra en 1 de cada 1 000 notas (lee todo), 5 k / 50 k notas | ≤ 100 ms / ≤ 1 s | 6.0 / 33 ms | ✅ | `metalmind-search` `palabra_rara` |
| Texto completo, palabra en casi todas las notas, 5 k / 50 k | ≤ 100 ms / ≤ 1 s | 11 / 116 ms | ✅ | `metalmind-search` `palabra_comun` |
| Texto completo, regex sin literal que la acelere, 5 k / 50 k | ≤ 100 ms / ≤ 1 s | 12 / 49 ms | ✅ | `metalmind-search` `regex_de_fecha` |
| Empezar una búsqueda de texto, recibir la primera nota y cancelarla, 5 k / 50 k | ≤ 8 ms | 1.3 / 1.4 ms | ✅ | `metalmind-search` `empezar_y_cancelar` |
| Texto completo en una nota abierta de 5 k líneas: palabra frecuente / sin resultado | ≤ 8 ms | 0.95 ms / 18 µs | ✅ | `metalmind-search` `nota_en_memoria` |
| `space /`, una tecla en el hilo de la UI con la búsqueda anterior corriendo (validar el patrón y mandarlo al hilo), 5 k / 50 k notas | ≤ 8 ms | 38 / 36 µs | ✅ | `metalmind-ui` `tecla_con_una_busqueda_corriendo` |
| `space /`, de la tecla al primer resultado (cancelar la anterior, buscar en una nota abierta de 5 k líneas y lanzar la nueva, en el hilo de la búsqueda), 5 k / 50 k | ≤ 100 ms | 2.5 / 2.4 ms | ✅ | `metalmind-ui` `de_la_tecla_al_primer_resultado` |
| `space /`, recibir un lote de 50 notas y preparar las 12 filas visibles | ≤ 8 ms | 31 µs | ✅ | `metalmind-ui` `recibir_un_lote_y_preparar_las_filas` |
| Buffer: insertar / borrar un carácter en 50 k | ≤ 0.1 ms | 5.2 / 2.8 µs | ✅ | `metalmind-buffer` |
| Buffer: undo + redo de una edición en 50 k | ≤ 0.1 ms | 2.0 µs | ✅ | `metalmind-buffer` |
| Buffer: una edición por línea en un lote, 5 k / 50 k | ≤ 16 / 160 ms | 4.4 / 60 ms | ✅ | `metalmind-buffer` `una_edicion_por_linea` |
| Buffer: undo de ese lote, 5 k / 50 k | ≤ 16 / 160 ms | 2.5 / 34 ms | ✅ | `metalmind-buffer` |
| Ordenar un lote del watcher con muchos renames | lineal | lineal: 1.4 / 7.8 / 35 ms con 1 k / 5 k / 20 k | ✅ | test `ordenar_muchos_renames_es_lineal` |

Los tags (05 §9), medidos el 2026-10-18 en la misma máquina: el vault de `metalmind_perf::tagged_vault`, con las rutas de `vault_paths`, una nota de cada diez sin tags y las demás con un tag en `tags:`, dos en el texto y un alias, y una página de tag cada 25 notas en un árbol de `parents` con varios padres. Abrir el vault también escanea los tags de cada nota: en la misma máquina, `main` con solo el índice de links abría 5 k y 50 k notas en 33.9 y 434 ms, y los tags suman 4.8 y 45 ms (un 14 % y un 10 %). El objetivo de la consulta es el de 05 §9 (filtrar `space f` por un tag con descendientes); con 50 k notas, 8 de sus 13 ms son juntar en un conjunto las rutas de 2 800 tags, lo que bajaría con ids de nota (el `NoteId` de M3). La UI no la repite mientras solo cambia la parte difusa de la consulta. El panel y los candidatos se arman una vez por `VaultUpdate`, no por tecla.

El estilo de las líneas del editor (03 §2, "Estilo en el editor"), medido el 2026-10-18 en la misma máquina: tras cada tecla se repinta el tramo editado (hasta la línea en blanco o la cerca que lo cierran) y lo que falta de la ventana, y lo de abajo se reusa en cuanto el estado coincide. Pintar de cero solo pasa la primera vez que se va al final de una nota larga: al abrirla se pinta la ventana. Abrir una cerca que no cierra cambia el estado de todo lo de abajo, pero se pinta solo la ventana; lo que cuesta con 50 k líneas es correr un lugar las 50 k entradas del estilo. La tecla del motor en Insert subió un 2 % en instrucciones (22.3 k a 22.8 k con 5 k líneas): mover la ventana ahora mide lo que mide cada línea.

Las filas de la UI se miden con lavapipe (abajo), que no es la máquina de pruebas: el arranque queda en ⚠️ hasta medir la app ahí. Bajo lavapipe entra en el objetivo, pero no se puede trasladar: con GPU, el spike midió 244–297 ms, más que lo que da lavapipe en una máquina de escritorio.

## Benchmark de la UI

`cargo bench -p metalmind --bench ui` (`crates/metalmind/benches/ui/`) mide la app real como la usa alguien: lanza el binario `metalmind` de release con el harness del smoke (`crates/metalmind/tests/support`, ver [testing](testing.md)), en un Xvfb propio y nunca en el display del entorno, le teclea con XTest e imprime una tabla en markdown. Necesita lo mismo que el smoke: Xvfb, un driver Vulkan (lavapipe sin GPU) y una fuente monoespaciada. Tarda unos 10 s más la compilación de release y no corre en CI: en la máquina de un runner con lavapipe sus números no se pueden comparar con nada, y compilar la app en release le cuesta más de 10 min sin caché.

**De dónde salen los tiempos.** `metalmind --frame-log ARCHIVO` (oculto en `--help`) escribe una línea por frame presentado (`FrameClock`, `metalmind-ui/src/frames.rs`): `t` desde el arranque del proceso, `cpu` del `render` de la raíz al último `paint` del editor, `present` hasta que vuelve `Window::present` de GPUI (el frame ya fue al renderer; cuándo llega a la pantalla no se ve desde la app), `input` desde la primera tecla que el frame muestra y `keys`, cuántas teclas muestra. El benchmark cuenta las teclas que manda y espera el frame que las muestra todas, así que cada muestra es una tecla y su frame, sin adivinar con pausas. El primer frame es la excepción: GPUI lo dibuja dentro de `open_window` y lo presenta en la siguiente petición de frame del sistema, sin volver a pintar, así que el arranque se mide hasta el primer frame **dibujado** (layout y paint de toda la ventana). Ese primer frame va siempre al log con `RUST_LOG=info` (`primer frame a … ms del arranque`); sin `--frame-log`, medir cuesta tres `Instant::now()` por frame.

**Qué mide.**

- Arranque al primer frame con una nota chica, una de 50 k líneas y una nota dentro de un vault de 5 k notas: 7 arranques por caso en el mismo Xvfb, tras uno que se descarta (el primero de cada Xvfb sale a veces varias veces más lento y a veces no). Antes de medir se hace `sync`, para que la escritura diferida de las notas generadas no caiga encima.
- El primer `space f` en ese vault: se teclea la ruta de una nota y `enter`, y se mide hasta que el título la muestra, desde el benchmark (sondeo de 10 ms).
- Sobre una nota con 200 fórmulas (abajo, "Fórmulas"): el arranque al primer frame y hasta que dejan de llegar frames (las imágenes visibles ya están), 60 `ctrl-d` con las cachés frías y otros 60 con todo en caché, y 60 teclas en Insert en una línea junto a fórmulas y dentro de un bloque de varias líneas.
- Sobre la nota de 50 k líneas, desde la mitad: 60 teclas en Insert, 60 motions en Normal (`j w b k e`) y 60 `ctrl-d`, de a una, cada una con su frame; y una ráfaga de 200 `ctrl-d` mandadas de golpe, como una tecla sostenida.

**Lavapipe no es una GPU.** Sin GPU, el Vulkan de mesa raspa en la CPU, en los mismos núcleos que la app: `present` y el arranque incluyen trabajo que una GPU hace aparte, y el tiempo de fondo (abrir el vault) compite con el render. Los números de esta sección sirven como **referencia relativa** para ver una regresión entre dos corridas en la misma máquina, no como el número contra el presupuesto, que sigue siendo el de la máquina de pruebas con GPU. `cpu` es lo que menos cambia con el driver: es trabajo de la app en el hilo de la UI.

Medido el 2026-10-19, con el markdown con estilo, tres corridas seguidas en una máquina de escritorio x86_64 con Linux, sin usar su GPU (lavapipe en un Xvfb). La máquina no estaba quieta (carga de 12 a 14 por otros procesos): la primera corrida sacó el arranque con archivo chico y con 50 k líneas 55–65 ms más arriba que las otras dos, y las teclas casi no se movieron (0.1 ms de p50). Rango de las tres:

| Medición | p50 | p95 |
|---|---|---|
| Arranque, archivo chico | 109–166 ms | 115–251 ms |
| Arranque, 50 k líneas | 162–233 ms | 168–251 ms |
| Arranque, en un vault de 5 k notas | 187–192 ms | 190–209 ms |
| Primer `space f`, ruta y `enter`, vault de 5 k notas | 31–41 ms | 42 ms |
| Tecla en Insert, de la tecla al frame / CPU | 0.95–1.02 / 0.75–0.79 ms | 1.10–1.24 / 0.86–0.97 ms |
| Tecla en Normal, de la tecla al frame / CPU | 0.86–0.89 / 0.72–0.74 ms | 0.97–0.99 / 0.80–0.83 ms |
| `ctrl-d`, de la tecla al frame / CPU | 1.49–1.57 / 1.34–1.42 ms | 1.69–1.78 / 1.54–1.64 ms |
| Ráfaga de 200 `ctrl-d`: CPU por frame | 1.05–1.07 ms | 1.15–1.22 ms |

Lo que encontró:

- El vault se abre antes de la ventana (`gui::prepare`), así que el arranque en un vault de 5 k notas paga `Vault::open`: ~80 ms más que con una nota sola (en las dos corridas sin el pico), y la fila "Abrir el vault" da 39 ms con `divan`. Lo demás de esa diferencia (el árbol del panel con 5 k notas en el primer frame, lavapipe en los mismos núcleos) no está separado. A cambio, el primer `space f` ya no espera nada: 31–41 ms con la ruta tecleada entera.
- Una nota de 50 k líneas suma ~55 ms al arranque (en las dos corridas sin el pico).
- El markdown con estilo (headings con su tamaño, código, links, tags) no movió el CPU por frame: la misma nota, que tiene de todo eso, daba 0.77 ms por tecla en Insert y 1.38 ms por `ctrl-d` justo antes, en la misma máquina, y ahora da 0.75–0.79 y 1.34–1.42 ms.
- Cada tecla se pinta en su propio frame también en ráfaga: 200 `ctrl-d` dan 200 frames, ~1.2 ms cada uno, sin acumular teclas pendientes.
- `ctrl-d` cuesta casi el doble que una tecla en Insert (~1.4 contra ~0.77 ms de CPU): entra media pantalla de líneas que GPUI no tenía en caché y les da forma, cuando una tecla en Insert solo cambia una. Sigue lejos de los 4 ms.
- Ninguna fila de la ventana necesita hoy una guarda en CI: la p50 de las teclas y del CPU por frame queda a menos de la mitad de su objetivo aun con el render en la CPU, y el arranque solo vale en la máquina de pruebas. Una guarda determinista del render, contando instrucciones de layout y paint sin ventana, queda en [#72](https://github.com/alex-glez-felix/metalmind/issues/72).

Para comparar un cambio en la UI, la misma máquina antes y después:

```sh
cargo bench -p metalmind --bench ui > antes.md
# cambios…
cargo bench -p metalmind --bench ui > despues.md
```

`cargo bench --workspace` lo incluye; sin Xvfb o sin driver, `--exclude metalmind`.

❌ no cumple y tiene su issue. `:g` y `:normal` corren su comando en cada línea como una edición aparte, igual que vim, y lo que cuesta es la transacción del buffer: unos 3 µs por línea.

## El finder y la paleta

El picker (`metalmind-ui/src/picker/`) no filtra en el hilo de la UI: cada tecla manda la búsqueda al executor de fondo de GPUI, de a una a la vez y con la consulta versionada. Lo que vuelve de una versión vieja se descarta, y si se teclea mientras corre una, al terminar se busca solo lo último. Así los 8 ms del prompt son un tope de latencia, no de bloqueo: con 50 k rutas la UI sigue respondiendo aunque la búsqueda tarde. Lo único que corre en el hilo de la UI es la búsqueda de un `enter` que llega antes que los resultados (la que se está esperando) y las posiciones de las letras que coinciden, solo para las filas visibles de la lista virtual (`FuzzyMatcher::positions`). Guardar las notas recientes va en un hilo propio; el vault lo abre el arranque, antes de la ventana (fila "Abrir el vault").

## La búsqueda de texto

`space /` lanza una búsqueda de `metalmind-search` en cada tecla, y empezar una cancela la anterior **esperando a sus hilos**. Hecho en el hilo de la UI no cabía: con la búsqueda de una palabra que está en casi todas las notas corriendo y una nota abierta de 5 k líneas con cambios, la tecla tardaba 5.6 / 7.5 ms de mediana con 5 k / 50 k notas, con picos de 30 ms (medido el 2026-10-18, la misma máquina de escritorio x86_64). Por eso la UI solo valida el patrón y lo manda a un hilo propio, que cancela, busca en las notas abiertas y lanza la nueva; si llegan varias teclas mientras cancela, salta a la última. Los resultados vuelven versionados y lo viejo se descarta, como en el picker difuso. Así, los 8 ms son un tope de latencia hasta que la búsqueda nueva arranca, no de bloqueo: la UI nunca espera a la búsqueda.

## Fórmulas

Las fórmulas (08 §10) se miden con `metalmind_perf::math_note(200)`: 200 fórmulas distintas, cada una con su número, dos de cada tres en línea y el resto en bloque, una de cada cuatro partida en varias líneas. Medido el 2026-10-19 en la misma máquina de escritorio x86_64; las filas de la ventana, con lavapipe en un Xvfb (carga de 4 a 6 por otros procesos) y en la misma corrida que las de la nota de 50 k líneas, que quedaron como antes de las fórmulas.

| Medición (200 fórmulas) | p50 | p95 |
|---|---|---|
| Arranque al primer frame | 111 ms | 112 ms |
| Arranque hasta pintar las fórmulas visibles | 134 ms | 156 ms |
| `ctrl-d`, caché fría: de la tecla al frame / CPU | 0.94 / 0.58 ms | 1.54 / 0.99 ms |
| `ctrl-d`, caché caliente: de la tecla al frame / CPU | 0.73 / 0.63 ms | 1.09 / 0.96 ms |
| Tecla en Insert en una línea junto a fórmulas: de la tecla al frame / CPU | 0.82 / 0.69 ms | 1.05 / 0.84 ms |
| Tecla en Insert dentro de un bloque de varias líneas: de la tecla al frame / CPU | 0.73 / 0.52 ms | 0.91 / 0.61 ms |

Lo que encontró:

- **El primer frame no espera ninguna imagen**: cuesta lo mismo que el de una nota chica (111 contra 107 ms). Mide las fórmulas que se ven (~33 µs cada una la primera vez: parsear, componer y validar con RaTeX) y reserva su lugar; las imágenes llegan del fondo en los frames que siguen, y a los 134 ms están todas las visibles.
- **El scroll no paga el pintado**: con la caché fría, cada `ctrl-d` mide las fórmulas que entran y manda a pintar sus imágenes al fondo; la tecla queda bajo 1 ms de p50, como con la caché caliente. La CPU por frame de la caché fría sale incluso menor: con la imagen todavía en camino, ese frame pinta un hueco.
- **Escribir dentro de un bloque** cambia su LaTeX en cada tecla: se mide de nuevo (µs) y se pinta otra vez en el fondo; la tecla no lo espera. Una tecla en una línea junto a fórmulas no toca ninguna.
- **Memoria**: el layout se guarda en em, sin color, en dos generaciones de 4 096 fórmulas; las imágenes, con tope de 32 MB (una fórmula ocupa ~29 KB a escala 1 y ~115 KB a escala 2, 08 §5), y lo que sale de la caché se suelta del atlas de GPUI.
- El rasterizador propio de la `DisplayList` que dejaba abierto 08 §7 no hizo falta: pintar va en el fondo y cuesta 0.12 ms por fórmula.

## Tres capas contra regresiones

### 1. Benchmarks de tiempo (`divan`, local)

`cargo bench -p <crate>` en `metalmind-buffer`, `metalmind-vim`, `metalmind-syntax`, `metalmind-search`, `metalmind-vault` y `metalmind-ui` (solo los modelos, sin ventana: `--bench file_tree`, `--bench picker`, `--bench text_search` y `--bench formulas`). Son los números de la tabla, salvo las filas de la ventana, que salen del [benchmark de la UI](#benchmark-de-la-ui). Se sigue con `divan` (elegido en M2, 03 §6): menos dependencias que `criterion` y sin harness que configurar. Lo que muta el estado (teclas de vim, ediciones) arma una nota nueva antes de cada iteración (`with_inputs`, fuera de la medición) y corre una iteración por muestra.

No sirven para CI: en los runners de GitHub el tiempo de pared varía más que las regresiones que se quieren atrapar.

### 2. Conteo de instrucciones en cada PR (`gungraun`, CI)

`crates/metalmind-perf/benches/instrucciones.rs` cuenta instrucciones con callgrind para un subconjunto de la tabla: una tecla en Insert, `dd`, `ciw`, `.`, `100@a`, `:%s`, `/` sin resultado, el scan (normal, patológico y con fórmulas), el estilo de las líneas tras una tecla, el finder, la búsqueda de texto en una nota en memoria y editar y deshacer en el buffer. La búsqueda en el vault no entra: lee el disco con varios hilos y su conteo no es determinista. El conteo es determinista: la misma entrada da el mismo número en cada corrida, haya o no otros procesos.

`gungraun` 0.20 y no `iai-callgrind`: es el mismo proyecto renombrado; `iai-callgrind` se quedó en 0.16.1 (julio de 2025) y `gungraun` sigue publicando (0.20.0, septiembre de 2026).

El job `instrucciones` de `.github/workflows/perf.yml`:

- **La referencia de `main` no se guarda: se mide.** En una PR, `HEAD` es el merge con la base y `HEAD^1` es la base. El job corre los benchmarks en la base (`--save-baseline=base`) y luego en la PR (`--baseline=base`), en el mismo runner. Guardar el número de `main` en un caché o un artefacto no sirve: el conteo depende de las extensiones del CPU, porque `memchr` y `ropey` eligen su implementación SIMD al arrancar, y dos runners de GitHub no siempre tienen el mismo CPU. Medir los dos lados en la misma máquina cuesta una corrida más (~15 s de benchmarks y lo que se recompile) y quita esa fuente de error. La base se compila en un `CARGO_TARGET_DIR` aparte: con el mismo, cargo daba por fresco el binario de la base (mismas rutas relativas, fuentes de la PR más viejas) y comparaba la base consigo misma.
- **Falla si un benchmark sube más del 5 % en instrucciones** (`--callgrind-limits='ir=5%'`). Repetir la corrida en la misma máquina da casi el mismo conteo: la diferencia más grande entre dos corridas fue 0.024 % (la semilla de los `HashMap` cambia por proceso), así que el 5 % es todo margen para regresiones de verdad y deja pasar cambios chicos que mueven unos pocos cientos de instrucciones. Una regresión del 5 % es chica para un usuario, pero 4 PRs de 5 % son un 22 %: el límite es para que cada subida se vea y se discuta cuando aparece.
- **Una regresión justificada** (una función nueva que cuesta lo que cuesta) se acepta poniendo la etiqueta `rendimiento-aceptado` en la PR: el job vuelve a correr, reporta la diferencia y no falla. La justificación va en la descripción de la PR.
- **Solo corre si cambia el núcleo medido** (`metalmind-buffer`, `metalmind-vim`, `metalmind-syntax`, `metalmind-search`, `metalmind-perf`, `Cargo.toml`, `Cargo.lock`, el toolchain o el propio workflow), y en `main` con los mismos filtros para dejar el caché de compilación. El tiempo en CI se anota en la PR que lo agrega; si pasa de unos 5 minutos con caché, se recorta el subconjunto. Una PR que solo toca la UI o la documentación no lo paga.
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
| Estilo tras una tecla en un párrafo (editar el buffer, seguir la edición y pintar la ventana), 5 k / 50 k | 177 k / 551 k |
| Finder 5 k / 50 k (ordenar todas las coincidencias, para la lista virtual) | 18.9 M / 187.8 M |
| Texto completo en una nota de 5 k: palabra frecuente / sin resultado | 12.4 M / 459 k |

Para correrlo en local hacen falta `valgrind` y `gungraun-runner` de la misma versión que `gungraun` (`cargo binstall gungraun-runner@0.20.0`, o el binario de su release):

```sh
cargo bench -p metalmind-perf --bench instrucciones -- --save-baseline=antes
# cambios…
cargo bench -p metalmind-perf --bench instrucciones -- --baseline=antes
```

`cargo bench --workspace` incluye este benchmark; sin valgrind, `--exclude metalmind-perf`.

### 3. Pruebas de complejidad (`cargo nextest`, en cada PR)

Lo que puede volverse cuadrático sin que ningún benchmark de tamaño fijo lo note (el scanner y el estilo de las líneas, la búsqueda, `:s`, el índice del vault, el de links y el de tags, los lotes de ediciones) tiene un test `complexity.rs` (o `…_es_lineal` en los tests unitarios de `metalmind-vault`) que corre la operación con tamaño N y **16N**:

- El paso es ×16 y no ×4 porque con ×4 un `N^1.5` da ×8, justo entre lineal (×4) y cuadrático (×16), y ningún umbral lo separa de lo lineal con margen. No es teórico: las rachas de backticks sin pareja de largos 1…k miden k² bytes y el código anterior al arreglo costaba k³, o sea `N^1.5` en bytes, y con ×4 pasaba.
- `metalmind_perf::assert_linear` falla si 16N tarda más de **×40** lo que N. Lineal da ×16; lo peor que se ve en verde son las de tags de `metalmind-vault`, entre ×23 y ×30 (`N log N` con la caché en contra: con 16N el costo por nota sube ×1.5). `N^1.5` da ×64 y cuadrático ×256: ×40 deja casi el doble de margen hacia los dos lados.
- `metalmind_perf::assert_flat` es para lo que no debería depender del tamaño, como una tecla en Insert: falla con más de **×4** (constante ×1, lineal ×16). En instrucciones, que no ven la caché, una tecla en 50 k líneas cuesta ×1.5 lo que en 5 k.
- Mide tiempo, no instrucciones: así corre en `cargo nextest` sin valgrind. Para que el ruido no lo tumbe, alterna hasta 11 corridas de cada tamaño y se queda con la más rápida de cada uno (el ruido solo suma tiempo), y exige que la corrida con N dure al menos 1 ms. Si tras 3 rondas lo grande ya gastó 2 s, deja de medir; y nextest corta a los 60 s cualquier test (`.config/nextest.toml`), porque con el código roto hasta una sola corrida grande puede tardar minutos. Corren de a una (el grupo `complexity` de `.config/nextest.toml`): el mínimo quita los picos, no la CPU repartida, y con más pruebas de tiempo que núcleos las de tags llegaron a ×41 en el CI y a ×66 en local con 2 CPU. Con el grupo, la suite tarda el doble en una máquina de 4 hilos (~80 → ~150 s), casi todo en estas pruebas.

Comprobado revirtiendo los dos arreglos cuadráticos que encontró la revisión en `metalmind-syntax`: sin el de los títulos `(` sin cerrar, ese test da ×253; sin el de las rachas de backticks fallan tres (rachas ×60, línea de 1 MB ×106, títulos ×256). Un caso que hoy no cumple queda con `#[ignore = "…#issue"]` hasta que se arregle, y el `ignore` se quita en el PR del arreglo.

## Al cambiar algo

- Operación nueva en el núcleo que corra en cada tecla o sobre todo el buffer: su fila en la tabla, su benchmark `divan`, y si recorre algo que crece, su prueba de complejidad. Si está en el camino de cada tecla, también en `instrucciones.rs`.
- Cambio en cómo se pinta el editor (`metalmind-ui/src/editor`) o en lo que corre al arrancar: el benchmark de la UI antes y después, en la misma máquina.
- Al tocar un número de la tabla, se actualiza con la fecha y el tipo de máquina en términos genéricos.
