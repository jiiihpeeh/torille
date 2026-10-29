# Browser extension build, install, and test

Both browser extensions (`extensions/chromium/`, `extensions/firefox/`) wrap the
same Rust spell-checker that the CLI uses, exposed as a `wasm32-unknown-
unknown` module via `wasm-pack`. The two extensions share content script,
worker, options page, and icons; only the manifest and the background
hosting differ. Firefox uses `background.scripts` (a single-threaded
background page) and a `gecko` block; Chromium uses
`background.service_worker` with `type: "module"`.

The build order matters: the extensions copy the wasm-pack output from
`torille-wasm/pkg/`, so the wasm crate must be built first.

## Layout

| Path | Purpose |
|---|---|
| `torille-wasm/` | The Rust crate that becomes the wasm artefact. `lib.rs` re-exports the public API. |
| `torille-wasm/pkg/` | `wasm-pack build` output. `torille_wasm.mjs` (JS shim) and `torille_wasm_bg.wasm` (the binary) are copied into each extension's `dist/spell-worker/`. |
| `extensions/chromium/` | MV3 extension for Chrome, Brave, Edge, Opera, Vivaldi, Arc. |
| `extensions/firefox/` | MV3 extension for Firefox (Gecko, `strict_min_version: 142`). |
| `extensions/chromium/dist/` | Build output. Loadable as "Load unpacked". |
| `extensions/firefox/dist/` | Build output. Zipped into the `.xpi`. |
| `extensions/chromium/torille-chromium.zip` | Share-as-zip form of the Chromium extension. |
| `extensions/firefox/torille-firefox.xpi` | Installable Firefox add-on. |

The two extension directories each own their own `node_modules/` (TypeScript
compiler + `puppeteer-core` for the chromium E2E) and a `build.mjs` that
runs the same shape (clean `dist/`, compile TS, copy HTML/icons/manifest,
copy wasm artefacts, zip).

## Prerequisites

- **Rust toolchain** with the `wasm32-unknown-unknown` target:
  ```bash
  rustup target add wasm32-unknown-unknown
  ```
- **wasm-pack** 0.14 or newer:
  ```bash
  cargo install wasm-pack
  ```
- **Node.js** 18+ and the system `zip` (used by both `build.mjs` to
  package the share-as-zip / `.xpi` form).
- **The repo's `node_modules`** are not pre-installed; each extension
  carries its own. `npm install` is not needed for the build itself,
  but the e2e test runner in `extensions/chromium/e2e/` reads
  `puppeteer-core` from there.

## Build

Run the three commands in order, from anywhere. The build scripts use
absolute paths, so the working directory does not matter.

```bash
# 1. wasm: compile + wasm-bindgen + wasm-opt.
cd torille-wasm
wasm-pack build --release
# Produces torille-wasm/pkg/torille_wasm.mjs and
# torille-wasm/pkg/torille_wasm_bg.wasm (about 10 MB; gzip ~3 MB).

# 2. Chromium: tsc, copy artefacts, zip.
node /home/j-p/torille/extensions/chromium/build.mjs
# Produces extensions/chromium/dist/ and
# extensions/chromium/torille-chromium.zip.

# 3. Firefox: tsc, copy artefacts, zip into .xpi.
node /home/j-p/torille/extensions/firefox/build.mjs
# Produces extensions/firefox/torille-firefox.xpi.
```

`wasm-pack build --release` takes a couple of minutes the first time
(heavy lto and a single codegen unit, see `torille-wasm/Cargo.toml`).
Subsequent builds are faster. The two extension build scripts finish in
seconds.

## Install

### Chromium family (Chrome, Brave, Edge, Opera, Vivaldi, Arc)

The `.xpi` analogue for Chrome is a directory. Either load it unpacked
or install the zip.

1. Open `chrome://extensions`.
2. Toggle **Developer mode** on (top right).
3. Click **Load unpacked** and pick `/home/j-p/torille/extensions/chromium/dist/`.
4. The extension appears in the toolbar; pin it from the puzzle-piece menu.

To share a built artefact, hand the `extensions/chromium/torille-chromium.zip` file (about
2.4 MB) to another developer. They extract it and load the resulting
directory the same way.

### Firefox

1. Open `about:debugging#/runtime/this-firefox`.
2. Click **Load Temporary Add-on…**.
3. Pick `/home/j-p/torille/extensions/firefox/torille-firefox.xpi`.
4. The extension loads for the current session; it is dropped on restart
   unless signed for permanent install.

For signed permanent install, push the `.xpi` to
`https://addons.mozilla.org` via the AMO developer flow; the manifest
is already shaped for that (`gecko.strict_min_version: 142.0`,
`data_collection_permissions: ["none"]`).

