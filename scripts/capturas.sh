#!/usr/bin/env bash
# Capturas de cada pantalla (editor con markdown, árbol, menú contextual,
# split, línea de comandos, finder, búsqueda de texto y paleta) en los dos
# temas de fábrica y en ventanas de 1920x1080 y 1280x720, sobre un vault que
# estresa el layout. Corre el test `capturas` de `metalmind` en un Xvfb
# propio, nunca en el display del entorno (ver docs/testing.md).
#
#   scripts/capturas.sh [salida] [--antes carpeta]
#
# `salida` es `target/capturas` si no se da. Con `--antes`, compara cada
# captura con la del mismo nombre en esa carpeta, deja `diff-*.png` con lo
# que cambió en rojo y sale con error si cambió algún píxel. El resumen va a
# `salida/resumen.md` y el título de la ventana en cada pantalla, a
# `salida/titulos.txt`. Usa lavapipe si está instalado, para que dos corridas
# en máquinas distintas dibujen igual.
set -euo pipefail

root="$(cd "$(dirname "$0")/.." && pwd)"
out="$root/target/capturas"
before=""
while (($#)); do
  case "$1" in
    --antes) before="$(realpath "$2")"; shift 2 ;;
    *) out="$1"; shift ;;
  esac
done
mkdir -p "$out"
out="$(realpath "$out")"
if [[ -n "$before" ]]; then
  [[ "$before" != "$out" ]] || { echo "--antes no puede ser la carpeta de salida: se borraría" >&2; exit 2; }
  compgen -G "$before/*.png" >/dev/null || { echo "no hay capturas en $before" >&2; exit 2; }
fi
rm -f "$out"/*.png "$out"/resumen*.md "$out"/titulos*.txt

export METALMIND_CAPTURAS="$out"
[[ -n "$before" ]] && export METALMIND_CAPTURAS_ANTES="$before"
if [[ -z "${VK_DRIVER_FILES:-}${VK_ICD_FILENAMES:-}" ]]; then
  for icd in /usr/share/vulkan/icd.d/lvp_icd*.json; do
    [[ -f "$icd" ]] && export VK_DRIVER_FILES="$icd" && break
  done
fi

status=0
(cd "$root" && cargo nextest run -p metalmind --test capturas --run-ignored only -E 'test(/^capturas::/)') || status=$?

{
  echo "| Captura | Píxeles distintos |"
  echo "|---|---|"
  cat "$out"/resumen-*.md 2>/dev/null || true
} >"$out/resumen.md"
cat "$out"/titulos-*.txt >"$out/titulos.txt" 2>/dev/null || true
echo "capturas en $out"
exit "$status"
