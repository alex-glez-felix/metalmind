#!/usr/bin/env bash
# Baja mitex 0.2.4 de crates.io a `vendor/mitex` y le aplica el parche que
# hace que convierta con la spec que se le pasa (ver README).
set -euo pipefail
cd "$(dirname "$0")"
sum=81f1eb466efdd212fe624f8bf24083f21d582f2d08e3b879bc4a059b25969d6c
work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT
curl -sfL -A metalmind-spike-math https://crates.io/api/v1/crates/mitex/0.2.4/download -o "$work/mitex.crate"
echo "$sum  $work/mitex.crate" | sha256sum -c --quiet
tar -xzf "$work/mitex.crate" -C "$work"
sed -i 's/\r$//' "$work/mitex-0.2.4/src/converter.rs"
patch -s -d "$work/mitex-0.2.4" -p1 <assets/mitex-0.2.4-spec.patch
rm -rf vendor/mitex
mkdir -p vendor
mv "$work/mitex-0.2.4" vendor/mitex
echo "mitex 0.2.4 parcheado en vendor/mitex"
