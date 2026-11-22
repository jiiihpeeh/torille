// The torille spell API, independent of *where* it runs.
//
// `spell-worker.ts` hosts this in a dedicated module worker, which is
// what Firefox uses (its MV3 background is an event page and can
// create workers). Chromium's MV3 background is a service worker,
// and a service worker cannot create a nested Worker, so the Chromium
// background imports this module directly (`background.ts` decides
// which host to use at runtime).
//
// Handlers return the response payload *without* the numeric request
// id; the host adds (worker) or resolves with (direct) it. A `null`
// return is a fire-and-forget op (`destroy-state`, `clear-state`,
// `update-state`, `debug`) that has no reply.

import { bg, initTorilleWasm } from './wasm-loader.js';
import { buildByteToUtf16Map } from './offsets.js';

// ───────────────────────────────────────────────────────────────────────
// §1  Debug
// ───────────────────────────────────────────────────────────────────────

// Workers don't see page localStorage and have no extension storage
// API by default, so the content script pushes the debug flag in via
// a 'debug' message whenever the options page toggles it. A direct
// host (Chromium service worker) gets the same message through the
// background. Also honour `localStorage.torille-debug = '1'` where the
// host has localStorage (the Firefox worker does not).
let TORILLE_DEBUG = false;
try {
  TORILLE_DEBUG = (typeof localStorage !== 'undefined') &&
                  localStorage.getItem('torille-debug') === '1';
} catch (_) { /* localStorage may be unavailable */ }

function dbg(label: string, ...args: unknown[]): void {
  if (!TORILLE_DEBUG) return;
  const head = label ? `[torille:${label}]` : '[torille]';
  console.log(head, ...args);
}

// ───────────────────────────────────────────────────────────────────────
// §2  Wasm lifecycle
// ───────────────────────────────────────────────────────────────────────
//
// `wasm-loader.ts` fetches the bytes and instantiates the module by
// hand. The URL is resolved against *this* module, which is always
// `dist/spell-worker/spell-core.js`, so the wasm sits next to it in
// both hosts.

let wasmReady: Promise<void> | null = null;

function ensureInit(): Promise<void> {
  if (wasmReady !== null) return wasmReady;
  wasmReady = (async (): Promise<void> => {
    dbg('core', 'init: starting wasm load');
    const wasmUrl = new URL('./torille_wasm_bg.wasm', import.meta.url);
    await initTorilleWasm(wasmUrl.toString());
    // Pay the lemma-set/catalogue cold start before the first user
    // keystroke. Failure here is non-fatal: the first interactive
    // call just pays the cost instead.
    try {
      bg.warm_caches();
    } catch (e) {
      dbg('core', 'warm_caches failed', String(e));
    }
    dbg('core', 'init: wasm ready');
  })().catch((e: unknown) => {
    wasmReady = null; // allow the next request to retry
    throw e;
  });
  return wasmReady;
}

// ───────────────────────────────────────────────────────────────────────
// §3  Caches
// ───────────────────────────────────────────────────────────────────────
//
// The background also keeps a shared cache, but messages still cross
// the worker's postMessage boundary on a hit. Caching here skips the
// wasm call for repeats within the same host (a tab reload respawns
// the Firefox worker; the Chromium service worker lives longer).
// Bounded so a long-lived host can't grow without limit.

const checkCache: Map<string, boolean> = new Map();
const CHECK_CACHE_LIMIT = 16384;

function checkCacheGet(word: string): boolean | undefined {
  const v = checkCache.get(word);
  if (v === undefined) return undefined;
  checkCache.delete(word);
  checkCache.set(word, v);
  return v;
}

function checkCacheSet(word: string, ok: boolean): void {
  if (checkCache.has(word)) {
    checkCache.delete(word);
  } else if (checkCache.size >= CHECK_CACHE_LIMIT) {
    const oldest = checkCache.keys().next().value;
    if (oldest !== undefined) checkCache.delete(oldest);
  }
  checkCache.set(word, ok);
}

const suggestCache: Map<string, string[]> = new Map();
const SUGGEST_CACHE_LIMIT = 2048;

// Every surface the user's saved word list generates. `set-user-words`
// rebuilds it; the check paths consult it before the wasm. It is a
// global layer, not per-textbox: a saved word is accepted everywhere.
let userForms: Set<string> = new Set();

function isUserWord(word: string): boolean {
  return userForms.has(word.toLowerCase());
}

