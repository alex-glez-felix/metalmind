#!/usr/bin/env bash
# Capturas del indicador del autoguardado en la barra de estado: "guardado",
# "sin guardar" (el autoguardado falla en una carpeta de solo lectura) y
# "cambió en disco" (el archivo cambia fuera con cambios sin guardar), con una
# nota de ruta larga, en los dos temas de fábrica y a 1920x1080 y 1280x720.
#   scripts/capturas-autoguardado.sh [ruta/al/binario]
# Corre en un Xvfb propio, nunca en el display del entorno, con la config, el
# estado y los datos en una carpeta temporal. Necesita Xvfb, xdotool, xprop,
# ImageMagick (`import`) y un driver Vulkan (lavapipe sin GPU).
set -euo pipefail

root="$(cd "$(dirname "$0")/.." && pwd)"
bin="$(realpath "${1:-$root/target/debug/metalmind}")"
out="$root/docs/capturas/autoguardado"
lavapipe="${VK_ICD_FILENAMES:-/usr/share/vulkan/icd.d/lvp_icd.json}"
[[ -f "$lavapipe" ]] && export VK_ICD_FILENAMES="$lavapipe"
pause="${METALMIND_CAPTURAS_PAUSE:-3}"

work="$(mktemp -d)"
xvfb=""
app=""
cleanup() {
  [[ -n "$app" ]] && kill "$app" 2>/dev/null || true
  [[ -n "$xvfb" ]] && kill "$xvfb" 2>/dev/null || true
  chmod -R u+w "$work" 2>/dev/null || true
  rm -rf "$work"
}
trap cleanup EXIT

# Un display libre del 90 al 99, con su propio Xvfb.
for n in $(seq 90 99); do
  [[ -e "/tmp/.X11-unix/X$n" || -e "/tmp/.X$n-lock" ]] && continue
  Xvfb ":$n" -screen 0 1920x1080x24 -nolisten tcp >"$work/xvfb.log" 2>&1 &
  xvfb=$!
  for _ in $(seq 50); do
    [[ -S "/tmp/.X11-unix/X$n" ]] && break
    sleep 0.1
  done
  if [[ -S "/tmp/.X11-unix/X$n" ]] && kill -0 "$xvfb" 2>/dev/null; then
    display=":$n"
    break
  fi
  xvfb=""
done
[[ -n "${display:-}" ]] || { echo "no arrancó ningún Xvfb" >&2; exit 1; }
x() { env -u WAYLAND_DISPLAY DISPLAY="$display" "$@"; }

folder="proyectos/2026/reuniones"
note="$folder/notas de la reunión trimestral con el equipo de diseño y producto.md"
mkdir -p "$out" "$work/config/metalmind"
: >"$out/titulos.txt"

# Abre la nota con `config` y deja la ventana de `size` en la esquina.
launch() {
  local config="$1" size="$2"
  rm -rf "$work/vault" "$work/state" "$work/data" "$work/cache"
  mkdir -p "$work/vault/$folder" "$work/state" "$work/data" "$work/cache"
  printf '# Reunión trimestral\n\nAcuerdos y pendientes de diseño y producto.\n' >"$work/vault/$note"
  printf '%s' "$config" >"$work/config/metalmind/config.yaml"
  (
    cd "$work/vault"
    exec env -u WAYLAND_DISPLAY DISPLAY="$display" HOME="$work" \
      XDG_CONFIG_HOME="$work/config" XDG_STATE_HOME="$work/state" \
      XDG_DATA_HOME="$work/data" XDG_CACHE_HOME="$work/cache" \
      "$bin" "$note" >>"$work/metalmind.log" 2>&1
  ) &
  app=$!
  window="$(x xdotool search --sync --onlyvisible --name 'Metalmind$' | head -1)"
  x xdotool windowmove "$window" 0 0
  x xdotool windowsize --sync "$window" "${size%x*}" "${size#*x}"
  x xdotool windowfocus --sync "$window"
  sleep "$pause"
}

keys() {
  x xdotool type --window "$window" --delay 80 "$1"
  x xdotool key --window "$window" Escape
}

capture() {
  local name="$1"
  sleep "$pause"
  x import -window "$window" "$out/$name.png"
  printf '%s: %s\n' "$name" "$(x xprop -id "$window" _NET_WM_NAME | sed 's/^[^"]*"//; s/"$//')" >>"$out/titulos.txt"
}

quit() {
  kill "$app"
  wait "$app" 2>/dev/null || true
  app=""
}

for theme in oscuro claro; do
  for size in 1920x1080 1280x720; do
    launch "theme: metalmind-$theme"$'\n' "$size"
    capture "guardado-$theme-$size"
    # Sin permiso de escribir en la carpeta, el autoguardado falla.
    chmod a-w "$work/vault/$folder"
    keys "iAgregar al acta: "
    capture "sin-guardar-$theme-$size"
    chmod u+w "$work/vault/$folder"
    quit

    # Con un delay largo el cambio sigue sin guardar cuando llega el de fuera.
    launch "theme: metalmind-$theme"$'\n'"editor:"$'\n'"  autosave_delay_ms: 60000"$'\n' "$size"
    keys "iAgregar al acta: "
    printf '# Reunión trimestral\n\nEditado desde otro equipo.\n' >"$work/vault/$note"
    capture "cambio-en-disco-$theme-$size"
    quit
  done
done
echo "capturas en $out"