## Verify

After loading, a quick check that the wasm + content script are wired:

- Open any page with a Finnish text input (e.g.
  https://fi.wikipedia.org/wiki/Suomi) and focus a textarea.
- Type `taloo` (a deliberate typo for `talo`). The overlay should mark
  the token; the context menu's "Tarkista oikeinkirjoitus" entry should
  surface corrections, with `talo` first.
- Open the extension's options page from the toolbar; the toggle
  "Ota torille käyttöön tällä sivustolla" should reflect the current
  host.

If the overlay does not appear, the worker is the first place to look
— the wasm binary is the largest dependency, and a failed fetch is the
common cause. The console (background page, MV3) shows the error.

## Test

### Firefox smoke test

`extensions/firefox/smoke.mjs` exercises the wasm in isolation:
1. Loads the wasm directly via `init(wasmBytes)`.
2. Re-implements the content-script's tokeniser regex and checks the
   wasm's `check` and `suggest_words` agree with the Rust unit tests.
3. Round-trips a `check` message through the worker module.

```bash
node /home/j-p/torille/extensions/firefox/smoke.mjs
```

### Firefox end-to-end (puppeteer + polyfill)

`extensions/firefox/e2e/run.mjs` drives a stub HTML page (`e2e/test-page.html`)
that polyfills `browser.runtime.getURL` to point at the real extension
files served by a local `http.server`. The content script runs in that
page exactly as it would inside a Firefox tab.

```bash
# In one shell, serve the extension:
cd /home/j-p/torille/extensions/firefox && python3 -m http.server 8766

# In another, drive the page:
node /home/j-p/torille/extensions/firefox/e2e/run.mjs
```

### Chromium end-to-end

`extensions/chromium/e2e/run.mjs` follows the same shape with
`puppeteer-core`. Note: MV3 service-worker extensions are not
reliably loadable under headless Chromium (Google's Chrome disables
`--load-extension` and `headless=new` partially ignores it). The
README in `extensions/chromium/` calls this out; the practical answer
is to load `dist/` in a regular Chromium session and verify manually.

## Manifest validation

Both `build.mjs` scripts validate the manifest before producing `dist/`:

- `manifest_version === 3`
- The right background shape for the target browser
  (Chromium requires `background.service_worker`; Firefox
  does not have one)
- No cross-pollination (`browser_specific_settings` in the
  Chromium manifest is an error; missing `gecko` in the Firefox
  manifest is an error)
- All required resources are present before the zip step

A manifest violation aborts the build with a clear message rather than
producing a broken artefact.

## Troubleshooting

**Wasm is huge.** Yes — about 10 MB uncompressed, ~3 MB gzipped over
the wire. `torille-wasm/Cargo.toml` already uses
`opt-level = "z"`, `lto = "fat"`, `codegen-units = 1`, and
`panic = "abort"`. There is no fat to trim further without dropping
the lemma lexicon size; the binary is dominated by the embedded Kotus
sanalista. Streaming `gzipped` over `moz-extension://` /
`chrome-extension://` keeps cold-start under a second on a modern
browser.

**`wasm-pack` warns about a newer version.** Cosmetic; 0.14.0 works.
The build pipeline does not depend on 0.15 features.

**TypeScript build is silent.** `tsc -p tsconfig.json` only prints
errors. If a stale `.js` from a previous build is in `dist/`, the
script's `rm -rf dist/` step at the top handles it. If a source file
fails to compile, `tsc` exits non-zero and the build aborts before
copying artefacts.

**`zip` not found.** The `build.mjs` scripts shell out to the system
`zip` to produce the share-as-zip / `.xpi` form. On Debian/Ubuntu:
`sudo apt install zip`. The unpacked `dist/` is still produced even if
the zip step fails (the chromium script logs a warning and continues;
the firefox script does the same on a best-effort basis).

**`wasm-opt` not found.** `wasm-pack` looks for it on PATH; the
current toolchain has it at `/home/j-p/.local/bin/wasm-opt`. If
missing, install via `cargo install wasm-opt` or your package manager.

**MV3 service-worker debug.** On Chromium, the service worker has its
own dev-tools entry under `chrome://serviceworker-internals` and
`chrome://extensions` → "Inspect views: service worker". Use that to
catch the `fetch('/spell-worker/torille_wasm_bg.wasm')` failures; the
background page console is the first place any "spell-worker 404" or
"wasm magic" error lands.

**Firefox temporary add-on is gone after restart.** Expected. To
persist, push through AMO signing or use the `web-ext run` workflow
during development:

```bash
cd /home/j-p/torille/extensions/firefox
npx web-ext run --source-dir dist/  # if web-ext is available
```