/// Bounded Levenshtein for the saved-word suggestions. The set is small
/// (one entry per saved word times its paradigm), and the popup asks for a
/// handful of candidates, so a plain DP is enough.
function editDistance(a: string, b: string, max: number): number {
  if (Math.abs(a.length - b.length) > max) return max + 1;
  const previous = new Array<number>(b.length + 1);
  const current = new Array<number>(b.length + 1);
  for (let j = 0; j <= b.length; j++) previous[j] = j;
  for (let i = 1; i <= a.length; i++) {
    current[0] = i;
    let rowMin = current[0];
    for (let j = 1; j <= b.length; j++) {
      const cost = a[i - 1] === b[j - 1] ? 0 : 1;
      current[j] = Math.min(
        previous[j] + 1,
        current[j - 1] + 1,
        previous[j - 1] + cost,
      );
      if (current[j] < rowMin) rowMin = current[j];
    }
    if (rowMin > max) return max + 1;
    for (let j = 0; j <= b.length; j++) previous[j] = current[j];
  }
  return previous[b.length];
}

/// Saved-word forms within two edits of `word`, closest first.
function customSuggestions(word: string, max: number): string[] {
  if (userForms.size === 0 || max <= 0) return [];
  const lower = word.toLowerCase();
  const found: { form: string; distance: number }[] = [];
  for (const form of userForms) {
    if (form === lower) continue;
    const distance = editDistance(lower, form, 2);
    if (distance <= 2) found.push({ form, distance });
  }
  found.sort((a, b) => a.distance - b.distance || a.form.localeCompare(b.form));
  return found.slice(0, max).map((entry) => entry.form);
}

function mergeSuggestions(
  custom: string[],
  engine: string[],
  max: number,
): string[] {
  const out: string[] = [];
  const seen = new Set<string>();
  for (const word of [...custom, ...engine]) {
    if (!seen.has(word)) {
      seen.add(word);
      out.push(word);
      if (out.length >= max) break;
    }
  }
  return out;
}

function suggestCacheGet(word: string, max: number): string[] | undefined {
  const key = max + '\u0000' + word;
  const v = suggestCache.get(key);
  if (v === undefined) return undefined;
  suggestCache.delete(key);
  suggestCache.set(key, v);
  return v;
}

function suggestCacheSet(word: string, max: number, suggestions: string[]): void {
  const key = max + '\u0000' + word;
  if (suggestCache.has(key)) {
    suggestCache.delete(key);
  } else if (suggestCache.size >= SUGGEST_CACHE_LIMIT) {
    const oldest = suggestCache.keys().next().value;
    if (oldest !== undefined) suggestCache.delete(oldest);
  }
  suggestCache.set(key, suggestions);
}

// ───────────────────────────────────────────────────────────────────────
// §4  Response shape
// ───────────────────────────────────────────────────────────────────────

export type AnalyzeHit = {
  word: string;
  start: number;
  end: number;
  suggestions: string[];
};

export type CoreResponse =
  | { type: 'check'; ok: boolean }
  | { type: 'check-fast'; ok: boolean | null }
  | { type: 'suggest'; suggestions: string[] }
  | { type: 'check-batch'; inputId: number; ok: (boolean | null)[] }
  | { type: 'suggest-batch'; inputId: number; results: string[][] }
  | { type: 'create-state'; idx: number }
  | { type: 'analyze-text'; hits: AnalyzeHit[]; stateValid: boolean }
  | { type: 'suggest-with-state'; suggestions: string[] }
  | { type: 'check-syntax'; diagnostics: R_SyntaxDiagnostic[] }
  | { type: 'check-style'; diagnostics: R_SyntaxDiagnostic[] }
  | { type: 'check-typography'; diagnostics: R_SyntaxDiagnostic[] }
  | {
      type: 'check-grammar-style';
      diagnostics: R_SyntaxDiagnostic[];
      style: R_SyntaxDiagnostic[];
    }
  | { type: 'unique-surfaces'; surfaces: string[] }
  | { type: 'wizard-sheet'; sheet: string }
  | { type: 'wizard-infer'; report: unknown }
  | { type: 'wizard-paradigm'; paradigm: unknown }
  | { type: 'wizard-user-words'; result: unknown }
  | { type: 'wizard-user-words-add'; result: unknown }
  | { type: 'wizard-user-words-remove'; result: unknown }
  | { type: 'set-user-words' };

