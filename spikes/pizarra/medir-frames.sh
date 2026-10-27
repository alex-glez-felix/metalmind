#!/usr/bin/env bash
# Corre el benchmark del visor con 500 y 2 000 elementos, en estilo limpio y
# a mano, y junta los reportes en markdown.
#   ./medir-frames.sh gamescope [1920x1080]   # GPU real, sin pantalla (gamescope --backend headless)
#   ./medir-frames.sh xvfb [1920x1080]        # sin GPU: Xvfb + lavapipe (VK_DRIVER_FILES)
# Con DETALLE=no, sin nivel de detalle: cada nodo con su malla a cualquier zoom.
set -euo pipefail
cd "$(dirname "$0")"
entorno="${1:-gamescope}"
tam="${2:-1920x1080}"
ancho="${tam%x*}"; alto="${tam#*x}"
cargo build --release --example pizarra -q
binario="${CARGO_TARGET_DIR:-target}/release/examples/pizarra"

case "$entorno" in
  gamescope)
    correr() { gamescope --backend headless -W "$ancho" -H "$alto" -w "$ancho" -h "$alto" -- \
      env -u WAYLAND_DISPLAY "$binario" "$@" 2>/dev/null || true; } ;;
  xvfb)
    pantalla=":$((90 + RANDOM % 9))"
    Xvfb "$pantalla" -screen 0 "${tam}x24" -nolisten tcp 2>/dev/null &
    xvfb=$!
    trap 'kill "$xvfb" 2>/dev/null || true' EXIT
    sleep 1
    correr() { env -u WAYLAND_DISPLAY DISPLAY="$pantalla" "$binario" "$@" 2>/dev/null; } ;;
  *) echo "entorno desconocido: $entorno (gamescope o xvfb)" >&2; exit 2 ;;
esac

for n in ${TABLEROS:-500 2000}; do
  for estilo in limpio mano; do
    bandera=(); [[ "$estilo" == mano ]] && bandera=(--mano)
    nombre="limpio"; [[ "$estilo" == mano ]] && nombre="a mano"
    correr --tam "$tam" --tablero "$n" "${bandera[@]}" ${DETALLE:+--sin-detalle} --banco \
      | sed "s/^### tablero de $n elementos/### $n elementos, $nombre, $tam, $entorno${DETALLE:+, sin nivel de detalle}/"
  done
done
