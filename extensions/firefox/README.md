# torille — Firefox extension

The extension's manifest is **generated into `dist/`**. The repository root
contains `manifest.template.json` only; it references build output that does
not exist until `npm run build` has run, so loading it directly is not a valid
add-on and Firefox reports something like:

```text
Unable to load script: moz-extension://…/content/content.js
```

Use one of these instead:

## Temporary add-on (development)

```bash
cd torille-wasm && wasm-pack build --release
cd ../extensions/firefox && npm run build
```

Then open `about:debugging#/runtime/this-firefox`, click **Load Temporary
Add-on…**, and select:

```text
extensions/firefox/dist/manifest.json
```

Or launch it directly:

```bash
cd extensions/firefox
npx web-ext run --source-dir dist
```

After a rebuild, reload the add-on from `about:debugging` and refresh the page.

## Packaged build

`npm run build` also writes `torille-firefox.xpi` next to this file. Release
Firefox refuses unsigned add-ons, so use a temporary add-on for local testing
or sign the XPI for distribution.

## Test page

The Solid manual test page lives in the demo:

```text
http://127.0.0.1:8765/extension-test.html
```

Start the demo with `torille-wasm/demo/run.sh` and load the extension before
opening the page.

## Tests

```bash
npm test
npm run build
```
