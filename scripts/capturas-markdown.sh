#!/usr/bin/env bash
# Capturas del markdown con estilo, en los dos temas de fábrica, sobre la nota
# de prueba de `metalmind-syntax` (headings, negrita, links resueltos y no,
# tags, embeds, código, listas, tareas, citas y una regla).
#   scripts/capturas-markdown.sh [ruta/al/binario]
# Corre en un Xvfb propio, nunca en el display del entorno, con la config, el
# estado y los datos en una carpeta temporal. Necesita Xvfb, xdotool,
# ImageMagick (`import`) y el driver Vulkan lavapipe.
set -euo pipefail

root="$(cd "$(dirname "$0")/.." && pwd)"
bin="$(realpath "${1:-$root/target/debug/metalmind}")"
out="$root/docs/capturas"
lavapipe="${VK_ICD_FILENAMES:-/usr/share/vulkan/icd.d/lvp_icd.json}"
[[ -f "$lavapipe" ]] && export VK_ICD_FILENAMES="$lavapipe"
pause="${METALMIND_CAPTURAS_PAUSE:-3}"

work="$(mktemp -d)"
xvfb=""
app=""
cleanup() {
  [[ -n "$app" ]] && kill "$app" 2>/dev/null || true
  [[ -n "$xvfb" ]] && kill "$xvfb" 2>/dev/null || true
  rm -rf "$work"
}
trap cleanup EXIT

# Un display libre del 90 al 99, con su propio Xvfb.
for n in $(seq 90 99); do
  [[ -e "/tmp/.X11-unix/X$n" || -e "/tmp/.X$n-lock" ]] && continue
  Xvfb ":$n" -screen 0 1280x900x24 -nolisten tcp >"$work/xvfb.log" 2>&1 &
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

vault="$work/vault"
mkdir -p "$vault" "$work/config/metalmind" "$work/state" "$work/data" "$work/cache" "$out"
cp "$root/crates/metalmind-syntax/tests/fixtures/estilos.md" "$vault/estilos.md"
echo "# Nota existente" >"$vault/Nota existente.md"
echo "Otra nota." >"$vault/otra.md"
printf '\x89PNG\r\n' >"$vault/diagrama.png"

for theme in oscuro claro; do
  printf 'theme: metalmind-%s\n' "$theme" >"$work/config/metalmind/config.yaml"
  (
    cd "$vault"
    exec env -u WAYLAND_DISPLAY DISPLAY="$display" HOME="$work" \
      XDG_CONFIG_HOME="$work/config" XDG_STATE_HOME="$work/state" \
      XDG_DATA_HOME="$work/data" XDG_CACHE_HOME="$work/cache" \
      "$bin" estilos.md >>"$work/metalmind.log" 2>&1
  ) &
  app=$!
  window="$(DISPLAY="$display" xdotool search --sync --onlyvisible --name 'Metalmind$' | head -1)"
  sleep "$pause"
  DISPLAY="$display" import -window "$window" "$out/markdown-$theme.png"
  kill "$app"
  wait "$app" 2>/dev/null || true
  app=""
done
echo "capturas en $out"
