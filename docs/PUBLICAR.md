# Publicación y preservación

## Publicar una versión con el CI (no compiles a mano)

El repo tiene dos workflows:

| Workflow | Cuándo corre | Qué hace |
|---|---|---|
| `ci.yml` | **todo push y todo pull request** | `cargo test` de la lógica, `npm ci` + build de la UI, y `cargo test --workspace` en Ubuntu con dependencias de Tauri |
| `release.yml` | **solo al empujar una etiqueta `v*`** | matriz `windows-latest` + `ubuntu-24.04` + `macos-latest` (universal): compila `.exe`/`.msi`, `.deb`/`.rpm`/`.AppImage` y `.dmg`, los firma con minisign, crea la release y sube `latest.json` (el que consulta el actualizador) |

La firma usa secretos del **repositorio de la organización**: la release con firma solo se produce empujando la etiqueta en `SlimefunNewHorizons/DrakeStar-Launcher` (o ejecutándolo manualmente desde Actions con una cuenta con acceso).

### 1) Los tres archivos que tienen que coincidir

| Archivo | Clave |
|---|---|
| `Cargo.toml` | `version` de `[workspace.package]` (la heredan todas las crates) |
| `src-tauri/tauri.conf.json` | `"version"` — **el que compara el workflow**: si la etiqueta no es `v` + esta versión, falla |
| `ui/package.json` | `"version"` |

Y de yapa, para que nada se queje: `Cargo.lock` (los paquetes `drakestar-*`) y la raíz de `ui/package-lock.json`.

### 2) El camino corto (recomendado): el script

```bash
node tools/bump-release.mjs 0.7.3            # o: npm run bump-release -- 0.7.3
```

Sincroniza los archivos de arriba, commitea `release v0.7.3`, crea la etiqueta anotada y empuja rama + etiqueta. Opciones útiles: `--dry-run` (ver sin tocar nada), `--no-git`, `--no-push`, `--remote org`, `--message "..."`, `--allow-dirty`. Se niega a seguir si la etiqueta ya existe (en local o en el remoto), si el árbol está sucio con cambios ajenos o si estás en HEAD suelto.

El equivalente a mano:

```bash
# edita los tres archivos (y Cargo.lock: cargo check --offline lo deja acorde)
git commit -am "release v0.7.3"
git tag -a v0.7.3 -m "DrakeStar 0.7.3"
git push origin main            # o la rama que corresponda
git push origin v0.7.3          # ESTO es el que despierta release.yml
```

Después se vigila en:
`https://github.com/SlimefunNewHorizons/DrakeStar-Launcher/actions/workflows/release.yml`
y la release publicada queda en `.../releases/tag/v0.7.3`.

### 3) Si trabajas desde un fork (`DrgX89/DrakeStar-Launcher`)

- `push` de tu rama y **pull request** al org: `ci.yml` corre solo (pruebas y build; en PRs de fork los secretos **nunca** se exponen, así que ahí no puede firmar: es a propósito).
- El `release.yml` se dispara **solo con etiquetas en el repo que las recibe**: etiquetar en tu fork compila en tu namespace y falla o queda sin firmar (la clave minisign no está ahí).
- Rutas para publicar de verdad desde un fork:
  1. Abre el PR, deja que `ci.yml` pase en verde, y que un mantenedor con acceso lo fusione; luego etiqueta **en el org** (o pídele que lo haga):

     ```bash
     git remote add org https://github.com/SlimefunNewHorizons/DrakeStar-Launcher.git
     git fetch org
     git checkout -b release-0.7.3 org/main   # o fusiona tu PR y Main
     git tag -a v0.7.3 -m "DrakeStar 0.7.3"
     git push org v0.7.3
     ```
  2. Si ya tienes permisos de escritura en el org, es directo: `git push org main` + `git push org v0.7.3`.
  3. Alternativa sin tocar etiquetas: añadir `workflow_dispatch:` a `release.yml` (una línea en el `on:`) y lanzarlo desde *Actions → Release → Run workflow* eligiendo la rama del PR. Útil si el mantenedor no quiere etiquetar sin revisar.

## Auditoría previa a publicar (realizada sobre el commit `872728a`)

| Comprobación | Resultado |
|---|---|
| Secretos (tokens, claves privadas, contraseñas, `.env`) | ninguno encontrado (búsqueda por patrones en todo el árbol) |
| Rutas locales del entorno de trabajo en el código | ninguna |
| Archivos generados versionados (`node_modules`, `target`, `ui/dist`, `ui/public/brand`) | ninguno; excluidos por `.gitignore` |
| Archivos grandes | el mayor es una captura PNG de ~244 KB; historial total ~4 MB |
| Licencia del proyecto | MIT (`LICENSE`) |
| Licencias de dependencias | Rust: MIT / Apache-2.0 / Unicode-3.0 / Zlib / BSD / ISC / MPL-2.0 (5 paquetes) / CDLA-Permissive-2.0; npm: MIT, Apache-2.0, ISC, BSD-3-Clause. Todas permisivas o de copyleft débil a nivel de archivo (MPL-2.0); sin GPL/AGPL |
| Código o marca de Horizon copiados | ninguno (`git ls-files` no contiene archivos de Horizon); solo se cita como inspiración |
| Autoría del historial | 7 commits con autor `Claude <noreply@anthropic.com>` y trailers `Co-Authored-By` / `Claude-Session`. Si prefieres que figure otra autoría, reescribe el historial **antes** del primer push (`git rebase --root --exec 'git commit --amend --reset-author --no-edit'` con tu `user.name`/`user.email`) |
| Capturas | 3 son de la app Tauri real (Xvfb), 10 del modo navegador con mock, etiquetadas |

## Publicar en el repositorio de la organización

```bash
# 1. Crear el repositorio vacío (sin README/licencia/.gitignore):
#    https://github.com/organizations/SlimefunNewHorizons/repositories/new  ->  DrakeStar-Launcher
# 2. Restaurar desde el bundle y subir:
git clone DrakeStar-Launcher.bundle DrakeStar-Launcher
cd DrakeStar-Launcher
git remote set-url origin https://github.com/SlimefunNewHorizons/DrakeStar-Launcher.git
git push -u origin main
```

Verificar el bundle antes de usarlo: `git bundle verify DrakeStar-Launcher.bundle`.

## Preservación
El bundle contiene todo el historial (`main`). Guárdalo fuera de la sesión (la sesión y su contenedor son efímeros).