// ───────────────────────────────────────────────────────────────────────
// §5  Handlers
// ───────────────────────────────────────────────────────────────────────

function handleCheck(
  msg: Extract<R_WorkerInbound, { type: 'check' }>,
): CoreResponse {
  if (isUserWord(msg.word)) {
    dbg('core', 'check:user-word', msg.word);
    return { type: 'check', ok: true };
  }
  let ok = false;
  try {
    ok = bg.check(msg.word);
  } catch (err) {
    // Let the host turn a throw into a transport error. Reporting
    // `false` here would mark a correctly-spelled word as wrong.
    dbg('core', 'check threw', msg.word, String(err));
    throw err;
  }
  dbg('core', 'check', msg.word, '->', ok);
  checkCacheSet(msg.word, ok);
  return { type: 'check', ok };
}

function handleCheckFast(
  msg: Extract<R_WorkerInbound, { type: 'check-fast' }>,
): CoreResponse {
  if (isUserWord(msg.word)) {
    dbg('core', 'check-fast:user-word', msg.word);
    return { type: 'check-fast', ok: true };
  }
  // Same first-pass gate as `handleAnalyzeText`, scoped to one word.
  // A hit means "misspelled"; no hits means the fast gates accept it.
  // This is the popup's "is the word under the caret wrong?" probe --
  // the full `check` can spend seconds in the engine gate on an
  // unseen word, which freezes the caret popup.
  const hits = bg.analyze_misspells(msg.idx, msg.word);
  const ok = hits.length === 0;
  dbg('core', 'check-fast', msg.word, '->', ok);
  return { type: 'check-fast', ok };
}

function handleSuggest(
  msg: Extract<R_WorkerInbound, { type: 'suggest' }>,
): CoreResponse {
  const engine = Array.from(bg.suggest_words(msg.word, msg.max));
  const suggestions = mergeSuggestions(
    customSuggestions(msg.word, msg.max), engine, msg.max);
  dbg('core', 'suggest', msg.word, '->', suggestions.length, 'candidates');
  suggestCacheSet(msg.word, msg.max, suggestions);
  return { type: 'suggest', suggestions };
}

// Batch counterpart of `handleCheck`: fill from the cache, run wasm
// only on misses, and return an array parallel to `words`.
function handleCheckBatch(
  msg: Extract<R_WorkerInbound, { type: 'check-batch' }>,
): CoreResponse {
  const words = msg.words;
  const ok: (boolean | null)[] = new Array(words.length);
  const missIdx: number[] = [];
  const missWords: string[] = [];
  for (let i = 0; i < words.length; i++) {
    if (isUserWord(words[i])) {
      ok[i] = true;
      continue;
    }
    const cached = checkCacheGet(words[i]);
    if (cached !== undefined) {
      ok[i] = cached;
    } else {
      missIdx.push(i);
      missWords.push(words[i]);
    }
  }
  for (let j = 0; j < missWords.length; j++) {
    const w = missWords[j];
    try {
      const v = bg.check(w);
      ok[missIdx[j]] = v;
      checkCacheSet(w, v);
    } catch (err) {
      // Unknown, not misspelled. The caller renders neither an
      // underline nor a "correct" verdict for a `null` entry.
      dbg('core', 'check-batch entry threw',
          { inputId: msg.inputId, word: w, err: String(err) });
      ok[missIdx[j]] = null;
    }
  }
  dbg('core', 'check-batch',
      { inputId: msg.inputId, total: words.length, misses: missWords.length });
  return { type: 'check-batch', inputId: msg.inputId, ok };
}

function handleSuggestBatch(
  msg: Extract<R_WorkerInbound, { type: 'suggest-batch' }>,
): CoreResponse {
  const items = msg.items;
  const results: string[][] = new Array(items.length);
  const missIdx: number[] = [];
  const missItems: { word: string; max: number }[] = [];
  for (let i = 0; i < items.length; i++) {
    const it = items[i];
    const cached = suggestCacheGet(it.word, it.max);
    if (cached !== undefined) {
      results[i] = cached;
    } else {
      missIdx.push(i);
      missItems.push(it);
    }
  }
  for (let j = 0; j < missItems.length; j++) {
    const it = missItems[j];
    try {
      const engine = Array.from(bg.suggest_words(it.word, it.max));
      const sugg = mergeSuggestions(
        customSuggestions(it.word, it.max), engine, it.max);
      results[missIdx[j]] = sugg;
      suggestCacheSet(it.word, it.max, sugg);
    } catch (err) {
      dbg('core', 'suggest-batch entry threw',
          { inputId: msg.inputId, word: it.word, err: String(err) });
      results[missIdx[j]] = [];
    }
  }
  dbg('core', 'suggest-batch',
      { inputId: msg.inputId, total: items.length, misses: missItems.length });
  return { type: 'suggest-batch', inputId: msg.inputId, results };
}

