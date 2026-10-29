#!/usr/bin/env bash
# Rehace `crates/metalmind-ui/assets/tabler.deflate`, el set de íconos de
# Tabler que va embebido (05 §7), desde el paquete `@tabler/icons` de npm.
#
#   scripts/tabler.sh [versión]
#
# Cada línea es `nombre<tab>contenido`, ordenada por nombre: lo de adentro
# del `<svg>` (que pone `metalmind-ui`) en un `<g>` con el trazo o el relleno,
# y los rellenos con el sufijo `-filled`, como en Iconify. Todo va comprimido
# con deflate.
set -euo pipefail

root="$(cd "$(dirname "$0")/.." && pwd)"
version="${1:-$(curl -sf https://registry.npmjs.org/@tabler/icons/latest | node -e 'process.stdout.write(JSON.parse(require("fs").readFileSync(0)).version)')}"
work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

curl -sfL "https://registry.npmjs.org/@tabler/icons/-/icons-$version.tgz" | tar xz -C "$work"
node - "$work/package" "$root/crates/metalmind-ui/assets" "$version" <<'JS'
const fs = require("fs");
const zlib = require("zlib");
const [pkg, out, version] = process.argv.slice(2);
const escape = (value) => String(value).replace(/&/g, "&amp;").replace(/"/g, "&quot;").replace(/</g, "&lt;");
const element = ([tag, attrs]) =>
  `<${tag}${Object.entries(attrs).map(([k, v]) => ` ${k}="${escape(v)}"`).join("")}/>`;
const lines = [];
const styles = [
  ["outline", "", '<g fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">'],
  ["filled", "-filled", '<g fill="currentColor">'],
];
for (const [style, suffix, group] of styles) {
  const nodes = JSON.parse(fs.readFileSync(`${pkg}/tabler-nodes-${style}.json`));
  for (const [name, children] of Object.entries(nodes)) {
    lines.push(`${name}${suffix}\t${group}${children.map(element).join("")}</g>`);
  }
}
lines.sort((a, b) => (a.split("\t")[0] < b.split("\t")[0] ? -1 : 1));
fs.writeFileSync(`${out}/tabler.deflate`, zlib.deflateRawSync(lines.join("\n") + "\n", { level: 9 }));
fs.writeFileSync(`${out}/tabler.LICENSE`, fs.readFileSync(`${pkg}/LICENSE`));
fs.writeFileSync(`${out}/tabler.version`, `${version}\n`);
JS
echo "tabler $version: $(wc -c <"$root/crates/metalmind-ui/assets/tabler.deflate") bytes"
