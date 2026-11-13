// Find misspellings in a piece of text.
//
// Three-pass render:
//   1. First pass: instant misspelling ranges on the primary
//      worker via `analyze_misspells` (no paradigm generation,
//      single-digit ms). Used to learn the byte ranges and
//      candidate words.
//   2. Check pass: as soon as the first-pass ranges come back,
//      the main thread fans out a cheap `check-known` per word
//      across a pool of 3 sibling workers (each one of which
//      holds its own wasm instance).
//   2b. Background suggestions: after the check pass settles,
//      warm the popup cache for the first thirty confirmed
//      misspellings only. Clicking one of those is then instant;
//      the cap keeps a long page from starting hundreds of
//      ranking calls.
//   3. Underlines appear *after* the wasm's full inflection-
//      aware `bg.check` confirms the word is NOT a known
//      Finnish form. Until then the word is invisible to the
//      UI -- this is what keeps `talossa`, `juoksee`,
//      `puhtaan`, `öihin` and friends from being flagged as
//      misspellings on screen. The popup's side cache fills
//      in parallel from the same pool's `bg.suggest_for_misspell`.

import {
  analyzeMisspellsAsync,
  updateState,
  dispatchCheckKnown,
  dispatchSuggestForMisspell,
} from './spell-client';

/** Byte range attached to each confirmed misspell so the
 *  underlay renderer knows where to draw the red squiggle. */
export interface MisspellRange {
  start: number;
  end: number;
}

export interface MisspellWithRange {
  word: string;
  start: number;
  end: number;
  /** Known word, wrong capitalization: do not dispatch the slow check. */
  caseOnly?: boolean;
}

/** A confirmed misspelling: the surface word plus the byte range the
 *  underlay renderer needs. The pipeline always carries the range, so
 *  this is the same shape as {@link MisspellWithRange}. */
export type Misspell = MisspellWithRange;

export type { MisspellRange as MisspellByteRange };

export interface MisspellCallbacks {
  /** Fired for every first-pass candidate misspell (and for
   *  surgical-diff reuses from the previous render) as soon
   *  as `analyze_misspells` returns. The renderer should
   *  draw the underline immediately -- the slow `bg.check`
   *  will run in the background and may demote a few of
   *  these (verb inflections, gradated forms) via
   *  `onDemoteMisspell` once it finishes. Net effect:
   *  underlines appear in single-digit ms instead of after
   *  a 1-2 second slow-check wait. */
  onMisspellCandidate?: (m: MisspellWithRange) => void;
  /** Fired when the parallel pool's slow `bg.check(word)`
   *  returns `true` -- i.e. the word IS a known Finnish
   *  form despite the first-pass misspell-flag. The
   *  renderer should drop the underline for this entry.
   *  (Typically: verb present-tense forms like `istuu`,
   *  `juoksee`; past participles the suffix-stripper
   *  can't unwind.) */
  onDemoteMisspell?: (m: MisspellWithRange) => void;
}

export interface FindMisspellingsOptions {
  /** The misspell list from the PREVIOUS render. Words in the
   *  new first-pass result that match a previous entry by
   *  (word, start, end) are reused: we re-fire the
   *  `onMisspellCandidate` callback synchronously and skip
   *  the slow per-word pool round-trip. This is the typing
   *  hot path -- a single character added at the end of a
   *  500-word document lets every other word skip the
   *  50-1100 ms `bg.check` call. */
  previouslyConfirmed?: MisspellWithRange[];
  /** Optional cancel check, called at three points: after
   *  the first pass resolves, before each per-word slow
   *  dispatch, and after each per-word slow result
   *  resolves. Returning `true` aborts the round: pending
   *  per-word calls that haven't been dispatched yet are
   *  skipped, and in-flight ones' results are dropped
   *  (no callback fires).
   *
   *  Critical for the Gutenberg reader, where a user
   *  flipping through 7k-word chapters fires a new
   *  `findMisspellings` round on every flip. Without this
   *  the slow per-word pool piles up hundreds of pending
   *  wasm calls in the sibling workers -- each chapter
   *  flip's calls queue behind the previous round's, the
   *  round-trip latency climbs to 17+ s by the 8th flip,
   *  and the wasm workers' heap grows until the browser
   *  OOMs. The render-path TextPanels don't need it
   *  because the 150 ms debounce on `debouncedText` keeps
   *  the rate low; we keep the field optional so we
   *  don't pay the closure cost on the typing hot path. */
  isCancelled?: () => boolean;
}

export interface FindMisspellingsResult {
  /** The raw first-pass hits. Callers that want every flag
   *  regardless of slow-check demotes (test harnesses, the
   *  per-render counter) use this. */
  hits: Misspell[];
  /** Resolves when every per-word slow-check call dispatched
   *  by THIS round has either completed or been cancelled
   *  via the `isCancelled` hook. Callers that want to
   *  observe "round fully settled" -- typically the
   *  puppeteer probe via `__torille.panels[i].onRenderDone`
   *  -- await this. */
  drained: Promise<void>;
}

