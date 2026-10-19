#!/usr/bin/env bash
# Corre `medir` en procesos nuevos (la primera fórmula de cada uno es la
# primera de verdad) y da la mediana de cada campo del resumen.
#   ./medir.sh [corridas] [escala]
set -euo pipefail
cd "$(dirname "$0")"
runs="${1:-7}"
scale="${2:-1}"
cargo build --release --bin medir >/dev/null
for motor in typst ratex mathml mathcore; do
  for _ in $(seq "$runs"); do
    ./target/release/medir "$motor" "$scale" | grep '^resumen'
  done | awk -F'\t' '
    {
      for (i = 2; i <= NF; i++) {
        split($i, kv, "="); key[i] = kv[1]; values[i, NR] = kv[2]
      }
      fields = NF; rows = NR
    }
    END {
      for (i = 2; i <= fields; i++) {
        n = 0
        for (r = 1; r <= rows; r++) sorted[++n] = values[i, r]
        if (sorted[1] ~ /^[0-9.]+$/) {
          for (a = 2; a <= n; a++) {
            v = sorted[a]
            for (b = a - 1; b >= 1 && sorted[b] + 0 > v + 0; b--) sorted[b + 1] = sorted[b]
            sorted[b + 1] = v
          }
        }
        if (n % 2 == 0 && sorted[1] ~ /^[0-9.]+$/) value = (sorted[n / 2] + sorted[n / 2 + 1]) / 2
        else value = sorted[int((n + 1) / 2)]
        printf "%s=%s\t", key[i], value
        delete sorted
      }
      print ""
    }'
done
