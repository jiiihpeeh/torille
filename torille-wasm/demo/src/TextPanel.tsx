// TextPanel: a self-contained spell-checked text field.
//
// Owns its own text signal, debounced text, wasm per-textbox state,
// first-pass misspell candidates, and the slow `bg.check` demote set.
// Render the same first-pass results with the surgical-diff reuse from
// the previous render -- unchanged (word, byte range) entries skip the
// per-word pool round-trip, which is what makes typing a single
// character at the end of a long document cost ~0 pool calls.
//
// Two TextPanels share the same wasm worker + suggestion pool (both
// per-editor states are independent u32 indices into the wasm's
// per-textbox state table). The suggestion side cache is keyed by
// word only, so a misspell that finishes in panel A is instant in
// panel B -- a small but pleasant consequence of the shared worker
// pool.

import {
  createMemo,
  createResource,
  createSignal,
  onCleanup,
  onMount,
  Show,
} from 'solid-js';
import { countWords, tokenize } from './tokenize';
import {
  findMisspellings,
  MisspellCallbacks,
  MisspellWithRange,
} from './find-misspellings';
import { Misspell } from './find-misspellings';
import {
  checkStyle,
  checkSyntax,
  createState,
  destroyState,
  ready as wasmReady,
} from './spell-client';
import {
  builtInGrammarCheckEnabled,
  builtInSpellCheckEnabled,
  builtInStyleCheckEnabled,
  useExtension,
} from './spell-check-toggle';
import type { DiagnosticSpan } from '../../protocol.js';
import { Editor } from './Editor';
import type { FocusTarget } from './Editor';

export interface TextPanelProps {
  /** Heading shown above the editor. */
  heading: string;
  /** Pre-fill text; subsequent edits do not reset to it. */
  initialText: string;
  /** Optional sub-line under the heading (e.g. a brief description). */
  subheading?: string;
}

