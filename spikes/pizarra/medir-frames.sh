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
# Un frame que GPUI rechaza no avanza el guion: sin tope, la corrida no acaba.
tope="${TOPE:-300}"
cargo build --release --example pizarra -q
binario="${CARGO_TARGET_DIR:-target}/release/examples/pizarra"

case "$entorno" in
  gamescope)
    # gamescope se cae al cerrar, después de que el visor terminó: su código
    # de salida no dice nada. Lo que vale es que haya reporte (abajo).
    correr() { timeout "$tope" gamescope --backend headless -W "$ancho" -H "$alto" -w "$ancho" -h "$alto" -- \
      env -u WAYLAND_DISPLAY "$binario" "$@" 2>>salida/medir-frames.log || true; } ;;
  xvfb)
    pantalla=":$((90 + RANDOM % 9))"
    Xvfb "$pantalla" -screen 0 "${tam}x24" -nolisten tcp 2>/dev/null &
    xvfb=$!
    trap 'kill "$xvfb" 2>/dev/null || true' EXIT
    sleep 1
    correr() { timeout "$tope" env -u WAYLAND_DISPLAY DISPLAY="$pantalla" "$binario" "$@" 2>>salida/medir-frames.log; } ;;
  *) echo "entorno desconocido: $entorno (gamescope o xvfb)" >&2; exit 2 ;;
esac

mkdir -p salida
fallos=0
for n in ${TABLEROS:-500 2000}; do
  for estilo in limpio mano; do
    bandera=(); [[ "$estilo" == mano ]] && bandera=(--mano)
    nombre="limpio"; [[ "$estilo" == mano ]] && nombre="a mano"
    # Una corrida que falla no corta las demás: se nota por la falta de reporte.
    reporte=$(correr --tam "$tam" --tablero "$n" "${bandera[@]}" ${DETALLE:+--sin-detalle} --banco) || true
    if [[ "$reporte" != *"| Fase |"* ]]; then
      echo "$n elementos, $nombre: el visor no dio reporte (ver salida/medir-frames.log)" >&2
      fallos=$((fallos + 1))
      continue
    fi
    sed "s/^### tablero de $n elementos/### $n elementos, $nombre, $tam, $entorno${DETALLE:+, sin nivel de detalle}/" <<<"$reporte"
    echo
  done
done
(( fallos == 0 ))