function handleCreateState(): CoreResponse {
  const idx = bg.create_text_area_state();
  dbg('core', 'create-state ->', idx);
  return { type: 'create-state', idx };
}

function handleDestroyState(
  msg: Extract<R_WorkerInbound, { type: 'destroy-state' }>,
): null {
  bg.destroy_text_area_state(msg.idx);
  dbg('core', 'destroy-state', msg.idx);
  return null;
}

function handleClearState(
  msg: Extract<R_WorkerInbound, { type: 'clear-state' }>,
): null {
  bg.clear_state(msg.idx);
  dbg('core', 'clear-state', msg.idx);
  return null;
}

function handleUpdateState(
  msg: Extract<R_WorkerInbound, { type: 'update-state' }>,
): null {
  bg.update_state_from_text(msg.idx, msg.text);
  dbg('core', 'update-state', { idx: msg.idx, len: msg.text.length });
  return null;
}

function handleAnalyzeText(
  msg: Extract<R_WorkerInbound, { type: 'analyze-text' }>,
): CoreResponse {
  // The background may have restarted (event-page suspension, a request
  // timeout that tore the worker down, an extension reload), losing every
  // per-textbox slot. Report that instead of returning an empty hit list,
  // which the content script would otherwise render as "no misspellings".
  if (!bg.text_area_state_exists(msg.idx)) {
    dbg('core', 'analyze-text:stale-state', { idx: msg.idx });
    return { type: 'analyze-text', hits: [], stateValid: false };
  }
  // First pass only: ranges, no suggestions. `analyze_text` would run
  // the suggestion ranker for every misspelling on every keystroke
  // (~5–9 s on a short sentence). Suggestions are fetched on click
  // through `suggest-with-state`.
  const arr = bg.analyze_misspells(msg.idx, msg.text);
  // Rust reports UTF-8 byte offsets; the content script slices a JS
  // string, so translate once per text (not once per hit).
  const offsetMap = buildByteToUtf16Map(msg.text);
  const hits: AnalyzeHit[] = [];
  for (let i = 0; i < arr.length; i++) {
    const h = arr[i] as { word: string; start: number; end: number };
    if (isUserWord(h.word)) continue;
    const start = offsetMap[h.start];
    const end = offsetMap[h.end];
    if (start === undefined || end === undefined) {
      dbg('core', 'analyze-text: offset out of range',
          { word: h.word, start: h.start, end: h.end });
      continue;
    }
    hits.push({ word: h.word, start, end, suggestions: [] });
  }
  dbg('core', 'analyze-text',
      { idx: msg.idx, len: msg.text.length, hits: hits.length });
  return { type: 'analyze-text', hits, stateValid: true };
}

function handleSuggestWithState(
  msg: Extract<R_WorkerInbound, { type: 'suggest-with-state' }>,
): CoreResponse {
  // Bounded Levenshtein over the lemma list (~10 ms), not the full
  // `suggest_words` pipeline (~4–5 s). This is the click path, so it
  // must feel instant.
  const engine = Array.from(
    bg.suggest_for_misspell(msg.idx, msg.word, msg.max, 2));
  const suggestions = mergeSuggestions(
    customSuggestions(msg.word, msg.max), engine, msg.max);
  dbg('core', 'suggest-with-state',
      { word: msg.word, n: suggestions.length });
  return { type: 'suggest-with-state', suggestions };
}

type RawDiagnostic = {
  kind: string;
  rule_id: string;
  severity: string;
  confidence: string;
  start: number;
  end: number;
  message: string;
  expected: string | null;
  explanation?: { relation: string; key: string } | null;
  corrections?: {
    score: number;
    note: string;
    edits?: { start: number; end: number; replacement: string }[];
  }[];
};

