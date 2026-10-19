#!/usr/bin/env bash
# Smoke sin GPU: corre `metalmind` bajo Xvfb con lavapipe y teclea con XTest
# (xdotool). Comprueba lo que se ve en disco y si el proceso sigue vivo.
#   scripts/smoke-x11.sh [ruta/al/binario]
# Necesita xvfb-run, xdotool y el driver Vulkan lavapipe (mesa-vulkan-drivers).
set -euo pipefail

if [[ -z "${METALMIND_SMOKE_INNER:-}" ]]; then
  exec env METALMIND_SMOKE_INNER=1 xvfb-run -a -s "-screen 0 1280x800x24" "$0" "$@"
fi

root="$(cd "$(dirname "$0")/.." && pwd)"
bin="$(realpath "${1:-$root/target/debug/metalmind}")"
lavapipe=/usr/share/vulkan/icd.d/lvp_icd.json
[[ -f "$lavapipe" ]] && export VK_ICD_FILENAMES="$lavapipe"
# Con render por CPU un frame tarda; esto da tiempo a que cada tecla se pinte.
pause="${METALMIND_SMOKE_PAUSE:-0.4}"

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT
# Config, datos y estado propios, sin autoguardado: los casos prueban guardar
# y salir como en vim, y nada toca la config ni el historial de quien lo corre.
export HOME="$work/home"
export XDG_CONFIG_HOME="$HOME/.config" XDG_DATA_HOME="$HOME/.local/share"
export XDG_STATE_HOME="$HOME/.local/state" XDG_CACHE_HOME="$HOME/.cache"
mkdir -p "$XDG_CONFIG_HOME/metalmind"
printf 'editor:\n  autosave: false\n' >"$XDG_CONFIG_HOME/metalmind/config.yaml"
failures=0

fail() {
  echo "  FALLA: $*" >&2
  failures=$((failures + 1))
}

# Abre Metalmind en una carpeta propia del caso y espera su ventana.
launch() {
  local case_dir="$1" file="$2"
  cd "$case_dir"
  "$bin" "$file" 2>>"$case_dir/metalmind.log" &
  pid=$!
  window="$(xdotool search --sync --onlyvisible --name 'Metalmind$' | head -1)"
  xdotool windowfocus --sync "$window"
  sleep "$pause"
}

keys() {
  xdotool key --delay 60 "$@"
  sleep "$pause"
}

typed() {
  xdotool type --delay 60 "$1"
  sleep "$pause"
}

alive() {
  kill -0 "$pid" 2>/dev/null
}

# Espera a que Metalmind salga; falla si sigue vivo.
expect_exit() {
  for _ in $(seq 50); do
    alive || { wait "$pid" || fail "salió con error"; return; }
    sleep 0.1
  done
  fail "$1"
  kill "$pid"
}

expect_alive() {
  alive || fail "$1"
}

expect_file() {
  local path="$1" expected="$2"
  if [[ ! -f "$path" ]]; then
    fail "no existe $path"
  elif [[ "$(cat "$path")" != "$expected" ]]; then
    fail "$path tiene $(printf '%q' "$(cat "$path")"), se esperaba $(printf '%q' "$expected")"
  fi
}

# Solo quedan los archivos esperados y el log: ningún temporal del guardado.
expect_only() {
  local case_dir="$1"
  shift
  local found wanted=""
  found="$(cd "$case_dir" && ls -A | { grep -v '^metalmind.log$' || true; } | sort | tr '\n' ' ')"
  if (($# > 0)); then
    wanted="$(printf '%s\n' "$@" | sort | tr '\n' ' ')"
  fi
  [[ "$found" == "$wanted" ]] || fail "en la carpeta hay: ${found:-nada} (se esperaba: ${wanted:-nada})"
}

new_case() {
  echo "$1"
  case_dir="$work/$2"
  mkdir -p "$case_dir"
}

new_case "archivo nuevo: i…<Esc>:wq lo crea" nuevo
launch "$case_dir" a.md
typed "ihola"
keys Escape
typed ":wq"
keys Return
expect_exit ":wq no cerró"
expect_file "$case_dir/a.md" "hola"
expect_only "$case_dir" a.md

new_case "con cambios, :q se niega y :q! sale sin guardar" cambios
launch "$case_dir" a.md
typed "ix"
keys Escape
typed ":q"
keys Return
expect_alive ":q cerró con cambios sin guardar"
typed ":q!"
keys Return
expect_exit ":q! no cerró"
expect_only "$case_dir"

new_case ":w conserva los permisos" permisos
printf 'uno\n' >"$work/permisos/a.md"
chmod 0640 "$work/permisos/a.md"
launch "$case_dir" a.md
typed "Ados"
keys Escape
typed ":w"
keys Return
typed ":q"
keys Return
expect_exit ":q tras :w no cerró"
expect_file "$case_dir/a.md" "unodos"
[[ "$(stat -c %a "$case_dir/a.md")" == 640 ]] || fail "los permisos cambiaron"
expect_only "$case_dir" a.md

new_case ":e abre otro archivo y el de antes queda oculto con sus cambios" editar
printf 'a1\na2\n' >"$work/editar/a.md"
printf 'b\n' >"$work/editar/b.md"
launch "$case_dir" a.md
typed "x"
typed ":e b.md"
keys Return
typed "Ax"
keys Escape
typed ":wq"
keys Return
expect_alive ":wq cerró aunque a.md tenía cambios"
typed ":wqa"
keys Return
expect_exit ":wqa no cerró"
expect_file "$case_dir/a.md" "1
a2"
expect_file "$case_dir/b.md" "bx"
expect_only "$case_dir" a.md b.md

new_case "ctrl-w v: dos paneles del mismo buffer, :q cierra de a uno" dividir
printf 'a\n' >"$work/dividir/a.md"
launch "$case_dir" a.md
keys ctrl+w v
typed "Ay"
keys Escape
keys ctrl+w h
typed ":w"
keys Return
typed ":q"
keys Return
expect_alive ":q con dos paneles cerró la ventana"
typed ":q"
keys Return
expect_exit ":q en el último panel no cerró"
expect_file "$case_dir/a.md" "ay"
expect_only "$case_dir" a.md

new_case "ctrl-w v y :e: cerrar la ventana mira todos los buffers" todos
printf 'a\n' >"$work/todos/a.md"
printf 'b\n' >"$work/todos/b.md"
launch "$case_dir" a.md
keys ctrl+w v
typed ":e b.md"
keys Return
typed "x"
keys ctrl+w h
typed ":q"
keys Return
expect_alive ":q cerró la ventana aunque quedaba el panel de b.md"
typed ":q"
keys Return
expect_alive ":q en el último panel perdió los cambios de b.md"
typed ":q!"
keys Return
expect_exit ":q! no cerró"
expect_file "$case_dir/b.md" "b"

if ((failures > 0)); then
  echo "$failures fallas; logs en los metalmind.log de cada caso" >&2
  trap - EXIT
  echo "carpeta de trabajo: $work" >&2
  exit 1
fi
echo "smoke en verde"
