#!/usr/bin/env node
//! Subir la version del launcher y publicar con el CI de GitHub Actions.
//!
//! Hace el trabajo sucio de un release: sincroniza los TRES archivos que tienen
//! que coincidir (Cargo.toml, src-tauri/tauri.conf.json, ui/package.json), deja
//! Cargo.lock acorde, commitea, etiqueta `vX.Y.Z` y empuja. El workflow
//! `.github/workflows/release.yml` compila en Windows/Ubuntu/macOS, firma con la
//! clave minisign y sube `latest.json`.
//!
//! Uso:
//!   npm run bump-release -- 0.7.2
//!   node tools/bump-release.mjs 0.7.2 --remote org --message "DrakeStar 0.7.2"
//!
//! Opciones:
//!   --dry-run   muestra los cambios sin escribir ni tocar git
//!   --no-git    solo sincroniza los archivos (nada de commit/tag/push)
//!   --no-push   commitea y etiqueta en local, no empuja
//!   --remote R  remoto al que empujar (por defecto origin)
//!   --message M texto del commit (por defecto "release vX.Y.Z")
//!   --allow-dirty permite seguir con el arbol de trabajo sucio
//!
//! Notas:
//!   - La etiqueta tiene que ser vX.Y.Z y coincidir con la version de
//!     tauri.conf.json: el workflow lo comprueba y falla si no coinciden.
//!   - Los secretos de firma viven en el repositorio de la organizacion: el
//!     release con firma solo se produce empujando la etiqueta ALLA.

