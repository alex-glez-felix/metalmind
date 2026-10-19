#!/usr/bin/env bash
# Capturas de las fórmulas, en los dos temas de fábrica, sobre la nota con
# las 22 fórmulas del spike (08 §2), una que no compone y un bloque de
# varias líneas: `crates/metalmind-perf/fixtures/22-formulas.md`.
#   scripts/capturas-formulas.sh [ruta/al/binario]
# Deja dos por tema: la nota tal cual (el cursor en el título) y con el
# cursor dentro del bloque de varias líneas, que muestra su fuente.
# Corre en un Xvfb propio, nunca en el display del entorno, con la config,
# el estado y los datos en una carpeta temporal. Necesita Xvfb, xdotool,
# ImageMagick (`import`) y el driver Vulkan lavapipe.
set -euo pipefail

root="$(cd "$(dirname "$0")/.." && pwd)"
bin="$(realpath "${1:-$root/target/debug/metalmind}")"
out="$root/docs/capturas"
lavapipe="${VK_ICD_FILENAMES:-/usr/share/vulkan/icd.d/lvp_icd.json}"
[[ -f "$lavapipe" ]] && export VK_ICD_FILENAMES="$lavapipe"
pause="${METALMIND_CAPTURAS_PAUSE:-4}"

work="$(mktemp -d)"
xvfb=""
app=""
cleanup() {
  [[ -n "$app" ]] && kill "$app" 2>/dev/null || true
  [[ -n "$xvfb" ]] && kill "$xvfb" 2>/dev/null || true
  rm -rf "$work"
}
trap cleanup EXIT

# Un display libre del 90 al 99, con su propio Xvfb, alto para que entre
# la nota entera.
for n in $(seq 90 99); do
  [[ -e "/tmp/.X11-unix/X$n" || -e "/tmp/.X$n-lock" ]] && continue
  Xvfb ":$n" -screen 0 1400x2300x24 -nolisten tcp >"$work/xvfb.log" 2>&1 &
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
[[ "$display" != "${DISPLAY:-}" ]] || { echo "el Xvfb cayó en el DISPLAY del entorno" >&2; exit 1; }

vault="$work/vault"
mkdir -p "$vault" "$work/config/metalmind" "$work/state" "$work/data" "$work/cache" "$out"
cp "$root/crates/metalmind-perf/fixtures/22-formulas.md" "$vault/formulas.md"

x() { DISPLAY="$display" xdotool "$@"; }

for theme in oscuro claro; do
  printf 'theme: metalmind-%s\n' "$theme" >"$work/config/metalmind/config.yaml"
  (
    cd "$vault"
    exec env -u WAYLAND_DISPLAY DISPLAY="$display" HOME="$work" \
      XDG_CONFIG_HOME="$work/config" XDG_STATE_HOME="$work/state" \
      XDG_DATA_HOME="$work/data" XDG_CACHE_HOME="$work/cache" \
      "$bin" formulas.md >>"$work/metalmind.log" 2>&1
  ) &
  app=$!
  window="$(x search --sync --onlyvisible --name 'Metalmind$' | head -1)"
  x windowsize "$window" 1400 2300
  sleep "$pause"
  DISPLAY="$display" import -window "$window" "$out/formulas-$theme.png"
  # El cursor dentro del bloque de varias líneas: se ve su fuente.
  x windowfocus --sync "$window"
  x key --window "$window" G k
  sleep "$pause"
  DISPLAY="$display" import -window "$window" "$out/formulas-$theme-cursor.png"
  kill "$app"
  wait "$app" 2>/dev/null || true
  app=""
done
echo "capturas en $out"
