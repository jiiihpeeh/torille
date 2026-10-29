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
// The header carries two toggles that cover every panel at once:
// the built-in checker's ON/OFF preference, and an "use the browser
// extension" switch. Turning the extension on vetoes the built-in
// engine, so the page hands the textareas to the installed add-on's
// content script instead of double-underlining them. Both signals
// live in `spell-check-toggle.ts` so the buttons in this shell and the
// consumers in the panels stay in sync without prop drilling.

import { createSignal, onCleanup, onMount, Show } from 'solid-js';
import { GutenbergReader } from './GutenbergReader';
import {
  builtInSpellCheckEnabled,
  setSpellCheckEnabled,
  setUseExtension,
  spellCheckEnabled,
  useExtension,
} from './spell-check-toggle';

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
  // The extension's content script inserts one
  // `.torille-overlay-host` per attached textarea. Watching for it
  // lets the note distinguish "extension is handling this page" from
  // "pick an extension build and load it" without relying on a
  // private handshake. The observer is cheap (one selector per DOM
  // mutation) and stops noticing as soon as a host exists.
  const [extensionDetected, setExtensionDetected] = createSignal(false);
  onMount(() => {
    const check = () => {
      if (document.querySelector('.torille-overlay-host')) {
        setExtensionDetected(true);
        return true;
      }
      return false;
    };
    if (check()) return;
    const observer = new MutationObserver(() => {
      if (check()) observer.disconnect();
    });
    observer.observe(document.documentElement || document.body, {
      childList: true,
      subtree: true,
    });
    onCleanup(() => observer.disconnect());
  });

  return (
    <>
      <h1>
        torille-wasm demo
        <button
          type="button"
          class="spell-check-toggle"
          disabled={useExtension()}
          aria-pressed={builtInSpellCheckEnabled() ? 'true' : 'false'}
          title={
            useExtension()
              ? 'Built-in checker paused while the browser extension is in use'
              : builtInSpellCheckEnabled()
                ? 'Turn the built-in checker OFF for every panel'
                : 'Turn the built-in checker ON for every panel'
          }
          onClick={() => setSpellCheckEnabled(!spellCheckEnabled())}
        >
          <Show
            when={builtInSpellCheckEnabled()}
            fallback={<>Built-in: <strong>OFF</strong></>}
          >
            Built-in: <strong>ON</strong>
          </Show>
        </button>
        <button
          type="button"
          class="extension-toggle"
          aria-pressed={useExtension() ? 'true' : 'false'}
          title={
            useExtension()
              ? 'Give the page back to the built-in checker'
              : 'Pause the built-in checker and let the torille browser extension check this page'
          }
          onClick={() => setUseExtension(!useExtension())}
        >
          <Show
            when={useExtension()}
            fallback={<>Extension: <strong>OFF</strong></>}
          >
            Extension: <strong>ON</strong>
          </Show>
        </button>
      </h1>
      <Show when={useExtension()}>
        <p
          class={
            extensionDetected() ? 'extension-note' : 'extension-note missing'
          }
          aria-live="polite"
        >
          {extensionDetected()
            ? 'Built-in checker paused. Underlines and suggestions on this page come from the torille browser extension.'
            : 'Built-in checker paused. No extension overlay detected yet — load extensions/chromium/dist or extensions/firefox/torille-firefox.xpi to check this page.'}
        </p>
      </Show>
      <Show when={extensionDetected() && !useExtension()}>
        <p class="extension-note missing" aria-live="polite">
          The torille browser extension is active on this page too. Turn "Extension" on to let it own the
          textareas, or remove the extension — otherwise both checkers underline the same text.
        </p>
      </Show>
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