import { execFileSync } from 'node:child_process';
import { readFileSync, writeFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import path from 'node:path';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const VERSION_RE = /^(\d+)\.(\d+)\.(\d+)(-[0-9A-Za-z.-]+)?$/;

function fail(msg) {
  console.error(`Error: ${msg}`);
  process.exit(1);
}
function git(args, { capture = true } = {}) {
  const stdio = capture ? ['ignore', 'pipe', 'pipe'] : 'inherit';
  try {
    return execFileSync('git', args, { cwd: root, stdio, encoding: 'utf8' });
  } catch (e) {
    const err = (e.stderr || e.message || '').toString().trim();
    fail(`git ${args.join(' ')}: ${err}`);
  }
}

// ---------- argumentos ----------
const args = process.argv.slice(2);
const version = args.find((a) => !a.startsWith('--'));
const opt = (name) => args.includes(`--${name}`);
const value = (name) => {
  const i = args.indexOf(`--${name}`);
  return i >= 0 ? args[i + 1] : undefined;
};
if (!version) {
  console.error('Uso: node tools/bump-release.mjs <X.Y.Z> [--dry-run] [--no-git] [--no-push] [--remote R] [--message M]');
  process.exit(2);
}
if (!VERSION_RE.test(version)) {
  fail(`"${version}" no es una version X.Y.Z valida (con -pre-release opcional)`);
}
const dryRun = opt('dry-run');
const noGit = opt('no-git');
const noPush = opt('no-push');
const remote = value('remote') ?? 'origin';
const tag = `v${version}`;
const message = value('message') ?? `release ${tag}`;

// ---------- archivos versionados ----------
function edit(file, transform, note) {
  const p = path.join(root, file);
  const before = readFileSync(p, 'utf8');
  const after = transform(before);
  if (before === after) return null;
  console.log(`  ~ ${file}${note ? ` (${note})` : ''}`);
  if (!dryRun) writeFileSync(p, after);
  return { before, after };
}

const changes = [];
const bumpWorkspace = (text) =>
  text.replace(/(\[workspace\.package\][^\[]*?^version = ")[^"]*(")/ms, `$1${version}$2`);
const bumpKey = (text) => text.replace(/("version"\s*:\s*")[^"]*(")/, `$1${version}$2`);
const bumpKeyAll = (text) => text.replace(/("version"\s*:\s*")[^"]*(")/g, `$1${version}$2`);
// Cargo.lock: solo los paquetes del workspace (los externos no se tocan)
const bumpLock = (text) =>
  text.replace(/(name = "drakestar[^"]*"\r?\nversion = ")[^"]*(")/g, `$1${version}$2`);
// package-lock.json: la raiz y la entrada "" (las dependencias tienen su propia version)
const bumpLockJson = (text) =>
  text
    .replace(/("version"\s*:\s*")[^"]*(")/, `$1${version}$2`)
    .replace(/(""\s*:\s*\{[^}]*?"version"\s*:\s*")[^"]*(")/s, `$1${version}$2`);

console.log(`\n== DrakeStar ${version} (etiqueta ${tag})`);
console.log(dryRun ? '== modo dry-run: no se escribe nada\n' : '');

for (const [file, transform, note] of [
  ['Cargo.toml', bumpWorkspace, 'version del workspace'],
  ['src-tauri/tauri.conf.json', bumpKey, 'la que comprueba el workflow'],
  ['ui/package.json', bumpKey, ''],
  ['ui/package-lock.json', bumpLockJson, 'raiz del lock de npm'],
  ['Cargo.lock', bumpLock, 'paquetes drakestar-*'],
]) {
  const change = edit(file, transform, note);
  if (change) changes.push({ file, ...change });
  else if (dryRun) console.log(`  = ${file} (ya esta en ${version})`);
}

if (dryRun) {
  console.log('\n== dry-run terminado. Repite sin --dry-run para aplicarlo.\n');
  process.exit(0);
}
if (changes.length === 0) {
  console.log(`Los archivos ya estan en ${version}.`);
} else {
  console.log(`\n${changes.length} archivo(s) actualizado(s).`);
}

// ---------- git ----------
if (noGit) {
  console.log('--no-git: commitea y etiqueta tu cuando quieras:');
  console.log(`  git commit -am "${message}"`);
  console.log(`  git tag -a ${tag} -m "${message}"`);
  console.log(`  git push ${remote} ${tag}`);
  process.exit(0);
}

if (git('rev-parse --is-inside-work-tree').trim() !== 'true') {
  fail('no estamos en un repositorio git');
}
const branch = git('rev-parse --abbrev-ref HEAD').trim();
if (branch === 'HEAD') fail('HEAD suelto (detached): haz checkout de una rama antes de publicar');

if (!opt('allow-dirty')) {
  const dirty = git('status --porcelain').trim();
  const mine = new Set(changes.map((c) => c.file));
  const foreign = dirty
    .split('\n')
    .filter(Boolean)
    .filter((l) => {
      const f = l.slice(3).split(' -> ').pop();
      return !mine.has(f) && !f.startsWith('ui/dist') && !f.startsWith('target');
    });
  if (foreign.length > 0) {
    fail('el arbol de trabajo tiene cambios ajenos al release:\n  ' + foreign.join('\n  ') + '\nHaz commit o usa --allow-dirty.');
  }
}

// la etiqueta no puede existir ni aqui ni alla (el workflow fallaria al duplicar el release)
const tagLocal = (() => {
  try {
    execFileSync('git', ['rev-parse', '-q', '--verify', `refs/tags/${tag}`], { cwd: root, stdio: 'ignore' });
    return true;
  } catch {
    return false;
  }
})();
if (tagLocal) fail(`la etiqueta ${tag} ya existe en local (git tag -d ${tag} para borrarla)`);
const remoteTags = git(`ls-remote --tags ${remote} refs/tags/${tag}`).trim();
if (remoteTags) fail(`la etiqueta ${tag} ya existe en '${remote}': ya se publico`);

const remotes = git('remote').trim().split('\n').filter(Boolean);
if (!remotes.includes(remote)) {
  fail(`el remoto '${remote}' no existe. Remotos: ${remotes.join(', ') || '(ninguno)'}\n` +
    `  Anade el de la organizacion con:\n` +
    `  git remote add org https://github.com/SlimefunNewHorizons/DrakeStar-Launcher.git`);
}

git(['add', ...changes.map((c) => c.file)]);
git(['commit', '-m', message, ...(changes.length ? [] : ['--allow-empty'])]);
git(['tag', '-a', tag, '-m', message]);
console.log(`\ncommit y etiqueta ${tag} creados en la rama ${branch}.`);

if (noPush) {
  console.log('--no-push: empuja cuando quieras:');
  console.log(`  git push ${remote} ${branch}`);
  console.log(`  git push ${remote} ${tag}`);
  process.exit(0);
}

console.log(`\n== empujando a '${remote}'`);
git(['push', remote, `HEAD:${branch}`], { capture: false });
git(['push', remote, tag], { capture: false });

console.log(`\nListo. El workflow Release deberia estar corriendo:`);
console.log(`  https://github.com/SlimefunNewHorizons/DrakeStar-Launcher/actions/workflows/release.yml`);
console.log(`Cuando termine, la release quedara en:`);
console.log(`  https://github.com/SlimefunNewHorizons/DrakeStar-Launcher/releases/tag/${tag}\n`);