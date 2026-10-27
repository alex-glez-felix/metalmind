#!/usr/bin/env bash
# Captura el visor en un Xvfb propio, sin GPU (lavapipe).
#   ./capturar.sh SALIDA.png ANCHOxALTO [argumentos del visor…]
# Necesita Xvfb, el ICD de lavapipe (VK_DRIVER_FILES) e `import` de ImageMagick.
set -euo pipefail
cd "$(dirname "$0")"
salida="$1"; tam="$2"; shift 2
pantalla=":$((90 + RANDOM % 9))"
Xvfb "$pantalla" -screen 0 "${tam}x24" -nolisten tcp &
xvfb=$!
trap 'kill "$visor" "$xvfb" 2>/dev/null || true' EXIT
sleep 1
cargo build --release --example pizarra -q
binario="${CARGO_TARGET_DIR:-target}/release/examples/pizarra"
env -u WAYLAND_DISPLAY DISPLAY="$pantalla" "$binario" --tam "$tam" "$@" &
visor=$!
sleep "${ESPERA:-4}"
env DISPLAY="$pantalla" import -window root "$salida"
echo "captura en $salida"