function toContentDiagnostics(
  text: string,
  rawDiagnostics: RawDiagnostic[],
): R_SyntaxDiagnostic[] {
  // The wasm side reports UTF-8 byte offsets. The content script slices
  // a JS string, so convert the diagnostic and edit offsets once here.
  const offsetMap = buildByteToUtf16Map(text);
  const diagnostics: R_SyntaxDiagnostic[] = [];
  for (const d of rawDiagnostics) {
    const start = offsetMap[d.start];
    const end = offsetMap[d.end];
    if (start === undefined || end === undefined) continue;
    const corrections: R_SyntaxCorrection[] = [];
    for (const c of d.corrections ?? []) {
      const edits: R_SyntaxEdit[] = [];
      let usable = true;
      for (const e of c.edits ?? []) {
        const editStart = offsetMap[e.start];
        const editEnd = offsetMap[e.end];
        if (editStart === undefined || editEnd === undefined) {
          usable = false;
          break;
        }
        edits.push({
          start: editStart,
          end: editEnd,
          replacement: e.replacement,
        });
      }
      if (usable) {
        corrections.push({ score: c.score, note: c.note, edits });
      }
    }
    diagnostics.push({
      kind: d.kind,
      rule_id: d.rule_id,
      severity: d.severity,
      confidence: d.confidence,
      start,
      end,
      message: d.message,
      expected: d.expected ?? null,
      explanation: d.explanation ?? null,
      corrections,
    });
  }
  return diagnostics;
}

function handleCheckSyntax(
  msg: Extract<R_WorkerInbound, { type: 'check-syntax' }>,
): CoreResponse {
  const raw = bg.check_grammar_style_with_state(msg.idx, msg.text, true, false);
  const diagnostics = toContentDiagnostics(msg.text, raw.diagnostics ?? []);
  dbg('core', 'check-syntax',
      { idx: msg.idx, len: msg.text.length, diagnostics: diagnostics.length });
  return { type: 'check-syntax', diagnostics };
}

function handleCheckStyle(
  msg: Extract<R_WorkerInbound, { type: 'check-style' }>,
): CoreResponse {
  const raw = bg.check_grammar_style_with_state(msg.idx, msg.text, false, true);
  const diagnostics = toContentDiagnostics(msg.text, raw.style ?? []);
  dbg('core', 'check-style',
      { idx: msg.idx, len: msg.text.length, diagnostics: diagnostics.length });
  return { type: 'check-style', diagnostics };
}

function handleCheckTypography(
  msg: Extract<R_WorkerInbound, { type: 'check-typography' }>,
): CoreResponse {
  const raw = bg.check_typography(msg.text);
  const diagnostics = toContentDiagnostics(msg.text, raw.diagnostics ?? []);
  dbg('core', 'check-typography',
      { len: msg.text.length, diagnostics: diagnostics.length });
  return { type: 'check-typography', diagnostics };
}

function handleUniqueSurfaces(
  msg: Extract<R_WorkerInbound, { type: 'unique-surfaces' }>,
): CoreResponse {
  const bytes = bg.unique_surfaces(msg.text);
  const surfaces = new TextDecoder().decode(bytes).split('\n').filter(Boolean);
  dbg('core', 'unique-surfaces', { len: msg.text.length, surfaces: surfaces.length });
  return { type: 'unique-surfaces', surfaces };
}

function handleSetUserWords(
  msg: Extract<R_WorkerInbound, { type: 'set-user-words' }>,
): CoreResponse {
  try {
    const forms = bg.wizard_user_forms(msg.text);
    userForms = new Set(Array.from(forms, (form) => String(form).toLowerCase()));
    // A word that was misspelled before this update may be a saved
    // word now (and the reverse). Do not let the old verdict linger.
    checkCache.clear();
    suggestCache.clear();
    dbg('core', 'set-user-words', { forms: userForms.size });
  } catch (err) {
    userForms = new Set();
    dbg('core', 'set-user-words failed', String(err));
  }
  // Ack the push: the background awaits it before the first check, so
  // a null (fire-and-forget) return would hang that request.
  return { type: 'set-user-words' };
}

function handleWizardSheet(): CoreResponse {
  return { type: 'wizard-sheet', sheet: bg.wizard_sheet() };
}

function handleWizardInfer(
  msg: Extract<R_WorkerInbound, { type: 'wizard-infer' }>,
): CoreResponse {
  return {
    type: 'wizard-infer',
    report: bg.wizard_infer_with_number(
      msg.word, msg.pos, msg.answers, msg.number ?? ''),
  };
}

