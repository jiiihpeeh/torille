# Browser extensions

Two builds wrap the same wasm checker and share the content script,
worker, and options page:

| directory | browser | build output |
|---|---|---|
| `chromium/` | Chrome, Brave, Edge, Opera, Vivaldi, Arc | `chromium/dist/` and `chromium/torille-chromium.zip` |
| `firefox/` | Firefox 142+ | `firefox/dist/` and `firefox/torille-firefox.xpi` |

Both need the wasm package first:

```bash
cd torille-wasm
wasm-pack build --release
```

## Build

```bash
node extensions/chromium/build.mjs
node extensions/firefox/build.mjs
```

Or, from either extension directory:

```bash
npm run build
```

The build compiles TypeScript, bundles the Solid options page with esbuild,
copies the wasm-pack output, and writes the loadable `dist/` directory
(plus the zip/XPI).

## Load in Chromium

1. Open `chrome://extensions` (or `brave://extensions`,
   `edge://extensions`, …).
2. Turn on **Developer mode**.
3. Click **Load unpacked**.
4. Choose `extensions/chromium/dist/`.
5. Pin the extension from the toolbar puzzle-piece menu if you want the
   icon visible.

Reload it from `chrome://extensions` after a rebuild, then refresh the page
being checked. **Details → Extension options** opens the settings page.

## Load in Firefox

1. Open `about:debugging#/runtime/this-firefox`.
2. Click **Load Temporary Add-on…**.
3. Choose `extensions/firefox/dist/manifest.json`.
4. Pin the extension from the toolbar puzzle-piece menu if needed.

Or run it with web-ext:

```bash
cd extensions/firefox
npx web-ext run --source-dir dist
```

Temporary add-ons last for the session. After a rebuild, click **Reload** on
the add-on in `about:debugging` and refresh the page. **Preferences /
Options** opens the settings page.

The checker ignores fenced code, inline code, raw HTML code elements,
BBCode payloads, bare URLs and email addresses. Code is masked before it reaches the wasm, so it cannot
change sentence boundaries or the learned proper-noun set. The full
implementation notes and follow-ups are in
[`TODO_EXTENSION_CODE_BLOCKS.md`](../TODO_EXTENSION_CODE_BLOCKS.md).

## Settings

The options page controls the underline style and colour, the suggestion
limit, click-vs-hover popup behaviour, and the popup colours. It also has
the **Tarkastus** toggles for code blocks, inline code, BBCode, URLs and
whole code editors. Changes save automatically to `storage.local` and apply live
to open tabs.

## Test pages

- Manual extension test page: `http://127.0.0.1:8765/extension-test.html`
  (start the demo with `torille-wasm/demo/run.sh`).
- Chromium e2e: `node extensions/chromium/e2e/run.mjs` after starting the
  local HTTP server described in that script.
- Firefox e2e: `node extensions/firefox/e2e/run.mjs`.
- Code/BBCode e2e: serve `extensions/chromium/` on `:8767`, then run
  `node extensions/chromium/e2e/code-blocks.mjs`.
- Touch e2e: serve the extension directory, then run
  `node extensions/chromium/e2e/touch.mjs` (Firefox: set
  `TORILLE_PORT=8766` or run its own copy).
- Unit tests: `npm test` in either extension directory (includes the
  ignore-range parser and Chromium/Firefox source-sync checks).

See each extension's README for the browser-specific manifest differences
and limitations.
