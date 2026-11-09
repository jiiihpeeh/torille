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
import { copyFile, mkdir, readFile, readdir, rename, rm, stat, utimes, writeFile } from 'node:fs/promises';
import { brotliCompress, constants as zlibConstants } from 'node:zlib';
import { promisify } from 'node:util';
import { dirname, join, resolve as pathResolveAbs } from 'path';
import { fileURLToPath } from 'url';

const __dirname = dirname(fileURLToPath(import.meta.url));
const isRelease = process.argv.includes('--release');
const isWatch = process.argv.includes('--watch');

const brotliCompressAsync = promisify(brotliCompress);

// Wipe dist/ before each build so the loose `torille_wasm_bg.wasm`
// from a previous build doesn't linger. Keep `*.br`: the wasm sidecar
// is keyed to `pkg/`'s mtime, and recompressing 17 MB on every
// `node build.mjs` (two minutes after the last one) is pure latency.
// In watch mode we only do this once at start; subsequent rebuilds
// reuse the dist dir.
if (!isWatch) {
  const dist = join(__dirname, 'dist');
  const entries = await readdir(dist).catch(() => []);
  await Promise.all(
    entries
      .filter((name) => !name.endsWith('.br'))
      .map((name) => rm(join(dist, name), { recursive: true, force: true })),
  );
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

// Copy the wasm next to the bundles so the runtime fetch in
// wasm-loader.ts resolves. Nothing else from pkg/ is needed: the
// glue (`torille_wasm_bg.js`) is bundled into each worker by
// esbuild, and the loader instantiates the .wasm by hand instead
// of importing `torille_wasm.js` (which would drag the browser
// into WebAssembly ES-module integration and its
// `application/wasm` MIME requirement -- see wasm-loader.ts).
//
// Keep the copied file's mtime equal to pkg/'s. `serve-demo` only
// trusts the `.br` sidecar when it is newer than the `.wasm`, and a
// copy stamped "now" would look newer than a sidecar written seconds
// earlier by the previous build.
const wasmSrc = join(__dirname, '../pkg/torille_wasm_bg.wasm');
const wasmDst = join(__dirname, 'dist/torille_wasm_bg.wasm');
const wasmSrcStat = await stat(wasmSrc);
await copyFile(wasmSrc, wasmDst);
await utimes(wasmDst, wasmSrcStat.atime, wasmSrcStat.mtime);

// Pre-compress the wasm. It is the largest asset by an order of
// magnitude (17 MB), and brotli takes it to ~4.3 MB. `serve-demo`
// serves this `.br` sidecar instead of compressing the same bytes on
// every request or process start. Quality 9 costs ~11 s for the 17 MB
// file; 11 would be ~38 s for ~0.6 MB more, which is not worth it for
// a dev server. If the sidecar is already newer than `pkg/`'s wasm,
// skip: repeated builds and `--watch` restarts stay fast.
const WASM_BROTLI_QUALITY = 9;

async function precompressWasm() {
  const dst = `${wasmDst}.br`;
  const [srcStat, dstStat] = await Promise.all([
    stat(wasmSrc),
    stat(dst).catch(() => null),
  ]);
  if (dstStat && dstStat.size > 0 && dstStat.mtimeMs >= srcStat.mtimeMs) {
    console.log(`  dist/torille_wasm_bg.wasm.br  ${dstStat.size} bytes (up to date)`);
    return dstStat.size;
  }
  const started = performance.now();
  const raw = await readFile(wasmSrc);
  const compressed = await brotliCompressAsync(raw, {
    params: {
      [zlibConstants.BROTLI_PARAM_QUALITY]: WASM_BROTLI_QUALITY,
      [zlibConstants.BROTLI_PARAM_LGWIN]: 24,
      [zlibConstants.BROTLI_PARAM_SIZE_HINT]: raw.length,
    },
  });
  // Write then rename: a watcher (or serve-demo) must never observe a
  // half-written sidecar.
  await writeFile(`${dst}.tmp`, compressed);
  await rename(`${dst}.tmp`, dst);
  const seconds = ((performance.now() - started) / 1000).toFixed(1);
  console.log(
    `  dist/torille_wasm_bg.wasm.br  ${compressed.length} bytes ` +
      `(${(compressed.length / raw.length * 100).toFixed(1)}% of ${raw.length}, ${seconds}s)`,
  );
  return compressed.length;
}

const wasmBrBytes = await precompressWasm();

const buildOptions = {
  entryPoints: [
    pathResolveAbs(__dirname, 'src/main.tsx'),
    pathResolveAbs(__dirname, 'src/extension-test.tsx'),
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
  // Expose the brotli-compressed wasm size to the loading splash so it
  // reflects the bytes the browser actually downloads.
  define: {
    __TORILLE_WASM_BR_BYTES__: String(wasmBrBytes),
  },
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
  console.log(`\nwrote dist/main.js, dist/extension-test.js, dist/spell-worker.js, dist/torille_wasm_bg.wasm`);
  if (result.metafile) {
    for (const [file, info] of Object.entries(result.metafile.outputs)) {
      console.log(`  ${file}  ${info.bytes} bytes`);
    }
  }
}

