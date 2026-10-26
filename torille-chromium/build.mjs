// Build the torille Chromium MV3 extension.
//
// Run from anywhere; paths are absolute:
//   node /home/j-p/torille/torille-chromium/build.mjs
//
// Steps:
//   1. Validate the manifest JSON (manifest_version === 3, no Firefox
//      `browser_specific_settings`, `background.service_worker` set).
//   2. Compile src/**/*.ts -> dist/** via tsc.
//   3. Copy `options.html` and `icons/` and `manifest.json` into dist/.
//   4. Copy wasm-pack artefacts from torille-wasm/pkg/ into
//      dist/spell-worker/.
//   5. Build a sibling `torille-chromium.zip` from dist/.

import { copyFile, mkdir, readdir, readFile, rm } from 'node:fs/promises';
import { execFileSync } from 'node:child_process';
import path from 'node:path';
import { fileURLToPath } from 'url';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const repo = path.resolve(__dirname, '..');
const pkgDir = path.join(repo, 'torille-wasm', 'pkg');
const dist = path.join(__dirname, 'dist');
const zipOut = path.join(repo, 'torille-chromium.zip');
const tsc = path.join(__dirname, 'node_modules', 'typescript', 'bin', 'tsc');
const tsconfig = path.join(__dirname, 'tsconfig.json');

/**
 * Print a "• <message>" banner to stdout for one build step.
 * @param {string} msg  Human-readable description of the step.
 */
function step(msg) { console.log(`\n• ${msg}`); }

/**
 * Read and parse `${__dirname}/manifest.json`, then assert that it
 * is shaped for Chrome MV3 (no Firefox-only `browser_specific_settings`,
 * `background.service_worker` is set). Aborts the build with a clear
 * message if any check fails.
 * @returns {Promise<Record<string, unknown>>}  The parsed manifest.
 */
async function loadAndValidateManifest() {
  const manifest = JSON.parse(
    await readFile(path.join(__dirname, 'manifest.json'), 'utf8'));
  if (manifest.manifest_version !== 3) {
    console.error('manifest_version is not 3; refusing to build a Chromium MV3 target');
    process.exit(1);
  }
  if (manifest.browser_specific_settings) {
    console.error('manifest contains browser_specific_settings; that\'s Firefox-only');
    process.exit(1);
  }
  if (!manifest.background?.service_worker) {
    console.error('background.service_worker is required for Chrome MV3');
    process.exit(1);
  }
  return manifest;
}

// 1. Validate manifest.
step('validating manifest.json');
await loadAndValidateManifest();

// 2. Wipe dist/ and run the compiler.
await rm(dist, { recursive: true, force: true });
step('compiling TypeScript');
execFileSync('node', [tsc, '-p', tsconfig], { stdio: 'inherit' });

// 3. Copy hand-written HTML, manifest, icons.
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

// 4. Wasm artefacts.
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

// 5. Strip the empty `dist/types.js` (a pure `import type` module that
//    tsc leaves behind) so the package doesn't ship a useless stub.
step('cleaning empty / dev-only artefacts');
await rm(path.join(dist, 'types.js'), { force: true });

// 6. Zip dist/ for share-and-load.
step('zipping dist/ -> torille-chromium.zip');
await rm(zipOut, { force: true });
try {
  execFileSync('zip', ['-qr', zipOut, '.'], { cwd: dist, stdio: 'inherit' });
} catch (e) {
  console.warn(
    `could not build ${zipOut} (${e.message}); ` +
    '`dist/` is still the load target -- "Load unpacked" on Chrome still works.',
  );
}

console.log(`\nwrote ${dist}/ and ${zipOut}`);