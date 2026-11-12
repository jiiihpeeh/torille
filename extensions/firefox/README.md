# torille — Firefox extension

The extension's manifest is **generated into `dist/`**. The repository root
contains `manifest.template.json` only; it references build output that does
not exist until `npm run build` has run, so loading it directly is not a valid
add-on and Firefox reports something like:

```text
Unable to load script: moz-extension://…/content/content.js
```

Use one of these instead:

## Settings

Open the extension options page to tune the checker. Changes save
automatically to `storage.local` and apply live to open tabs:

- underline style (wavy/solid/dashed/dotted/none) and colour;
- maximum number of suggestions;
- open suggestions on click or hover;
- popup background, text, and highlighted-suggestion colours.

The same keys are read by the content script at startup and on
`storage.onChanged`, so an options tab and a page being checked stay in sync.

## Grammar check

Right-click a text field and choose **Tarkista kielioppi**. The check is
on-demand and does not run on every keystroke. A floating panel lists the
diagnostics in the checker's rank order; clicking a correction replaces the
field text. Corrections are applied only to `<textarea>` and `<input>` fields;
contenteditable shows the diagnostics read-only.

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

After a rebuild, click **Reload** on the add-on in `about:debugging` and
refresh the page you are testing. Open the settings from the add-on's
**Preferences** / **Options** button. The extension also appears in the
toolbar; pin it from the puzzle-piece menu if needed.

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
