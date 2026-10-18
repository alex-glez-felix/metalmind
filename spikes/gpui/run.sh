#!/usr/bin/env bash
# Compila y corre el spike. Ver README.md.
#   ./run.sh [vim|kit] [grande|chico] [--bench]
set -euo pipefail
cd "$(dirname "$0")"

view="${1:-vim}"
size="${2:-grande}"
bench="${3:-}"

case "$view" in vim | kit) ;; *) echo "vista desconocida: $view (vim o kit)" >&2; exit 2 ;; esac
case "$size" in
  grande) megabytes=50 ;;
  chico) megabytes=2 ;;
  *) echo "tamaño desconocido: $size (grande o chico)" >&2; exit 2 ;;
esac
case "$bench" in "" | --bench) ;; *) echo "opción desconocida: $bench (--bench)" >&2; exit 2 ;; esac

cargo build --release
bin=target/release/metalmind-spike-gpui
fixture="fixtures/$size.md"
[[ -f "$fixture" ]] || "$bin" gen --mb "$megabytes" --out "$fixture"

mkdir -p resultados
name="resultados/$(date +%Y%m%d-%H%M%S)-$view-$size${bench:+-bench}"
args=("$view" "$fixture")
[[ -z "$bench" ]] || args+=("$bench")
# El reporte sale por stdout y el log (GPU elegida, teclas) por stderr.
"$bin" "${args[@]}" > >(tee "$name.md") 2> >(tee "$name.log" >&2)
echo "reporte en $name.md y log en $name.log"
