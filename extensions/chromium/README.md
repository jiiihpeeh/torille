# torille-chromium

Manifest V3 WebExtension for Chromium-based browsers (Chrome, Brave, Edge,
Opera, Vivaldi, Arc), sibling of [torille-firefox](../firefox). Both
extensions share the same code (`content.js`, `spell-worker.js`, `options.{html,js}`,
`icons/`); only `manifest.json` and `background.js`'s hosting differ.

## What's different from Firefox

| | Firefox (`extensions/firefox/`) | Chromium (`extensions/chromium/`) |
|---|---|---|
| manifest_version | 3 | 3 |
| background.scripts | `background.js` | (not used) |
| background.service_worker | (not used) | `background.js` (`type: "module"`) |
| `browser_specific_settings.gecko` | required (strict_min_version 142) | not present |
| `data_collection_permissions` | `["none"]` | not present |
| `web_accessible_resources` | flat array | `{resources, matches}` array |
| `minimum_chrome_version` | n/a | n/a (Chrome doesn't have one) |

Everything else is identical: the wasm worker, the content-script overlay,
the per-host blacklist, the per-textbox toggle, the options page, the
debug logging.

## Install

The `.xpi` analogue for Chrome is a directory. Build it first:

```bash
node /home/j-p/torille/extensions/chromium/build.mjs
```

That writes `/home/j-p/torille/extensions/chromium/dist/` (the loadable extension)
and `/home/j-p/torille/extensions/chromium/torille-chromium.zip` (a share-as-zip form).

Then:

1. Open `chrome://extensions` in the browser.
3. Toggle **Developer mode** on (top right).
4. Click **Load unpacked** and pick `dist/`.

## Manifest validation

The `build.mjs` script checks for:

- `manifest_version === 3`
- No `browser_specific_settings` (would be set on the wrong browser)
- `background.service_worker` is set (Chrome MV3 requires it)
- All resources are present before producing `dist/`

## Limitations vs the Firefox build

1. **Headless test driving is unreliable.** Loading MV3 service-worker
   extensions under headless Chromium requires either `--load-extension`
   (which Google's Chrome build disables for licensing reasons and
   Chromium's `headless=new` mode partially ignores) or a CDP domain
   that newer Chromium builds do not expose uniformly. The
   `extensions/chromium/smoke.mjs` script attempts both; if neither works,
   install the extension in a regular Chromium session and verify
   manually.
2. **Web Extension policy.** Brave and Vivaldi honour MV3 with no
   additions; Edge adds One sync at end; Chrome's release channel is
   the slowest. All of these load `dist/` via the "Load unpacked"
   button on `chrome://extensions`.
3. **Signature.** Unpacked extensions run unprivileged. To publish
   through the Chrome Web Store, sign with the Chrome Web Store
   developer key; the manifest is otherwise unchanged.