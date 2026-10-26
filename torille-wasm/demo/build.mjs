// esbuild build script for the Solid + TypeScript demo.
//
// Solid's JSX transform runs through babel-preset-solid because that's
// what Solid's official tooling uses; esbuild-plugin-solid wraps it for
// esbuild.
//
// Two entry points:
//   * `src/main.tsx` -- the Solid editor, runs on the main thread.
//   * `src/spell-worker.ts` -- the wasm host, runs in a module worker
//                              (`{ type: 'module' }`) so spell-check
//                              never blocks typing.
//   * `src/suggestion-worker.ts` -- a minimal sibling of the above
//                                  used only for the per-textbox
//                                  render's parallel post-pass
//                                  (suggestion fan-out + slow
//                                  check-known). Spawned as a pool
//                                  from the main thread so several
//                                  wasm calls run concurrently.
//
// The wasm is loaded manually at runtime: wasm-loader.ts fetches
// `dist/torille_wasm_bg.wasm` as bytes and instantiates it with
// `WebAssembly.instantiate` (the wasm-bindgen shim's own loading
// path needs the .wasm served as an ES module with a
// `Content-Type: application/wasm`, which the dev workflow's
// fallback server doesn't do). We copy the .wasm from wasm-pack's
// `pkg/` output to `dist/` so that fetch resolves; the glue JS
// (`pkg/torille_wasm_bg.js`) is imported by the loader and so is
// bundled into each worker.
//
// Why the wasm-external plugin below still exists: if a `.wasm`
// import ever creeps back into the source graph (e.g. someone
// re-imports the wasm-bindgen wrapper), marking it external keeps
// it a runtime fetch. The alternative -- esbuild's
// `loader: { '.wasm': 'binary' }` -- inlines the bytes as a
// base64 `Uint8Array` default export, which is not a wasm
// exports namespace and crashes any `wasm.__wbindgen_start()`
// call at module load.
//
// Usage:
//   node build.mjs           # development build, source maps
//   node build.mjs --release # minified, no source maps
//   node build.mjs --watch   # rebuild on every src/ edit
//                            # (the dev server serves the freshly
//                            #  built bundle immediately)
//
// `--watch` exists because the demo server
// (`cargo run --release -p torille-demo-server`) does NOT rebuild --
// it just serves whatever happens to be in dist/. Without `--watch`,
// edits to src/ are invisible to the browser until you re-run
// `node build.mjs` manually, which has bitten us before: the page logs
// looked stale, the `onConfirmedMisspell` callback seemed never to fire,
// etc., all because dist/main.js hadn't been rebuilt since the last edit.

import * as esbuild from 'esbuild';
import { solidPlugin } from 'esbuild-plugin-solid';
import { copyFile, mkdir, rm } from 'node:fs/promises';
import { dirname, join, resolve as pathResolveAbs } from 'path';
import { fileURLToPath } from 'url';

const __dirname = dirname(fileURLToPath(import.meta.url));
const isRelease = process.argv.includes('--release');
const isWatch = process.argv.includes('--watch');

// Wipe dist/ before each build so the loose `torille_wasm_bg.wasm`
// from a previous build doesn't linger. In watch mode we only do
// this once at start; subsequent rebuilds reuse the dist dir.
if (!isWatch) {
  await rm(join(__dirname, 'dist'), { recursive: true, force: true });
}
await mkdir(join(__dirname, 'dist'), { recursive: true });

// Guard: keep any `.wasm` import in the source graph external, so
// it stays a runtime fetch rather than an inlined base64 blob.
// The loader fetches the bytes itself (see the header).
const wasmExternalPlugin = {
  name: 'wasm-external',
  setup(build) {
    build.onResolve({ filter: /\.wasm$/ }, (args) => ({
      path: args.path,
      external: true,
    }));
  },
};

const buildOptions = {
  entryPoints: [
    pathResolveAbs(__dirname, 'src/main.tsx'),
    pathResolveAbs(__dirname, 'src/spell-worker.ts'),
    pathResolveAbs(__dirname, 'src/suggestion-worker.ts'),
  ],
  bundle: true,
  outdir: join(__dirname, 'dist'),
  entryNames: '[name]',
  format: 'esm',
  target: 'es2022',
  plugins: [solidPlugin(), wasmExternalPlugin],
  sourcemap: !isRelease,
  minify: isRelease,
  metafile: true,
  logLevel: 'info',
  absWorkingDir: __dirname,
};

if (isWatch) {
  // `context()` + `watch()` rebuilds on every src/ change. The
  // dev server picks up the new dist/main.js on its next fetch.
  // We do NOT re-copy the .wasm on every change -- it's the
  // same artifact until `wasm-pack` reruns.
  const ctx = await esbuild.context(buildOptions);
  await ctx.watch();
  console.log('watching src/ for changes (Ctrl-C to stop)');
} else {
  const result = await esbuild.build(buildOptions);
  console.log(`\nwrote dist/main.js, dist/spell-worker.js, dist/torille_wasm_bg.wasm`);
  if (result.metafile) {
    for (const [file, info] of Object.entries(result.metafile.outputs)) {
      console.log(`  ${file}  ${info.bytes} bytes`);
    }
  }
}

// Copy the wasm next to the bundles so the runtime fetch in
// wasm-loader.ts resolves. Nothing else from pkg/ is needed: the
// glue (`torille_wasm_bg.js`) is bundled into each worker by
// esbuild, and the loader instantiates the .wasm by hand instead
// of importing `torille_wasm.js` (which would drag the browser
// into WebAssembly ES-module integration and its
// `application/wasm` MIME requirement -- see wasm-loader.ts).
await copyFile(
  join(__dirname, '../pkg/torille_wasm_bg.wasm'),
  join(__dirname, 'dist/torille_wasm_bg.wasm'),
);
