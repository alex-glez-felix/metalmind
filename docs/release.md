# Cómo se hace un release

Un release es un tag `vX.Y.Z` sobre `main`. Al empujarlo, el workflow
`.github/workflows/v-release.yml` compila, empaqueta y publica el GitHub
Release solo. El AUR se actualiza a mano después.

## Qué hay

| Pieza | Dónde | Qué hace |
|---|---|---|
| Config de dist | `dist-workspace.toml`, `[profile.dist]` en `Cargo.toml` | Objetivos, archivos incluidos, paquetes de apt para compilar. |
| Workflow | `.github/workflows/v-release.yml` | Generado por `dist generate`; no se edita a mano. Corre solo con tags `v*`. |
| Escritorio | `packaging/linux/` | `nota.desktop`, ícono SVG y metainfo de AppStream. Van en cada tarball. |
| AUR | `packaging/aur/nota-bin/`, `packaging/aur/nota-git/` | PKGBUILD y `.SRCINFO` de cada paquete. |
| Notas | `CHANGELOG.md` | dist usa la sección de la versión como texto del release. |

[dist](https://github.com/axodotdev/cargo-dist) es el nombre actual de cargo-dist: el
binario se llama `dist` y el crate sigue siendo `cargo-dist`.

Cada release sube, por arquitectura, `nota-<objetivo>.tar.xz` con su `.sha256`.
También sube `source.tar.gz` y `sha256.sum`. Cada tarball trae el binario
`nota`, `nota.desktop`, `nota.svg`, la metainfo, `README.md`, `LICENSE` y
`CHANGELOG.md`.

## Plataformas

Se publican `x86_64-unknown-linux-gnu` y `aarch64-unknown-linux-gnu`, cada una
compilada en un runner nativo de su arquitectura. Los runners son Ubuntu 22.04,
así que el binario pide glibc 2.35 o más nueva.

macOS y Windows no se declaran. GPUI tiene backend para los dos, pero nota no
funcionaría ahí sin trabajo extra:

- **Nunca se compiló ni se probó fuera de Linux.** El CI corre solo en Ubuntu.
  Si la compilación de un objetivo falla, falla el release entero.
- **macOS** necesita un bundle `.app`, firma y notarización para que Gatekeeper
  lo abra. dist entrega un tarball con el binario suelto.
- **Windows** abriría una consola junto a la ventana mientras el binario no
  declare `windows_subsystem = "windows"`.
- La lista de fuentes del tema solo tiene familias comunes en Linux.

Agregar uno es sumarlo a `targets` en `dist-workspace.toml`, correr `dist
generate` y resolver lo de arriba. Antes conviene sumar ese sistema al CI.

## Paso a paso

Hace falta `dist` en la misma versión que `cargo-dist-version`:

```sh
curl --proto '=https' --tlsv1.2 -LsSf https://github.com/axodotdev/cargo-dist/releases/download/v0.33.0/cargo-dist-installer.sh | sh
```

### 1. Commit de release

Desde una rama, con `main` en verde:

1. Subir `version` en `[workspace.package]` del `Cargo.toml` raíz y correr
   `cargo check` para que `Cargo.lock` la tome.
2. En `CHANGELOG.md`, renombrar `[Unreleased]` a `[X.Y.Z] - AAAA-MM-DD`, abrir
   una `[Unreleased]` vacía arriba y actualizar los enlaces del final:

   ```md
   [Unreleased]: https://github.com/alex-glez-felix/nota/compare/vX.Y.Z...HEAD
   [X.Y.Z]: https://github.com/alex-glez-felix/nota/releases/tag/vX.Y.Z
   ```

3. Agregar la versión al principio de `<releases>` en la metainfo. Si no existe,
   crear el bloque antes de `</component>`:

   ```xml
   <releases>
     <release version="X.Y.Z" date="AAAA-MM-DD"/>
   </releases>
   ```

4. Comprobar que dist ve la versión y las notas:

   ```sh
   dist plan --tag vX.Y.Z
   dist manifest --tag vX.Y.Z --output-format=json | jq -r .announcement_changelog
   appstreamcli validate --no-net packaging/linux/io.github.alex_glez_felix.nota.metainfo.xml
   ```

5. PR con `chore(release): vX.Y.Z` y merge a `main`.

### 2. Tag

```sh
git switch main && git pull
git tag -a vX.Y.Z -m "vX.Y.Z"
git push origin vX.Y.Z
gh run watch "$(gh run list --workflow v-release.yml --limit 1 --json databaseId -q '.[0].databaseId')"
```

El tag tiene que coincidir con la versión del `Cargo.toml`. Si no, el job
`plan` falla y no se publica nada. Un sufijo como `v0.2.0-rc.1` marca el release
como prerelease.

### 3. Revisar el release

```sh
gh release view vX.Y.Z
gh release download vX.Y.Z -p 'nota-x86_64-unknown-linux-gnu.tar.xz*' -D /tmp/nota-release
cd /tmp/nota-release && sha256sum -c nota-x86_64-unknown-linux-gnu.tar.xz.sha256
tar -xJf nota-x86_64-unknown-linux-gnu.tar.xz && ./nota-x86_64-unknown-linux-gnu/nota --version
```

### Si algo sale mal

Si el workflow falla antes del job `host`, no se publicó nada. Se arregla en
`main`, se borra el tag y se vuelve a crear:

```sh
git push --delete origin vX.Y.Z && git tag -d vX.Y.Z
```

Si el release ya se publicó con un error, no se reescribe: se publica `vX.Y.Z+1`.

## AUR

Los dos paquetes viven en `packaging/aur/` y se publican copiando el
`PKGBUILD` y el `.SRCINFO` a su repo del AUR. Hace falta una cuenta en
aur.archlinux.org con una llave SSH.

### nota-bin, en cada release

```sh
cd packaging/aur/nota-bin
sed -i 's/^pkgver=.*/pkgver=X.Y.Z/; s/^pkgrel=.*/pkgrel=1/' PKGBUILD
updpkgsums                         # baja los tarballs de las dos arquitecturas
makepkg --printsrcinfo > .SRCINFO
makepkg -f && namcap PKGBUILD nota-bin-*.pkg.tar.zst
```

`updpkgsums` reemplaza los `SKIP` por las sumas reales. Cuando la versión no
cambia y solo cambia el PKGBUILD, se sube `pkgrel`. Luego se hace el commit en
este repo y se sube al AUR:

```sh
git clone ssh://aur@aur.archlinux.org/nota-bin.git /tmp/aur-nota-bin   # la primera vez crea el paquete
cp PKGBUILD .SRCINFO /tmp/aur-nota-bin/
cd /tmp/aur-nota-bin && git add PKGBUILD .SRCINFO && git commit -m "X.Y.Z-1" && git push
```

### nota-git, una sola vez

`nota-git` compila `main` y calcula su versión con `pkgver()`, así que no se
actualiza con cada release. Se sube la primera vez igual que `nota-bin`, con
`ssh://aur@aur.archlinux.org/nota-git.git`. Después, solo cuando cambia su
`PKGBUILD`, regenerando antes el `.SRCINFO`.

`namcap` avisa `Reference to x86_64 should be changed to $CARCH` en `nota-bin`.
Es un falso positivo: lo provocan `source_x86_64` y `sha256sums_x86_64`, que
son la forma documentada de dar fuentes por arquitectura.

## Actualizar dist

```sh
dist selfupdate            # o instalar la versión nueva con el script de arriba
dist init --yes            # sube cargo-dist-version y regenera el workflow
dist plan
```

`dist init` reescribe `v-release.yml`. Cualquier cambio va en
`dist-workspace.toml`, nunca en el workflow.
