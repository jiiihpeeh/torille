// App shell: a free-typing spell-check panel plus the Gutenberg reader.
//
// The first panel is a small "free typing" editor pre-filled with a
// mix of inflected Finnish forms (istuu, talossa, koira, ...) and a
// few obvious misspellings; this is the playground where the user can
// type / click / pick suggestions.
//
// The book reader below it loads the whole local Gutenberg corpus, so
// the old fixed Minna Canth slice panel is gone: `Sylvi` is available
// as a normal book there.
//
// Each panel owns its own per-textbox wasm state. All panels share the
// primary spell worker, the suggestion pool, and the side cache --
// so a misspelling that finishes in one panel is instant in another.
//
// The header carries a single Spell-check toggle that turns the
// red underlines + suggestion popups ON / OFF for every panel at
// once. When OFF, every panel short-circuits its spell-check
// resource (no wasm calls, no per-word dispatch, no popup). The
// shared signal lives in `spell-check-toggle.ts` so the button
// in this shell and the consumers in the panels stay in sync
// without prop drilling.

import { Show } from 'solid-js';
import { GutenbergReader } from './GutenbergReader';
import { spellCheckEnabled, setSpellCheckEnabled } from './spell-check-toggle';

const DEFAULT_TEXT =
  'talo taloo koirra puhtaan öihin auto xyzzy\n\n' +
  'Kissa istuu talossa. Koira juoksee puistossa. Lintu laulaa puussa.';

// `?text=...` lets the Firefox extension pre-fill the editor
// with the page's selected text (the background service worker
// appends it when opening the demo popup window). The query
// string is read once at mount; subsequent edits in the
// textarea don't reset it. Only the FIRST panel honours the
// URL param; the book reader owns its own text.
function initialTextFromUrl(): string {
  try {
    const params = new URLSearchParams(globalThis.location?.search ?? '');
    const t = params.get('text');
    return t !== null ? t : DEFAULT_TEXT;
  } catch {
    return DEFAULT_TEXT;
  }
}

export function App() {
  return (
    <>
      <h1>
        torille-wasm demo
        <button
          type="button"
          class="spell-check-toggle"
          aria-pressed={spellCheckEnabled() ? 'true' : 'false'}
          title={
            spellCheckEnabled()
              ? 'Turn spell-check OFF for every panel'
              : 'Turn spell-check ON for every panel'
          }
          onClick={() => setSpellCheckEnabled(!spellCheckEnabled())}
        >
          <Show
            when={spellCheckEnabled()}
            fallback={<>Spell-check: <strong>OFF</strong></>}
          >
            Spell-check: <strong>ON</strong>
          </Show>
        </button>
      </h1>
      <p>
        Click a red word to see suggestions; arrow keys + Enter to pick.
        All panels share the same wasm worker + suggestion pool.
      </p>

      <TextPanel
        heading="Free text"
        subheading="Type freely -- misspellings are flagged as you type."
        initialText={initialTextFromUrl()}
      />

      <GutenbergReader />

      <details>
        <summary>How it works</summary>
        <pre>{`// Two-pass render: misspell ranges come back instantly; per-word
// suggestions post later and stream into the popup as they finish.
import {
  ready, createState, updateState, analyzeMisspellsAsync,
} from './spell-client.js';
await ready();
const idx = await createState();
updateState(idx, text);
const hits = await analyzeMisspellsAsync(idx, text, 8, 3);`}</pre>
      </details>
    </>
  );
}

// Late import so the editor components see the full module graph in
// the right order. (Solid's render() inside `main.tsx` calls App
// after the wasm worker is ready, so this is safe.)
import { TextPanel } from './TextPanel';