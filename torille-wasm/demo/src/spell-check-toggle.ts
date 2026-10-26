// Global spell-check on/off toggle.
//
// All three panels (free-text, Canth, Gutenberg) read this
// signal. When `false`, each panel short-circuits its spell-
// check resource: the wasm `analyze_misspells` call is
// skipped entirely, the suggestion pool is not consulted,
// no underlines are rendered. Toggling on resumes normal
// rendering on the next debounced tick.
//
// The toggle lives in its own module so the button in App.tsx
// and the consumers in TextPanel / GutenbergReader share
// the same reactive signal without prop-drilling through
// every panel.
//
// Default: ON. The demo's first paint should already be in
// the working state the user expects to see.

import { createSignal } from 'solid-js';

const [spellCheckEnabled, setSpellCheckEnabled] = createSignal<boolean>(true);

export { spellCheckEnabled, setSpellCheckEnabled };