function handleWizardParadigm(
  msg: Extract<R_WorkerInbound, { type: 'wizard-paradigm' }>,
): CoreResponse {
  return {
    type: 'wizard-paradigm',
    paradigm: bg.wizard_paradigm(msg.word, msg.class, msg.pos),
  };
}

function handleWizardUserWords(
  msg: Extract<R_WorkerInbound, { type: 'wizard-user-words' }>,
): CoreResponse {
  return {
    type: 'wizard-user-words',
    result: bg.wizard_user_words(msg.text),
  };
}

function handleWizardUserWordsAdd(
  msg: Extract<R_WorkerInbound, { type: 'wizard-user-words-add' }>,
): CoreResponse {
  return {
    type: 'wizard-user-words-add',
    result: bg.wizard_user_words_add_with_number(
      msg.text, msg.word, msg.class, msg.pos, msg.number ?? ''),
  };
}

function handleWizardUserWordsRemove(
  msg: Extract<R_WorkerInbound, { type: 'wizard-user-words-remove' }>,
): CoreResponse {
  return {
    type: 'wizard-user-words-remove',
    result: bg.wizard_user_words_remove(msg.text, msg.word),
  };
}

function handleCheckGrammarStyle(
  msg: Extract<R_WorkerInbound, { type: 'check-grammar-style' }>,
): CoreResponse {
  const raw = msg.payloads !== undefined && msg.payloads.length > 0
    ? bg.check_grammar_style_from_morphology(
        msg.idx, msg.text, msg.payloads, msg.syntax, msg.style)
    : bg.check_grammar_style_with_state(
        msg.idx, msg.text, msg.syntax, msg.style);
  return {
    type: 'check-grammar-style',
    diagnostics: toContentDiagnostics(msg.text, raw.diagnostics ?? []),
    style: toContentDiagnostics(msg.text, raw.style ?? []),
  };
}

// ───────────────────────────────────────────────────────────────────────
// §6  Public surface
// ───────────────────────────────────────────────────────────────────────

export interface SpellCore {
  init(): Promise<void>;
  handle(msg: R_WorkerInbound): Promise<CoreResponse | null>;
  setDebug(on: boolean): void;
}

export function createSpellCore(): SpellCore {
  return {
    init: ensureInit,

    setDebug(on: boolean): void {
      TORILLE_DEBUG = on;
      dbg('core', 'debug ->', TORILLE_DEBUG);
    },

    async handle(msg: R_WorkerInbound): Promise<CoreResponse | null> {
      // `debug` must work before (and without) a successful wasm
      // load, so it stays ahead of the init await.
      if (msg.type === 'debug') {
        TORILLE_DEBUG = !!msg.on;
        dbg('core', 'debug ->', TORILLE_DEBUG);
        return null;
      }
      await ensureInit();
      switch (msg.type) {
        case 'check': return handleCheck(msg);
        case 'check-fast': return handleCheckFast(msg);
        case 'suggest': return handleSuggest(msg);
        case 'check-batch': return handleCheckBatch(msg);
        case 'suggest-batch': return handleSuggestBatch(msg);
        case 'create-state': return handleCreateState();
        case 'destroy-state': return handleDestroyState(msg);
        case 'clear-state': return handleClearState(msg);
        case 'update-state': return handleUpdateState(msg);
        case 'analyze-text': return handleAnalyzeText(msg);
        case 'suggest-with-state': return handleSuggestWithState(msg);
        case 'check-syntax': return handleCheckSyntax(msg);
        case 'check-style': return handleCheckStyle(msg);
        case 'check-typography': return handleCheckTypography(msg);
        case 'check-grammar-style': return handleCheckGrammarStyle(msg);
        case 'unique-surfaces': return handleUniqueSurfaces(msg);
        case 'set-user-words': return handleSetUserWords(msg);
        case 'wizard-sheet': return handleWizardSheet();
        case 'wizard-infer': return handleWizardInfer(msg);
        case 'wizard-paradigm': return handleWizardParadigm(msg);
        case 'wizard-user-words': return handleWizardUserWords(msg);
        case 'wizard-user-words-add': return handleWizardUserWordsAdd(msg);
        case 'wizard-user-words-remove': return handleWizardUserWordsRemove(msg);
        default: {
          const unknown = msg as { type?: string };
          throw new Error(`unknown message type: ${unknown.type ?? '?'}`);
        }
      }
    },
  };
}