export function TextPanel(props: TextPanelProps) {
  // Two text signals: `text` follows every keystroke (so the
  // textarea stays responsive and the underlay mirrors the
  // typed characters); `debouncedText` updates a beat after
  // the user stops typing (so the spell-check does not run
  // on every keypress).
  const [text, setText] = createSignal(props.initialText);
  const [debouncedText, setDebouncedText] = createSignal(
    props.initialText,
  );
  const [focused, setFocused] = createSignal<FocusTarget | null>(null);

  // The per-textbox state lives in the wasm worker. We start
  // at -1 (no state yet) and assign it on mount once the wasm
  // is ready. The resource depends on it so render-after-init
  // works.
  const [stateIdx, setStateIdx] = createSignal<number>(-1);

  // First-pass misspell candidates: every word the wasm's
  // `analyze_misspells` flagged as not-in-lemma-set (and not
  // a learned proper noun / phrase). The user sees these
  // *immediately* as red underlines -- the slow `bg.check`
  // runs in the background and may demote a few (verb
  // inflections, gradated forms) via the `onDemoteMisspell`
  // callback. The visible misspellings = candidates - demoted.
  const [candidates, setCandidates] = createSignal<MisspellWithRange[]>(
    [],
  );
  // Set of `word` strings the slow check has confirmed as
  // valid Finnish forms. Keyed by word (not (word, start,
  // end)) because the slow check is per-word: a demote for
  // "tuntee" applies to every occurrence of "tunnee" in
  // the current render, including positions whose byte
  // range differs from the demote's original range.
  const [demoted, setDemoted] = createSignal<Set<string>>(new Set());
  // Monotonic generation counter. Each render increments
  // it; a stale callback (one whose owning render was
  // already superseded by a newer debounced edit) is
  // dropped -- this is what keeps a slow `bg.check` from
  // render 1 polluting render 2's candidate list with stale
  // byte ranges.
  const [renderGen, setRenderGen] = createSignal(0);

  // Snapshot of the *previous* render's candidate list, used
  // as a surgical-diff hint to `findMisspellings`. The ref is
  // updated in the `onMisspellCandidate` callback (after the
  // renderGen staleness check) so it always tracks the
  // visible candidate list.
  let lastCandidates: MisspellWithRange[] = [];

  let checkTimer: ReturnType<typeof setTimeout> | undefined;

  const scheduleCheck = (next: string): void => {
    if (checkTimer !== undefined) clearTimeout(checkTimer);
    checkTimer = setTimeout(() => {
      setDebouncedText(next);
      checkTimer = undefined;
    }, 150);
  };

  const onText = (next: string): void => {
    setText(next);
    scheduleCheck(next);
  };

  // Create the wasm-side state once on mount and drop it on
  // unmount. `findMisspellings` internally dispatches
  // `updateState` and lets the suggestion pool handle the
  // per-word passes.
  onMount(async () => {
    await wasmReady();
    const idx = await createState();
    setStateIdx(idx);
  });

  onCleanup(() => {
    if (checkTimer !== undefined) clearTimeout(checkTimer);
    const idx = stateIdx();
    if (idx >= 0) destroyState(idx);
  });

  // Misspellings resource — fires on debounced text /
  // state-idx change. The async fetcher runs the fast
  // first-pass (`analyze-misspells`) and fires
  // `onMisspellCandidate` for every flag in single-digit
  // milliseconds; the slow per-word `bg.check` runs in
  // parallel and demotes verb inflections / gradated forms
  // via `onDemoteMisspell` once it finishes.
  //
  // The fetcher awaits the `drained` half of
  // `findMisspellings` before resolving, so the resource
  // signal only fires once the slow-check pool has fully
  // settled (or the round was cancelled). Subscribers to
  // `entry.onRenderDone` (the puppeteer probe's
  // `__torille.panels[i].onRenderDone`) get a callback at
  // the same moment, replacing the 1.5 s fixed-time wait
  // the probe used to guess with.
  const renderDoneSubscribers = new Set<() => void>();
  const [spellResource] = createResource(
    // The toggle is part of the tracker's input so flipping
    // it invalidates the resource and triggers a fresh
    // fetcher call -- where the OFF branch returns
    // immediately without any wasm calls. Reading the
    // signal here also subscribes the resource to it,
    // so toggling never goes unnoticed.
    () => ({
      text: debouncedText(),
      idx: stateIdx(),
      on: builtInSpellCheckEnabled(),
    }),
    async ({ text, idx, on }): Promise<Misspell[]> => {
      // Off: wipe the visible underlay and any open popup,
      // bump the render generation so any in-flight round
      // gets cancelled, fire the render-done subscribers
      // (probes `await` these), and return without firing
      // a single wasm call. No `analyze_misspells`, no
      // per-word dispatch, no suggestion pool work.
      if (!on) {
        const myGen = renderGen() + 1;
        setRenderGen(myGen);
        setCandidates([]);
        setDemoted(new Set());
        setFocused(null);
        // The fetch returned, so subscribers can fire.
        // The `renderGen() === myGen` guard from the
        // on-path is unnecessary here -- the OFF path is
        // synchronous except for the microtask boundary.
        for (const sub of renderDoneSubscribers) sub();
        return [];
      }
      if (idx < 0) return [];
      const myGen = renderGen() + 1;
      setRenderGen(myGen);
      // Snapshot the previous render's candidate list BEFORE
      // we wipe `candidates` -- the surgical diff in
      // `findMisspellings` reads this to skip the slow pool
      // for words whose byte range and surface form are
      // unchanged from the previous render.
      const prevForDiff = lastCandidates;
      setCandidates([]);
      setDemoted(new Set());
      const cb: MisspellCallbacks = {
        onMisspellCandidate: (m: MisspellWithRange) => {
          const cur = renderGen();
          if (cur !== myGen) {
            console.warn('[demo] stale candidate', { m, myGen, cur });
            return;
          }
          setCandidates((cur2) => {
            if (cur2.some(
              (e) => e.start === m.start && e.end === m.end,
            )) return cur2;
            const next = [...cur2, m];
            lastCandidates = next;
            return next;
          });
        },
        onDemoteMisspell: (m: MisspellWithRange) => {
          if (renderGen() !== myGen) return;
          if (!candidates().some((c) => c.word === m.word)) {
            console.warn('[demo] demote for word no longer candidate', m);
            return;
          }
          console.log('[demo] demote misspell', m);
          setDemoted((cur2) => {
            if (cur2.has(m.word)) return cur2;
            const next = new Set(cur2);
            next.add(m.word);
            return next;
          });
        },
      };
      const result = await findMisspellings(text, idx, cb, {
        previouslyConfirmed: prevForDiff,
        isCancelled: () => renderGen() !== myGen,
      });
      // Wait for the slow-check pool to drain. The pool is
      // fire-and-forget in spirit, but the returned
      // `drained` promise resolves when every dispatched
      // call has either completed or been cancelled.
      await result.drained;
      // Only fire the render-done signal if this round is
      // still the active one -- a newer round that started
      // during the drain cancels this round and gets its
      // own signal.
      if (renderGen() === myGen) {
        for (const sub of renderDoneSubscribers) sub();
      }
      return result.hits;
    },
  );
  const misspellings = createMemo<Misspell[]>(() => {
    const dem = demoted();
    return candidates().filter((m) => !dem.has(m.word));
  });

  // Grammar and style are opt-in and run on the same debounced text as the
  // spell check. They are separate wasm calls: `checkSyntax` uses the
  // per-textbox syntax cache, `checkStyle` is stateless. Both are skipped
  // when the browser extension owns the page.
  const [grammarDiagnostics] = createResource(
    () => ({
      text: debouncedText(),
      idx: stateIdx(),
      on: builtInGrammarCheckEnabled(),
    }),
    async ({ text, idx, on }): Promise<DiagnosticSpan[]> => {
      if (!on || idx < 0) return [];
      return checkSyntax(idx, text.replace(/\r\n?/g, '\n'));
    },
  );
  const [styleDiagnostics] = createResource(
    () => ({ text: debouncedText(), on: builtInStyleCheckEnabled() }),
    async ({ text, on }): Promise<DiagnosticSpan[]> => {
      if (!on) return [];
      return checkStyle(text.replace(/\r\n?/g, '\n'));
    },
  );

  // Debug hook: each panel exposes its live state to `window` so
  // the puppeteer probes can see why a word is or isn't
  // underlined. Two panels -> `__torille.panels[i]`; the legacy
  // `__torille` proxy still points at the LAST-MOUNTED panel for
  // backward compat with probe-sylvi, probe-debug, etc. (The
  // single-panel probes don't need to know about panels.)
  if (typeof window !== 'undefined') {
    const w = window as any;
    if (!w.__torille) {
      w.__torille = {};
      w.__torille.panels = [];
    }
    // Lazily populated memory-info getter so the probe
    // can read the wasm worker's linear memory size
    // without a hard import. The first call is what
    // triggers the round-trip to the worker.
    if (typeof w.__torille.getMemoryInfo !== 'function') {
      import('./spell-client').then(({ getWorkerMemoryInfo }) => {
        w.__torille.getMemoryInfo = getWorkerMemoryInfo;
      });
    }
    // Lazily populated cache-sizes getter so the probe
    // can see exactly which bounded map is at capacity
    // (or growing) at every step. Local to the main
    // thread -- workers' caches aren't visible from
    // here; we depend on `getMemoryInfo` + the wasm
    // memory delta for the worker-side picture.
    if (typeof w.__torille.getCacheSizes !== 'function') {
      import('./spell-client').then(({ getCacheSizes }) => {
        w.__torille.getCacheSizes = getCacheSizes;
      });
    }
    const entry = {
      candidates,
      demoted,
      misspellings,
      heading: () => props.heading,
      // Programmatic text setter. Goes through the same
      // `onText` path the Editor's input event uses, so
      // Solid state + wasm state stay in sync. Replaces
      // the prototype-setter hack the probe used to do.
      // The Editor's `createEffect` watches `props.text`
      // and updates the underlying textarea's `.value`,
      // so we don't need to touch the DOM here.
      setText: (next: string): void => onText(next),
      // Programmatic focus setter. Replaces the brittle
      // `mouse.click` at span-coordinates approach: the
      // Editor's `createEffect` watches `props.focused`
      // and opens the popup the same way a user click
      // would. The probe uses this for the "click and
      // read the popup" check. A bare word string resolves
      // to its first occurrence; pass `{ word, start }` to
      // name a specific occurrence.
      setFocused: (w: string | FocusTarget | null): void => {
        if (typeof w === 'string') {
          const tok = tokenize(text()).find(
            (t) => t.kind === 'word' && t.text === w,
          );
          setFocused(
            tok && tok.kind === 'word'
              ? { word: w, start: tok.start }
              : { word: w, start: -1 },
          );
        } else {
          setFocused(w);
        }
      },
      // Hard reset of the panel's per-textbox state.
      // Drops the wasm state's text + proper-noun set
      // (so a fresh chapter starts with a clean slate),
      // wipes the underlay's misspell list, and clears
      // the popup. Used by the probe's stress path to
      // simulate a real page navigation between chapters
      // -- the kind of teardown the browser does on
      // navigation but we don't do today.
      clearState: (): void => {
        const idx = stateIdx();
        if (idx >= 0) {
          destroyState(idx);
          const newIdx = createState();
          setStateIdx(newIdx);
        }
        setText('');
        setDebouncedText('');
        setCandidates([]);
        setDemoted(new Set());
        setFocused(null);
      },
      // Subscribe to render completion. The callback fires
      // when the resource's spell-check round has fully
      // settled (first-pass in, slow-check pool drained)
      // AND no newer round has started. Returns an
      // unsubscribe fn.
      onRenderDone: (cb: () => void): (() => void) => {
        renderDoneSubscribers.add(cb);
        return () => {
          renderDoneSubscribers.delete(cb);
        };
      },
    };
    w.__torille.panels.push(entry);
    const idx = w.__torille.panels.length - 1;
    // Legacy single-panel surface: always points at the latest
    // panel so existing probes keep working unchanged.
    w.__torille.candidates = candidates;
    w.__torille.demoted = demoted;
    w.__torille.misspellings = misspellings;
    w.__torille.lastPanelIdx = idx;
    onCleanup(() => {
      const arr = w.__torille?.panels;
      if (arr) {
        const i = arr.indexOf(entry);
        if (i >= 0) arr.splice(i, 1);
      }
    });
  }

  return (
    <>
      <h2>
        {props.heading}{' '}
        <small>
          ({misspellings().length} misspelling
          {misspellings().length === 1 ? '' : 's'})
        </small>
      </h2>
      <Show when={props.subheading}>
        <p class="subheading">{props.subheading}</p>
      </Show>
      <div class="status" aria-live="polite">
        <span>{countWords(text())} words</span>
        <Show
          when={builtInSpellCheckEnabled()}
          fallback={
            <span class="muted">
              {useExtension() ? 'built-in off — using extension' : 'spell-check off'}
            </span>
          }
        >
          <Show
            when={spellResource.loading && misspellings().length === 0}
            fallback={
              <Show
                when={misspellings().length === 0}
                fallback={
                  <span class="bad">
                    {misspellings().length} misspellings
                  </span>
                }
              >
                <span class="ok">all spelled</span>
              </Show>
            }
          >
            <span class="checking">
              <span class="status-dot" aria-hidden="true" />
              checking…
            </span>
          </Show>
        </Show>
        <Show when={builtInGrammarCheckEnabled()}>
          <span>{grammarDiagnostics()?.length ?? 0} grammar</span>
        </Show>
        <Show when={builtInStyleCheckEnabled()}>
          <span>{styleDiagnostics()?.length ?? 0} style</span>
        </Show>
      </div>
      <Editor
        text={text()}
        onText={onText}
        misspellings={misspellings()}
        grammarDiagnostics={grammarDiagnostics() ?? []}
        styleDiagnostics={styleDiagnostics() ?? []}
        focused={focused()}
        stateIdx={stateIdx()}
        onFocus={(w) => setFocused(w)}
        onClose={() => setFocused(null)}
      />
    </>
  );
}