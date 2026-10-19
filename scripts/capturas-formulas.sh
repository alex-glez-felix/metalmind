#!/usr/bin/env bash
# Capturas de las fórmulas en los dos temas de fábrica y en dos tamaños de
# ventana (1920x1080 y 1280x720), en tres escenas:
#   - inicio: la nota con las 22 fórmulas del spike (08 §2),
#     `crates/metalmind-perf/fixtures/22-formulas.md`, con el cursor en el
#     título;
#   - final: la misma nota con el cursor dentro del bloque de varias líneas
#     del final, que muestra su fuente;
#   - larga: una nota de 1 300 líneas, con números absolutos de 4 dígitos,
#     párrafos largos con fórmulas en línea, bloques de varias líneas, el
#     cursor dentro de una fórmula y el ratón sobre la última fórmula rota
#     que se ve, en la última línea: su mensaje cerca de los bordes derecho
#     e inferior.
#   scripts/capturas-formulas.sh [ruta/al/binario]
# Deja `docs/capturas/formulas-<escena>-<tema>-<ancho>x<alto>.png`.
# Corre en un Xvfb propio, nunca en el display del entorno, con la config,
# el estado y los datos en una carpeta temporal. Necesita Xvfb, xdotool,
# ImageMagick (`import`, `magick`) y el driver Vulkan lavapipe.
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
[[ "$display" != "${DISPLAY:-}" ]] || { echo "el Xvfb cayó en el DISPLAY del entorno" >&2; exit 1; }

vault="$work/vault"
mkdir -p "$vault" "$work/config/metalmind" "$work/state" "$work/data" "$work/cache" "$out"
cp "$root/crates/metalmind-perf/fixtures/22-formulas.md" "$vault/formulas.md"

# La nota larga: párrafos de más de 250 caracteres con fórmulas en línea,
# un bloque de varias líneas y uno solo en su línea cada tanto, y en la
# línea 1170 (o la primera libre después) una zona de prueba que termina en una línea de fórmulas rotas.
awk 'BEGIN {
  for (i = 1; i <= 1300; i++) {
    if (!zone && i >= 1170) {
      zone = 1
      print "### Zona de prueba"
      print "El cursor está dentro de $\\alpha_{aqui} + \\beta^2 = \\gamma$ y esa línea muestra la fuente; el párrafo sigue largo, con $\\sqrt{x_{1170}^2 + y^2}$ en medio y más texto que pasa del borde de la ventana."
      print "Un párrafo largo con $\\frac{\\partial f}{\\partial x}$, $\\sum_{k=1}^{n} k^2 = \\frac{n(n+1)(2n+1)}{6}$ y $e^{i\\theta}$, y texto que sigue y sigue hasta pasar del borde."
      print "$$"
      print "\\begin{pmatrix} a & b \\\\"
      print "c & d \\end{pmatrix} \\begin{pmatrix} x \\\\ y \\end{pmatrix} = \\begin{pmatrix} ax + by \\\\ cx + dy \\end{pmatrix}"
      print "$$"
      print "Otro párrafo con $\\int_0^1 x^2\\,dx = \\frac{1}{3}$ y $\\lim_{n \\to \\infty} (1 + 1/n)^n = e$ entre palabras que siguen hasta el borde."
      line = "Rotas:"
      for (k = 0; k < 24; k++) line = line " texto $\\frac{1}{$"
      print line
      i += 8
      continue
    }
    if (i % 50 == 0) { print "## Sección " i / 50; continue }
    if (i % 50 == 17) {
      print "$$"
      print "\\begin{aligned} f_{" i "}(x) &= (x+1)^2 \\\\"
      print "&= x^2 + 2x + 1 \\end{aligned}"
      print "$$"
      i += 3
      continue
    }
    if (i % 50 == 33) { print "$$\\int_0^{" i "} e^{-t^2}\\,dt$$"; continue }
    printf "Párrafo %d: la energía es $E_{%d} = mc^2$ y el texto sigue con palabras y más palabras, luego $\\frac{a_{%d}}{b_{%d}} + \\sqrt{%d}$ y continúa hasta pasar del borde de la ventana, con $\\sum_{k=1}^{%d} k$ al final.\n", i, i, i, i, i, i
  }
}' >"$vault/larga.md"

x() { DISPLAY="$display" xdotool "$@"; }

# Abre `note` con `theme` y `config`, deja la ventana de `size` y corre
# `act` antes de la captura.
shoot() {
  local scene="$1" theme="$2" size="$3" note="$4" config="$5" act="$6"
  printf 'theme: metalmind-%s\n%s' "$theme" "$config" >"$work/config/metalmind/config.yaml"
  (
    cd "$vault"
    exec env -u WAYLAND_DISPLAY DISPLAY="$display" HOME="$work" \
      XDG_CONFIG_HOME="$work/config" XDG_STATE_HOME="$work/state" \
      XDG_DATA_HOME="$work/data" XDG_CACHE_HOME="$work/cache" \
      "$bin" "$note" >>"$work/metalmind.log" 2>&1
  ) &
  app=$!
  window="$(x search --sync --onlyvisible --name 'Metalmind$' | head -1)"
  x windowmove "$window" 0 0
  x windowsize "$window" "${size%x*}" "${size#*x}"
  sleep "$pause"
  x windowfocus --sync "$window"
  "$act" "$theme"
  sleep "$pause"
  DISPLAY="$display" import -window "$window" "$out/formulas-$scene-$theme-$size.png"
  kill "$app"
  wait "$app" 2>/dev/null || true
  app=""
}

nothing() { :; }

into_block() {
  x key --window "$window" G k
}

# La línea de fórmulas rotas abajo de todo, el cursor dentro de una fórmula
# unas líneas arriba y el ratón sobre la última rota que se ve.
long_note() {
  local error
  error="$(awk '/^Rotas:/ { print NR; exit }' "$vault/larga.md")"
  x type --window "$window" ":$error"
  x key --window "$window" Return z b
  x type --window "$window" '?aqui'
  x key --window "$window" Return
  sleep "$pause"
  local shot="$work/hover.png" color
  DISPLAY="$display" import -window "$window" "$shot"
  if [[ "$1" == oscuro ]]; then color="#f7768e"; else color="#c4304f"; fi
  # El rectángulo de lo que tiene el color de error: las fórmulas rotas.
  local box w rest h offsets left top
  box="$(magick "$shot" -fuzz 2% -fill white -opaque "$color" -fill black +opaque white -format '%@' info:)"
  w="${box%%x*}"
  rest="${box#*x}"
  h="${rest%%+*}"
  offsets="${rest#*+}"
  left="${offsets%%+*}"
  top="${offsets#*+}"
  # Sin movimiento no hay evento: el puntero puede seguir ahí desde la
  # captura anterior.
  x mousemove --window "$window" 10 10
  sleep 0.5
  x mousemove --window "$window" "$((left + w - 20))" "$((top + h / 2))"
}

for size in 1920x1080 1280x720; do
  for theme in oscuro claro; do
    shoot inicio "$theme" "$size" formulas.md "" nothing
    shoot final "$theme" "$size" formulas.md "" into_block
    shoot larga "$theme" "$size" larga.md $'editor:\n  relativenumber: false\n' long_note
  done
done
echo "capturas en $out"
