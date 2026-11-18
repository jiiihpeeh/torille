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

- spelling underline style (wavy/solid/dashed/dotted/none) and colour;
- grammar error/warning/suggestion colours and grammar underline visibility;
- style underline colour and style underline visibility;
- maximum number of suggestions;
- open suggestions on click or hover;
- popup background, text, and highlighted-suggestion colours.

The same keys are read by the content script at startup and on
`storage.onChanged`, so an options tab and a page being checked stay in sync.

## Omat sanat (word wizard)

The options page links to **Omat sanat** (`options/wizard.html`). It finds a
Kotus class for a word that is not in the sanalista — a name, a loanword, a
dialect word — by asking for a few inflected forms. The engine's own
`inflect_in` decides what each candidate class would generate; the page only
compares strings and shows the evidence.

Saved words are stored as the `torille-user-words` text blob in
`storage.local`:

```text
# torille-user-words v1
mökki	5*A	substantiivi
```

The background pushes the blob to the wasm worker as `set-user-words`; the
worker inlines every generated form into its accepted set, so a saved word and
its inflections are no longer underlined and are offered as suggestions within
two edits. The list is exportable and importable as the same `.txt`.

The same text format is used by the demo page
(`torille-wasm/demo/wizard.html`), which stores it in `localStorage`.

The part-of-speech choices are `substantiivi`, `adjektiivi`, `numeraali`,
`järjestysluku`, `pronomini`, `erisnimi` and `verbi`. A proper name keeps its
initial capital in the preview and the saved list; it inflects as a noun.

## Grammar check

Right-click a text field and choose **Tarkista kielioppi**, **Tarkista tyyli**
or **Tarkista kielioppi ja tyyli**. The combined command runs both layers in
one morphology pass and shows a single ranked panel. The check is on-demand
and does not run on every keystroke. A floating panel lists the diagnostics;
clicking a correction replaces the field text. Textarea and input fields are
written directly; contenteditable uses a range-aware writer so the surrounding
markup stays intact.

Each finding also has an **älä huomauta** button. It hides that stable rule id
(not one span) and stores the choice in `torille-ignored-rules`; the options
page can clear the list. Ignored rules survive edits and reloads.

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
