// Global spell-check source toggle.
//
// All panels (free-text, Canth, Gutenberg) read these signals. The
// built-in checker is the wasm engine bundled with the demo; the
// extension is the separately-installed browser add-on, whose content
// script overlays every textarea on the page. Running both at once
// double-underlines the same text, so `useExtension` is a hard veto on
// the built-in engine.
//
// `spellCheckEnabled` is the user's preference for the built-in
// checker (the original ON/OFF button). `useExtension` temporarily
// overrides it: `builtInSpellCheckEnabled()` is the effective state
// every panel should consult. When the extension is selected each
// panel short-circuits its spell-check resource: no wasm
// `analyze_misspells`, no per-word dispatch, no built-in underlines or
// popup. The extension's own overlay takes over the textarea.
//
// The signals live in their own module so the buttons in App.tsx and
// the consumers in TextPanel / GutenbergReader share them without
// prop-drilling through every panel.
//
// Defaults: built-in ON, extension OFF. The demo's first paint should
// already be in the working state the user expects to see.

import { createSignal } from 'solid-js';

const [spellCheckEnabled, setSpellCheckEnabled] = createSignal<boolean>(true);
const [useExtension, setUseExtension] = createSignal<boolean>(false);
// The grammar and style layers are opt-in. They are separate from
// spelling so the default demo stays a spell-check playground.
const [grammarCheckEnabled, setGrammarCheckEnabled] = createSignal<boolean>(false);
const [styleCheckEnabled, setStyleCheckEnabled] = createSignal<boolean>(false);

// Effective built-in state. Read these instead of the preference
// signals anywhere the wasm pipeline is gated. The extension is a hard
// veto for all three layers, so the page never double-underlines.
const builtInSpellCheckEnabled = (): boolean =>
  spellCheckEnabled() && !useExtension();
const builtInGrammarCheckEnabled = (): boolean =>
  grammarCheckEnabled() && !useExtension();
const builtInStyleCheckEnabled = (): boolean =>
  styleCheckEnabled() && !useExtension();

export {
  spellCheckEnabled,
  setSpellCheckEnabled,
  grammarCheckEnabled,
  setGrammarCheckEnabled,
  styleCheckEnabled,
  setStyleCheckEnabled,
  useExtension,
  setUseExtension,
  builtInSpellCheckEnabled,
  builtInGrammarCheckEnabled,
  builtInStyleCheckEnabled,
};
