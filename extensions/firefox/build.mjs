// Build script for the torille Firefox extension.
//
// Run from anywhere; paths are absolute:
//   node /home/j-p/torille/extensions/firefox/build.mjs
//
// Steps:
//   1. Compile src/**/*.ts -> dist/** via tsc.
//   2. Copy `options.html` (hand-written) into dist/options/.
//   3. Copy the wasm-pack artefacts from torille-wasm/pkg/ into
//      dist/spell-worker/ (the worker resolves them relative to
//      its own URL via `import.meta.url`).
//   4. Zip dist/ into extensions/firefox/torille-firefox.xpi.

import { copyFile, mkdir, readdir, rm } from 'node:fs/promises';
import { execFileSync } from 'node:child_process';
import path from 'node:path';
import { fileURLToPath } from 'url';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const repo = path.resolve(__dirname, '..', '..');
const pkgDir = path.join(repo, 'torille-wasm', 'pkg');
const dist = path.join(__dirname, 'dist');
const xpiOut = path.join(__dirname, 'torille-firefox.xpi');
const tsc = path.join(__dirname, 'node_modules', 'typescript', 'bin', 'tsc');
const tsconfig = path.join(__dirname, 'tsconfig.json');

/**
 * Print a "• <message>" banner to stdout for one build step.
 * @param {string} msg  Human-readable description of the step.
 */
function step(msg) { console.log(`\n• ${msg}`); }

// 1. Compile TS -> dist/. tsc only writes files, it doesn't clean
//    up stale artefacts (old .map files, removed sources), so we
//    wipe dist/ first.
step('cleaning dist/');
await rm(dist, { recursive: true, force: true });

step('compiling TypeScript');
execFileSync('node', [tsc, '-p', tsconfig], { stdio: 'inherit' });

// 2. options.html is hand-written; tsc only handles JS/TS so copy it
//    next to the compiled options.js. The manifest.json also goes
//    verbatim into the package root.
step('copying options.html');
await mkdir(path.join(dist, 'options'), { recursive: true });
await copyFile(
  path.join(__dirname, 'options.html'),
  path.join(dist, 'options', 'options.html'),
);

step('copying manifest.json');
await copyFile(
  path.join(__dirname, 'manifest.json'),
  path.join(dist, 'manifest.json'),
);

step('copying icons/');
await mkdir(path.join(dist, 'icons'), { recursive: true });
for (const entry of await readdir(path.join(__dirname, 'icons'))) {
  await copyFile(
    path.join(__dirname, 'icons', entry),
    path.join(dist, 'icons', entry),
  );
}

// 3. Wasm artefacts go in dist/spell-worker/ alongside the compiled
//    spell-worker.js. The worker resolves them via
//    `import init from './torille_wasm.mjs'` + the wasm-bindgen
//    shim's own `new URL(..., import.meta.url)`.
step('copying wasm-pack artefacts');
await mkdir(path.join(dist, 'spell-worker'), { recursive: true });
// The pkg is a bundler-target build: `torille_wasm.js` imports the
// `.wasm` as an ES module, which the extension's fetch cannot serve
// with the right MIME. `wasm-loader.ts` imports the glue directly and
// instantiates the bytes by hand, so the glue must ship by its
// generated name (`./torille_wasm_bg.js`) next to the worker.
await copyFile(
  path.join(pkgDir, 'torille_wasm_bg.js'),
  path.join(dist, 'spell-worker', 'torille_wasm_bg.js'),
);
await copyFile(
  path.join(pkgDir, 'torille_wasm_bg.wasm'),
  path.join(dist, 'spell-worker', 'torille_wasm_bg.wasm'),
);

// 4. Strip the empty `dist/types.js` (a pure `import type` module that
//    tsc leaves behind) so the .xpi doesn't ship a useless stub.
step('cleaning empty / dev-only artefacts');
await rm(path.join(dist, 'types.js'), { force: true });

// 5. Zip dist/ into the .xpi. The system `zip` is fine -- we don't
//    need a streaming zip writer, and Firefox happily reads the
//    stored (no-compression) zip an `.xpi` is.
step('zipping dist/ -> torille-firefox.xpi');
// Delete the old .xpi so the zip tool doesn't accumulate stale entries.
await rm(xpiOut, { force: true });
execFileSync('zip', ['-qr', xpiOut, '.'], { cwd: dist, stdio: 'inherit' });

console.log(`\nwrote ${xpiOut}`);