#!/usr/bin/env bash
# Cuánto suma la pizarra a un binario de GPUI, en release y sin símbolos.
set -euo pipefail
cd "$(dirname "$0")"
destino="${CARGO_TARGET_DIR:-target}/release/examples"
medir() { local copia="$destino/$1-sin-simbolos"; cp "$destino/$1" "$copia"; strip "$copia"; stat -c %s "$copia"; rm "$copia"; }
cargo build --release --example vacio -q
base=$(medir vacio vacio)
cargo build --release --example pizarra --no-default-features -q
limpio=$(medir pizarra limpio)
cargo build --release --example pizarra -q
mano=$(medir pizarra mano)
mb() { awk -v b="$1" 'BEGIN { printf "%.2f MB", b / 1048576 }'; }
echo "| Binario | Tamaño | Suma |"
echo "|---|---|---|"
echo "| ventana GPUI vacía | $(mb "$base") | — |"
echo "| visor, estilo limpio | $(mb "$limpio") | +$(mb $((limpio - base))) |"
echo "| visor, con roughr | $(mb "$mano") | +$(mb $((mano - base))) (roughr: +$(mb $((mano - limpio)))) |"