/** Words per first-pass window. Matches the Gutenberg reader's page
 *  budget, which is the size the worker's memory has settled on. */
const CHUNK_WORDS = 2000;

/** How many confirmed misspellings get their suggestions generated in the
 *  background after a render's checks settle. The cap keeps a long page from
 *  kicking off hundreds of ranking calls; the user only needs the first few
 *  clickable words to be instant. */
const BACKGROUND_SUGGESTION_LIMIT = 30;

/** Split `text` into word-aligned windows of at most `maxWords` words,
 *  as JS-string index ranges. A window ends at whitespace, so no word
 *  is ever split across two calls, and `text.slice` over the ranges
 *  reconstructs `text`. */
function chunkByWords(
  text: string,
  maxWords: number,
): Array<{ start: number; end: number }> {
  const out: Array<{ start: number; end: number }> = [];
  const n = text.length;
  let i = 0;
  while (i < n) {
    const start = i;
    let words = 0;
    while (i < n && words < maxWords) {
      while (i < n && /\s/.test(text[i])) i++;
      const wordStart = i;
      while (i < n && !/\s/.test(text[i])) i++;
      if (i > wordStart) words++;
    }
    out.push({ start, end: i });
  }
  return out;
}

export async function findMisspellings(
  text: string,
  stateIdx: number,
  callbacks?: MisspellCallbacks,
  options?: FindMisspellingsOptions,
): Promise<FindMisspellingsResult> {
  // The state mirrors whatever was last typed in this textarea.
  // Updating on every render is cheap (microseconds) and keeps
  // proper nouns / multi-word phrases learnable.
  updateState(stateIdx, text);

  // ── first pass, windowed ─────────────────────────────────────
  //
  // The wasm's `find_misspells` tokenises the whole string it is
  // handed and holds every token, so one call over a large
  // document sets a high-water mark in the worker's linear
  // memory that is never returned (wasm memory only grows). The
  // Gutenberg reader already paginates its chapters; the plain
  // editor does not, so a big paste is one unbounded call. Split
  // it into word-aligned windows of at most CHUNK_WORDS words.
  //
  // The byte offsets in the per-window hits are relative to the
  // window's UTF-8, so adding the running UTF-8 byte length of
  // the preceding windows reproduces exactly the offsets a
  // single whole-text call would have returned.
  const chunks = chunkByWords(text, CHUNK_WORDS);
  const encoder = new TextEncoder();
  const hits: Misspell[] = [];
  let byteBase = 0;
  for (const { start, end } of chunks) {
    if (options?.isCancelled?.()) {
      return { hits: [], drained: Promise.resolve() };
    }
    const window = text.slice(start, end);
    const windowHits = await analyzeMisspellsAsync(
      stateIdx, window, /* max */ 8, /* maxDistance */ 3,
    );
    for (const h of windowHits) {
      hits.push({
        word: h.word,
        start: h.start + byteBase,
        end: h.end + byteBase,
        caseOnly: h.caseOnly,
      });
    }
    byteBase += encoder.encode(window).length;
  }
  // If the round was cancelled while `analyze_misspells` was
  // in flight (e.g. user flipped chapters during the first-pass
  // wasm call), drop everything. The new round's resource
  // takes over from here.
  if (options?.isCancelled?.()) {
    return { hits: [], drained: Promise.resolve() };
  }
  if (hits.length === 0) {
    return { hits: [], drained: Promise.resolve() };
  }

  // ── surgical diff against the previous render ────────────────
  //
  // Build a (word, start, end) -> previous-entry map from the
  // previous confirmed list. A word whose surface form and
  // byte position are unchanged from the previous render is
  // a *typing-noise-free* reuse candidate: it was a misspell
  // before, it still is, no need to re-run the slow pool.
  //
  // We key on (word, start, end) rather than just `word`
  // because the same surface form can appear at multiple
  // positions in the text -- each occurrence has its own
  // confirmed-misspell lifecycle (the user might fix only
  // one of them).
  const prevByKey = new Map<string, MisspellWithRange>();
  if (options?.previouslyConfirmed) {
    for (const m of options.previouslyConfirmed) {
      prevByKey.set(`${m.word}|${m.start}|${m.end}`, m);
    }
  }

  const hintFor = new Map<
    string,
    { start: number; end: number; caseOnly?: boolean }
  >();
  for (const h of hits) {
    hintFor.set(h.word, {
      start: h.start,
      end: h.end,
      caseOnly: h.caseOnly,
    });
  }
  const words = Array.from(hintFor.keys());

  // First pass: fire `onMisspellCandidate` for EVERY first-pass
  // hit (and for surgical-diff reuses) immediately. The user
  // gets the underline in the same frame the resource
  // resolves; the slow check's only job from here is to
  // optionally demote verb inflections and the like.
  let reused = 0;
  let toCheck: string[] = [];
  for (const word of words) {
    if (options?.isCancelled?.()) {
      // Round was cancelled between the first-pass resolution
      // and now. The caller has already started a new round;
      // stop emitting candidates and bail out of the dispatch.
      return { hits: [], drained: Promise.resolve() };
    }
    const range = hintFor.get(word)!;
    const key = `${word}|${range.start}|${range.end}`;
    callbacks?.onMisspellCandidate?.({
      word, start: range.start, end: range.end, caseOnly: range.caseOnly,
    });
    if (range.caseOnly) {
      // The slow check lowercases, so it would demote a case-only flag.
      // The capitalization decision is final here.
      reused++;
    } else if (prevByKey.has(key)) {
      // Unchanged from the previous render: don't re-run the
      // slow check (the previous render already demoted or
      // confirmed it; either way the answer is settled).
      reused++;
    } else {
      toCheck.push(word);
    }
  }

  console.warn('[findMisspellings] dispatch',
               `textLen=${text.length} hits=${hits.length} words=${words.length}` +
               ` reused=${reused} toCheck=${toCheck.length}`);

  // Second pass: only the changed words hit the slow pool.
  // For unchanged words, the cross-pool memos already have
  // the bg.check / bg.suggest_for_misspell answers, but we
  // skip them entirely on the typing hot path -- the user
  // already saw those underlines render and the popup
  // already has the candidates cached.
  //
  // Track each slow check as a promise so the caller can
  // await `drained` and know when the round has fully
  // settled (or been cancelled). Without this, the
  // `drained` half would resolve the moment the first
  // pass returns, and the puppeteer probe would have to
  // guess a fixed grace period to wait for the slow pool.
  // Phase A: only the cheap known-word check runs for the whole
  // render. Suggestions are deliberately *not* generated here:
  // a page with 200 candidates would otherwise fan out 200
  // ranking calls before the check itself has settled.
  const confirmed = new Set<string>();
  const slowChecks: Promise<void>[] = [];
  for (const word of toCheck) {
    if (options?.isCancelled?.()) {
      // New round has started. Stop dispatching: every
      // additional `dispatchCheckKnown` would queue a fresh
      // wasm call in a sibling worker whose result the new
      // round is going to ignore anyway. The postMessage
      // round-trip + wasm invocation cost real RAM per
      // pending call; cancelling here is what keeps rapid
      // chapter flipping from blowing up the heap.
      break;
    }
    const slowCheck = (async () => {
      const known = await dispatchCheckKnown(word);
      // The slow check may have finished well after the
      // round was cancelled. Drop the result: the
      // component has moved on, the candidate list it
      // cares about is the *new* round's.
      if (options?.isCancelled?.()) return;
      if (known) {
        // The slow check says this is a valid Finnish form
        // after all. Demote -- drop the underline the
        // first-pass drew.
        const range = hintFor.get(word);
        if (range !== undefined) {
          console.warn('[findMisspellings] demote (slow check says known):', word);
          callbacks?.onDemoteMisspell?.({
            word, start: range.start, end: range.end,
          });
        }
      } else {
        // Confirmed misspell: keep it underlined and remember it for the
        // background suggestion phase.
        confirmed.add(word);
      }
    })();
    slowChecks.push(slowCheck);
  }

  // The function-level return is the *raw* first-pass range.
  // The Solid resource in App.tsx ignores it and listens on
  // the candidate/demote callbacks instead -- underlines
  // appear immediately from the first pass and the slow
  // check only refines them. Returning the raw hits is
  // preserved for callers (test harnesses, the per-render
  // counter) that want every flag, even ones we later demote.
  //
  // `drained` resolves when every slow-check we dispatched
  // (or none, if `toCheck` was empty) has settled. The
  // `.catch(() => undefined)` makes the `Promise.all`
  // resolve even if a single slow check throws -- the
  // round is "drained" either way and the resource needs
  // to fire its `onRenderDone` subscribers regardless of
  // individual call errors.
  const drained = Promise.all(slowChecks.map((p) => p.catch(() => undefined)))
    .then(() => {
      if (options?.isCancelled?.()) return;
      // Phase B: the check is settled, so the visible misspell list is
      // final. Warm the popup cache for at most the first thirty confirmed
      // words. These calls are fire-and-forget: `drained`/`onRenderDone`
      // must not wait for ranking, or the status would stay "checking…"
      // while suggestions are computed.
      let scheduled = 0;
      for (const word of toCheck) {
        if (!confirmed.has(word)) continue;
        if (scheduled >= BACKGROUND_SUGGESTION_LIMIT) break;
        scheduled++;
        void dispatchSuggestForMisspell(word, /* max */ 8, /* dist */ 3)
          .catch(() => undefined);
      }
    });
  return {
    hits,
    drained,
  };
}
