# Spike de fórmulas

Proyecto aparte, fuera del workspace (como `spikes/gpui`): no toca el editor ni el binario. Compara tres caminos para pasar `$…$` y `$$…$$` a una imagen que GPUI pueda pintar. Los resultados y la recomendación están en [`docs/research/08-matematicas.md`](../../docs/research/08-matematicas.md).

| Motor (`medir <motor>`) | Camino |
|---|---|
| `typst` | `mitex` (LaTeX → math de Typst) + `typst` + `typst-render` |
| `ratex` | RaTeX: parser y layout de KaTeX en Rust + su rasterizador |
| `mathml` | `pulldown-latex` (LaTeX → MathML) + `formulary` (layout de MathML Core) + `resvg` |
| `mathcore` | lo mismo con `math-core` en lugar de `pulldown-latex` |

Cada motor es una feature (`typst`, `ratex`, `mathml`; todas por defecto), para enlazar uno solo en otro binario: así se midió cuánto crece `metalmind` con cada uno (08 §5). `medir`, `galeria` y los tests comparan los motores entre sí y piden las tres.

## Antes de compilar

mitex 0.2.4, el último en crates.io, convierte con la spec compilada dentro del crate aunque se le pase otra, y esa spec usa nombres que Typst 0.14 y 0.15 quitaron (`diff`, `angle.l`). `preparar.sh` baja mitex 0.2.4 de crates.io, comprueba su checksum y le aplica `assets/mitex-0.2.4-spec.patch` (tres líneas) en `vendor/mitex`, que `Cargo.toml` usa con `[patch.crates-io]`:

```sh
./preparar.sh
```

## Correr

```sh
export CARGO_BUILD_JOBS=3
./medir.sh 7 1                                   # 7 procesos por motor, escala 1; mediana de cada campo
./medir.sh 7 2                                   # escala 2 (HiDPI)
cargo run --release --bin galeria -- galeria.png 1.5
cargo run --release --example tres_formulas      # ventana GPUI con RaTeX
```

`medir` mide en un proceso nuevo: el arranque del motor, la primera fórmula, cada una de las demás (nueva para el motor), las mismas otra vez, 200 fórmulas distintas (para ver crecer la caché de Typst) y la memoria (RSS de `/proc` y heap vivo con `peak_alloc`).

En Xvfb, sin GPU: lavapipe (`VK_ICD_FILENAMES` apuntando a `lvp_icd.json`) y `env -u WAYLAND_DISPLAY DISPLAY=:9N`.

## Lo embebido

- `assets/mitex/`: el paquete de Typst de mitex (`packages/mitex/specs`) de upstream, commit `23ab8df` (2026-10-11), Apache-2.0. El de 0.2.4 importa `@preview/xarrow` (GPL-3.0-only) y usa símbolos que Typst 0.15 ya no tiene.
- Las fuentes salen de los crates: `typst-assets` (New Computer Modern, OFL) y `ratex-katex-fonts` (KaTeX, OFL).

## Tests

`cargo nextest run --release` comprueba lo que afirma el reporte: qué fórmulas compone cada motor, que mitex convierte con la spec que recibe (falla sin `preparar.sh`), la línea base, la escala y el paso a BGRA.
