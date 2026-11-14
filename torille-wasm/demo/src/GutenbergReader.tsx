// GutenbergReader: a Finnish public-domain book panel.
//
// On mount the reader loads the local corpus catalogue
// (`books/catalog.json`, written by `tools/fetch_finnish_books.py`),
// lets the user filter it by title/author, and shows the chosen book's
// UTF-8 text (`books/<id>.txt`) through the same `Editor` component the
// free-text panels use. Nothing is fetched cross-origin. Long chapters
// are split into ~`PAGE_WORD_BUDGET`-word pages (see `paginateChapter`)
// so one render never hands the spell-checker a whole multi-thousand-word
// chapter; the extra pages show up as "LUKU I — osa 2/5" entries in the
// chapter dropdown. Spell-check works the same as the regular panels
// because the underlying state goes through the same wasm worker pool.
//
// Book selection persists across chapter changes -- the
// previous chapter is dropped (along with any in-place edits)
// the moment the user picks a new chapter. The reader is
// therefore best understood as a "chapter viewer", not a
// long-form editor: edits are intentionally not preserved
// across chapter navigation, matching the reader's read-mostly
// role. A future "save edits" feature would slot in here.

import {
  createEffect,
  createMemo,
  createResource,
  createSignal,
  onCleanup,
  onMount,
  Show,
  untrack,
} from 'solid-js';
import { countWords, tokenize } from './tokenize';
import {
  findMisspellings,
  MisspellCallbacks,
  MisspellWithRange,
} from './find-misspellings';
import {
  checkGrammarStyle,
  createState,
  destroyState,
  ready as wasmReady,
} from './spell-client';
import { Editor } from './Editor';
import type { FocusTarget } from './Editor';
import {
  builtInGrammarCheckEnabled,
  builtInSpellCheckEnabled,
  builtInStyleCheckEnabled,
  useExtension,
} from './spell-check-toggle';
import type { DiagnosticSpan } from '../../protocol.js';
import {
  Book,
  CatalogEntry,
  Chapter,
  filterCatalog,
  loadBook,
  loadCatalog,
} from './gutenberg';

/** Read the initial book id from the URL (?book= or ?bookId=). */
function bookIdFromUrl(): number {
  try {
    const params = new URLSearchParams(globalThis.location?.search ?? '');
    const b = params.get('book') ?? params.get('bookId');
    if (b === null) return -1;
    const n = Number(b);
    return Number.isFinite(n) ? n : -1;
  } catch {
    return -1;
  }
}

/** Read the initial chapter index from the URL (?chapter=). */
function chapterFromUrl(): number {
  try {
    const params = new URLSearchParams(globalThis.location?.search ?? '');
    const c = params.get('chapter');
    if (c === null) return -1;
    const n = Number(c);
    return Number.isFinite(n) && n >= 0 ? Math.floor(n) : -1;
  } catch {
    return -1;
  }
}

/** Update the URL to reflect the currently opened book and chapter. */
function updateBookUrl(bookId: number, chapterIdx: number): void {
  try {
    const url = new URL(globalThis.location.href);
    if (bookId >= 0) {
      url.searchParams.set('book', String(bookId));
    } else {
      url.searchParams.delete('book');
    }
    if (chapterIdx >= 0) {
      url.searchParams.set('chapter', String(chapterIdx));
    } else {
      url.searchParams.delete('chapter');
    }
    globalThis.history?.replaceState({}, '', url.toString());
  } catch {
    // ignore URL update failures
  }
}

/** How many catalogue cards one page of the picker shows. */
const PAGE_SIZE = 24;
type SortKey = 'title' | 'author' | 'id';

/** Finnish collation, with leading punctuation ignored so `"Muru"` sorts
 * with the `M` titles instead of before every letter. */
const collator = new Intl.Collator('fi', { sensitivity: 'base', numeric: true });
const titleKey = (title: string): string =>
  title.replace(/^[^\p{L}\p{N}]+/u, '');

export function GutenbergReader() {
  // The downloaded catalogue (`books/catalog.json`), loaded once.
  const [catalogResource] = createResource<CatalogEntry[]>(() => loadCatalog());
  const catalog = createMemo<CatalogEntry[]>(() => catalogResource() ?? []);

  // Local filter + sort over the catalogue (title / author).
  const [filterQuery, setFilterQuery] = createSignal<string>('');
  const [sortBy, setSortBy] = createSignal<SortKey>('id');
  const [page, setPage] = createSignal<number>(1);
  const sortedResults = createMemo<CatalogEntry[]>(() => {
    const list = filterCatalog(catalog(), filterQuery()).slice();
    list.sort((a, b) => {
      switch (sortBy()) {
        case 'author':
          return (
            collator.compare(a.author, b.author) ||
            collator.compare(titleKey(a.title), titleKey(b.title))
          );
        case 'id':
          return a.id - b.id;
        default:
          return (
            collator.compare(titleKey(a.title), titleKey(b.title)) ||
            collator.compare(a.author, b.author)
          );
      }
    });
    return list;
  });
  const pageCount = createMemo(() =>
    Math.max(1, Math.ceil(sortedResults().length / PAGE_SIZE)),
  );
  const pageResults = createMemo<CatalogEntry[]>(() => {
    const start = (page() - 1) * PAGE_SIZE;
    return sortedResults().slice(start, start + PAGE_SIZE);
  });
  // A new query or sort starts at the first page. The separate clamp effect
  // keeps the page valid when a filter shrinks the result set.
  createEffect(() => {
    filterQuery();
    sortBy();
    setPage(1);
  });
  createEffect(() => {
    const total = pageCount();
    setPage((current) => Math.min(Math.max(1, current), total));
  });
  const clearFilter = (): void => {
    setFilterQuery('');
  };
  const pickRandom = (): void => {
    const list = sortedResults();
    if (list.length === 0) return;
    const options = list.filter((entry) => entry.id !== bookId());
    const pool = options.length > 0 ? options : list;
    setBookId(pool[Math.floor(Math.random() * pool.length)].id);
  };

  // The chosen book. Starts from URL if present, else first catalogue entry.
  const initialBook = bookIdFromUrl();
  const [bookId, setBookId] = createSignal<number>(initialBook);
  createEffect(() => {
    const list = catalog();
    if (list.length === 0) return;
    const current = untrack(bookId);
    if (current < 0) {
      setBookId(list[0].id);
      return;
    }
    // If the book from URL isn't in the catalogue, fall back to first
    const exists = list.some((c) => c.id === current);
    if (!exists) {
      setBookId(list[0].id);
    }
  });

  // Currently displayed chapter (0-based). Start from URL if present.
  const initialChapter = chapterFromUrl();
  const [chapterIdx, setChapterIdx] = createSignal<number>(initialChapter >= 0 ? initialChapter : 0);
  // In-editor text for the current chapter. Reset on chapter change.
  const [text, setText] = createSignal<string>('');
  const [debouncedText, setDebouncedText] = createSignal<string>('');
  const [focused, setFocused] = createSignal<FocusTarget | null>(null);

  // Spell-check state, mirrored from TextPanel.
  const [stateIdx, setStateIdx] = createSignal<number>(-1);
  const [candidates, setCandidates] = createSignal<MisspellWithRange[]>([]);
  const [demoted, setDemoted] = createSignal<Set<string>>(new Set());
  const [renderGen, setRenderGen] = createSignal(0);
  let lastCandidates: MisspellWithRange[] = [];
  let checkTimer: ReturnType<typeof setTimeout> | undefined;

  // Grammar and style are opt-in and run on the same debounced chapter text
  // as the spell check. The worker chunks the page, reports progress and
  // cancels a stale run when the chapter changes.
  const [grammarProgress, setGrammarProgress] = createSignal<
    { done: number; total: number; ms?: number } | null
  >(null);
  const grammarBatchTimes: number[] = [];
  let grammarRun = 0;

  // Load the book. Re-fires on `bookId` change. Solid's
  // resource is the right shape here -- we get a single
  // "loading" state, then the parsed Book, or an Error.
  // Load the chosen book from the local corpus. Before the catalogue has
  // loaded, `bookId` is -1: return nothing rather than throwing (a throw
  // during the initial render aborts the whole app).
  // The catalogue loads asynchronously, so `bookId` alone is not enough to
  // identify the book: the first fetch can run before the entries arrive and
  // then the resource never re-fires because the id did not change. On a
  // fresh `?book=…` load that left the reader with the right selected title
  // but an empty chapter until the user clicked a book. Track the catalogue
  // alongside the id so the resource re-fetches as soon as it is available.
  const [bookResource] = createResource<
    Book | undefined,
    { id: number; entries: CatalogEntry[] }
  >(
    () => ({ id: bookId(), entries: catalog() }),
    ({ id, entries }, info) => {
      const entry = entries.find((c) => c.id === id);
      if (!entry) return undefined;
      return loadBook(entry, (info as { signal?: AbortSignal }).signal);
    },
  );

  // On a book change, go back to the first chapter (unless URL specifies otherwise).
  // The chapter-sync effect below then loads that chapter's body.
  let lastBookForChapterReset = -2;
  createEffect(() => {
    const book = bookResource();
    // Do not consume the initial-URL marker until the actual book is
    // loaded. `bookId` is set from the URL immediately, while the
    // catalogue (and therefore the book) is still loading; an earlier
    // version treated that as the initial load, saw no `chapters`, and
    // reset a valid `?chapter=2` to zero before the text arrived.
    if (!book) return;
    const id = book.id;
    if (lastBookForChapterReset === -2) {
      // Initial load - respect URL chapter if valid
      lastBookForChapterReset = id;
      const urlChap = chapterFromUrl();
      if (urlChap >= 0 && urlChap < book.chapters.length) {
        setChapterIdx(urlChap);
      } else {
        setChapterIdx(0);
      }
      return;
    }
    if (lastBookForChapterReset !== id) {
      lastBookForChapterReset = id;
      setChapterIdx(0);
    }
  });

  // Sync the displayed text to the current chapter. This is
  // the "pagination" step: each chapter flip replaces the
  // editor's contents with the new chapter's body. Must be
  // `createEffect` (not `createMemo`) because the body has
  // side effects (`setText`, `setDebouncedText`); a memo's
  // value is supposed to be a pure derivation.
  //
  // `stateIdx` is read via `untrack`: we only want to know
  // which slot to destroy, not subscribe to it. Without the
  // untrack, this effect re-fires whenever `stateIdx`
  // changes -- including the write we do here (`setStateIdx(-1)`)
  // and the resolve the sibling state-allocation effect
  // performs (which sets it to the new idx). That pair of
  // effect->effect writes used to loop indefinitely,
  // destroying and re-creating wasm state slots on every
  // tick. Tracking only `bookResource` and `chapterIdx`
  // keeps the loop out.
  createEffect(() => {
    const book = bookResource();
    if (!book) return;
    const ch = book.chapters[chapterIdx()];
    // Gutenberg's plain-text files use CRLF. Normalise here so the
    // editor, the wasm spell-checker and the bookmarked offsets all
    // work on the same LF text (a textarea normalises CRLF anyway).
    const body = (ch?.body ?? '').replace(/\r\n?/g, '\n');
    // Hard reset of the wasm per-textbox state on every
    // chapter flip. The previous chapter's
    // `proper_nouns: HashSet<String>` would stay alive in
    // wasm linear memory until the next `update` call
    // does `self.proper_nouns.clear()` -- dropping the
    // slot up front and re-creating it is a fast,
    // idempotent way to start the new chapter on a clean
    // slate.
    //
    // `createState()` is async (round-trips the wasm
    // worker for a fresh slot idx); the sibling
    // state-allocation effect below performs the actual
    // create once the wasm is ready. Here we only
    // (a) destroy the previous slot, and (b) flip
    // `stateIdx` to -1 so the spell-check resource
    // short-circuits with no hits until the new slot is
    // available (the resource already guards on `idx < 0`).
    const idx = untrack(stateIdx);
    if (idx >= 0) {
      destroyState(idx);
      setStateIdx(-1);
    }
    setText(body);
    setDebouncedText(body);
    // Update URL to reflect current book and chapter
    updateBookUrl(book.id, chapterIdx());
  });

  // Async slot allocation. Tracks ONLY `bookResource`
  // and `chapterIdx`. The `stateIdx` read for the
  // `>= 0` guard is also untracked, otherwise this
  // would re-fire on every `stateIdx` write and pile up
  // duplicate createState calls.
  //
  // When stateIdx is -1 (just reset by the chapter-sync
  // effect above, or never allocated on mount), fires
  // a fresh createState and writes the resolved idx
  // back into the stateIdx signal. The spell-check
  // resource's tracker reads stateIdx, so the resolve
  // triggers a fresh render with the right per-textbox
  // state slot.
  createEffect(() => {
    const book = bookResource();
    const _chap = chapterIdx();
    if (!book) return;
    if (untrack(stateIdx) >= 0) return;
    let cancelled = false;
    const myBook = book;
    const myChap = _chap;
    createState().then((newIdx) => {
      if (cancelled) return;
      // Guard against a chapter flip that happened
      // while we were waiting on the wasm worker: only
      // commit the new idx if we're still on the same
      // chapter+book that requested it.
      if (bookResource() !== myBook || chapterIdx() !== myChap) {
        destroyState(newIdx);
        return;
      }
      setStateIdx(newIdx);
    });
    onCleanup(() => { cancelled = true; });
  });

  const onText = (next: string): void => {
    setText(next);
    if (checkTimer !== undefined) clearTimeout(checkTimer);
    checkTimer = setTimeout(() => {
      setDebouncedText(next);
      checkTimer = undefined;
    }, 150);
  };

  // Keep URL in sync when book/chapter change via UI
  createEffect(() => {
    const book = bookResource();
    const id = book?.id ?? bookId();
    if (id < 0) return;
    const ch = chapterIdx();
    updateBookUrl(id, ch);
  });

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

  // Spell-check resource. Mirrors TextPanel's wiring but
  // also passes an `isCancelled` closure so the slow
  // per-word pool stops dispatching when the user flips
  // to a different chapter. Without this, a rapid
  // sequence of next-chapter clicks (5-10 in a few
  // seconds) queues 5-10 rounds of per-word wasm calls
  // in the sibling-worker pool -- the round-trip latency
  // climbs to 17+ s by the 8th flip and the wasm
  // workers' heap grows until the browser OOMs. The
  // `isCancelled` check stops the dispatch loop
  // immediately on a fresh round and drops in-flight
  // pool results when they finish.
  //
  // The resource awaits the `drained` half of
  // `findMisspellings` before resolving, so the
  // `__torille.panels[i].onRenderDone` subscribers fire
  // only after the slow-check pool has fully settled.
  const renderDoneSubscribers = new Set<() => void>();
  const [spellResource] = createResource(
    // The toggle is part of the tracker's input so flipping
    // it invalidates the resource and triggers a fresh
    // fetcher call -- where the OFF branch returns
    // immediately without any wasm calls. Same shape as
    // TextPanel's wiring.
    () => ({
      text: debouncedText(),
      idx: stateIdx(),
      on: builtInSpellCheckEnabled(),
    }),
    async ({ text, idx, on }): Promise<unknown[]> => {
      if (!on) {
        const myGen = renderGen() + 1;
        setRenderGen(myGen);
        setCandidates([]);
        setDemoted(new Set<string>());
        setFocused(null);
        for (const sub of renderDoneSubscribers) sub();
        return [];
      }
      if (idx < 0) return [];
      const myGen = renderGen() + 1;
      setRenderGen(myGen);
      const prevForDiff = lastCandidates;
      setCandidates([]);
      setDemoted(new Set<string>());
      const cb: MisspellCallbacks = {
        onMisspellCandidate: (m) => {
          if (renderGen() !== myGen) return;
          setCandidates((cur) => {
            if (cur.some((e) => e.start === m.start && e.end === m.end)) {
              return cur;
            }
            const next = [...cur, m];
            lastCandidates = next;
            return next;
          });
        },
        onDemoteMisspell: (m) => {
          if (renderGen() !== myGen) return;
          if (!candidates().some((c) => c.word === m.word)) return;
          setDemoted((cur) => {
            if (cur.has(m.word)) return cur;
            const next = new Set(cur);
            next.add(m.word);
            return next;
          });
        },
      };
      const result = await findMisspellings(text, idx, cb, {
        previouslyConfirmed: prevForDiff,
        // `isCancelled` is a closure over `myGen`; the
        // round is current iff `myGen` still matches the
        // resource's `renderGen()` -- which the next
        // chapter flip will bump.
        isCancelled: () => renderGen() !== myGen,
      });
      await result.drained;
      if (renderGen() === myGen) {
        for (const sub of renderDoneSubscribers) sub();
      }
      return result.hits;
    },
  );

  const misspellings = createMemo(() => {
    const dem = demoted();
    return candidates().filter((m) => !dem.has(m.word));
  });

  // One chunked worker call resolves morphology once and returns both
  // streams. The worker posts `check-progress` between sentence batches and
  // cancels the previous run when the chapter text changes.
  const [grammarStyle] = createResource(
    () => ({
      text: debouncedText(),
      idx: stateIdx(),
      syntax: builtInGrammarCheckEnabled(),
      style: builtInStyleCheckEnabled(),
    }),
    async ({ text, idx, syntax, style }): Promise<{
      diagnostics: DiagnosticSpan[];
      style: DiagnosticSpan[];
    }> => {
      if ((!syntax && !style) || idx < 0) {
        setGrammarProgress(null);
        return { diagnostics: [], style: [] };
      }
      const run = ++grammarRun;
      grammarBatchTimes.length = 0;
      const report = (done: number, total: number, ms?: number) => {
        if (run !== grammarRun) return;
        if (ms !== undefined) grammarBatchTimes.push(ms);
        setGrammarProgress({ done, total, ms });
      };
      try {
        return await checkGrammarStyle(
          idx,
          text.replace(/\r\n?/g, '\n'),
          syntax,
          style,
          report,
        );
      } finally {
        if (run === grammarRun) setGrammarProgress(null);
      }
    },
  );
  const grammarDiagnostics = () => grammarStyle()?.diagnostics ?? [];
  const styleDiagnostics = () => grammarStyle()?.style ?? [];

  // ── navigation helpers ────────────────────────────────────
  const goPrev = (): void => {
    setChapterIdx((i) => Math.max(0, i - 1));
  };
  const goNext = (): void => {
    const total = bookResource()?.chapters.length ?? 0;
    setChapterIdx((i) => Math.min(total - 1, i + 1));
  };
  const onChapterPick = (e: Event): void => {
    const v = (e.currentTarget as HTMLSelectElement).value;
    setChapterIdx(parseInt(v, 10));
  };
  // Load a catalogue entry (a row in the results list).
  const onBookPick = (id: number): void => {
    setBookId(id);
  };

  const book = createMemo(() => bookResource());
  const chapter = createMemo<Chapter | undefined>(() => {
    const b = book();
    if (!b) return undefined;
    return b.chapters[chapterIdx()];
  });

  // Debug hook: register the reader as a `__torille` panel so
  // the puppeteer probe can use the same `setText` /
  // `onRenderDone` API as the regular TextPanels. Without
  // this the probe would have to special-case the reader
  // (different selectors, different timing).
  if (typeof window !== 'undefined') {
    const w = window as any;
    if (!w.__torille) {
      w.__torille = {};
      w.__torille.panels = [];
    }
    const entry = {
      candidates,
      demoted,
      misspellings,
      grammarDiagnostics,
      styleDiagnostics,
      grammarProgress,
      grammarBatchTimes: () => grammarBatchTimes,
      heading: () => 'Project Gutenberg',
      setText: (next: string): void => onText(next),
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
      // Hard reset -- drops the wasm per-textbox state
      // and the underlay's misspell list. Useful when
      // the user reloads a book or after a deep probe
      // run so the next render starts from scratch.
      clearState: (): void => {
        const idx = stateIdx();
        if (idx >= 0) {
          destroyState(idx);
          void createState().then((newIdx) => setStateIdx(newIdx));
        }
        setText('');
        setDebouncedText('');
        setCandidates([]);
        setDemoted(new Set<string>());
        setFocused(null);
        setGrammarProgress(null);
        grammarBatchTimes.length = 0;
      },
      onRenderDone: (cb: () => void): (() => void) => {
        renderDoneSubscribers.add(cb);
        return () => {
          renderDoneSubscribers.delete(cb);
        };
      },
    };
    w.__torille.panels.push(entry);
    onCleanup(() => {
      const arr = w.__torille?.panels;
      if (arr) {
        const i = arr.indexOf(entry);
        if (i >= 0) arr.splice(i, 1);
      }
    });
  }

  return (
    <div class="gutenberg-reader">
      <h2>
        Project Gutenberg
        <Show when={book()}>
          {' — '}
          <em>{book()!.title}</em>
          {' '}
          <small>{book()!.author}</small>
        </Show>
      </h2>
      <p class="subheading">
        Editable chapter-by-chapter reader. Long chapters are split into
        pages; the chapter menu lists each page. Spell-check + click-to-suggest
        work as in the other panels.
      </p>

      <section class="book-picker" aria-label="Kirjavalinta">
        <div class="picker-head">
          <div>
            <h3>Kirjasto</h3>
            <p class="picker-hint">
              {catalog().length.toLocaleString('fi')} vapaasti luettavaa
              kirjaa. Hae nimellä tai kirjailijalla.
            </p>
          </div>
          <button
            type="button"
            class="picker-random"
            onClick={pickRandom}
            disabled={sortedResults().length === 0}
            title="Valitse satunnainen kirja nykyisistä tuloksista"
          >
            ✦ Satunnainen
          </button>
        </div>

        <div class="picker-controls">
          <label class="search-field">
            <span class="search-icon" aria-hidden="true">⌕</span>
            <input
              type="search"
              value={filterQuery()}
              placeholder="Hae kirjaa tai kirjailijaa…"
              aria-label="Hae kirjaa tai kirjailijaa"
              onInput={(e) => setFilterQuery(e.currentTarget.value)}
            />
            <Show when={filterQuery().length > 0}>
              <button
                type="button"
                class="search-clear"
                aria-label="Tyhjennä haku"
                title="Tyhjennä haku"
                onClick={clearFilter}
              >
                ×
              </button>
            </Show>
          </label>
          <label class="sort-field">
            <span>Järjestä</span>
            <select
              value={sortBy()}
              onChange={(e) => setSortBy(e.currentTarget.value as SortKey)}
            >
              <option value="title">Nimen mukaan</option>
              <option value="author">Kirjailijan mukaan</option>
              <option value="id">Luettelon järjestys</option>
            </select>
          </label>
        </div>

        <div class="catalog-meta">
          <span>
            <strong>{sortedResults().length.toLocaleString('fi')}</strong>{' '}
            {filterQuery().length > 0 ? 'osumaa' : 'kirjaa'}
            <Show when={filterQuery().length > 0}>
              {' '}haulle “{filterQuery()}”
            </Show>
          </span>
          <span class="catalog-page-label">
            {page()} / {pageCount()}
          </span>
        </div>

        <Show when={catalogResource.loading}>
          <div class="catalog-loading" aria-live="polite">
            <span class="spinner" aria-hidden="true" />
            Ladataan luetteloa…
          </div>
        </Show>
        <Show when={catalogResource.error}>
          <p class="error">
            Luettelon lataus epäonnistui: {String(catalogResource.error)}
          </p>
        </Show>

        <Show when={!catalogResource.loading && sortedResults().length > 0}>
          <ul class="catalog-results" role="list">
            {pageResults().map((c) => (
              <li>
                <button
                  type="button"
                  class={
                    c.id === bookId()
                      ? 'catalog-result active'
                      : 'catalog-result'
                  }
                  aria-pressed={c.id === bookId()}
                  title={c.title}
                  onClick={() => onBookPick(c.id)}
                >
                  <span class="r-title">{c.title}</span>
                  <span class="r-meta">
                    <span class="r-author">
                      {c.author.length > 0 ? c.author : 'Tuntematon'}
                    </span>
                    <span class="r-id">#{c.id}</span>
                  </span>
                  <Show when={c.id === bookId()}>
                    <span class="r-status">Valittu</span>
                  </Show>
                </button>
              </li>
            ))}
          </ul>
        </Show>

        <Show when={!catalogResource.loading && sortedResults().length === 0}>
          <div class="catalog-empty">
            <p>Ei tuloksia haulle “{filterQuery()}”.</p>
            <button type="button" class="picker-clear" onClick={clearFilter}>
              Tyhjennä haku
            </button>
          </div>
        </Show>

        <Show when={sortedResults().length > PAGE_SIZE}>
          <nav class="catalog-pagination" aria-label="Kirjaluettelon sivut">
            <button
              type="button"
              onClick={() => setPage(1)}
              disabled={page() === 1}
              aria-label="Ensimmäinen sivu"
              title="Ensimmäinen sivu"
            >
              «
            </button>
            <button
              type="button"
              onClick={() => setPage((p) => Math.max(1, p - 1))}
              disabled={page() === 1}
            >
              Edellinen
            </button>
            <span class="page-indicator">
              Sivu <strong>{page()}</strong> / {pageCount()}
            </span>
            <button
              type="button"
              onClick={() => setPage((p) => Math.min(pageCount(), p + 1))}
              disabled={page() === pageCount()}
            >
              Seuraava
            </button>
            <button
              type="button"
              onClick={() => setPage(pageCount())}
              disabled={page() === pageCount()}
              aria-label="Viimeinen sivu"
              title="Viimeinen sivu"
            >
              »
            </button>
          </nav>
        </Show>
      </section>

      <Show when={bookResource.loading}>
        <span class="loading">Ladataan kirjaa…</span>
      </Show>
      <Show when={bookResource.error}>
        <p class="error">
          Lataus epäonnistui: {String(bookResource.error)}
        </p>
      </Show>

      <Show when={book()}>
        <div class="gutenberg-nav">
          <button
            type="button"
            onClick={goPrev}
            disabled={chapterIdx() === 0}
          >
            ← Edellinen
          </button>
          <label class="chapter-pick">
            Luku{' '}
            <select value={chapterIdx()} onChange={onChapterPick}>
              {book()!.chapters.map((c, i) => (
                <option value={i}>
                  {i + 1}. {c.heading}
                </option>
              ))}
            </select>
            {' '}/ {book()!.chapters.length}
          </label>
          <button
            type="button"
            onClick={goNext}
            disabled={chapterIdx() >= book()!.chapters.length - 1}
          >
            Seuraava →
          </button>
        </div>
      </Show>

      <Show when={chapter()}>
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
            <span>{grammarDiagnostics().length} grammar</span>
          </Show>
          <Show when={builtInStyleCheckEnabled()}>
            <span>{styleDiagnostics().length} style</span>
          </Show>
          <Show when={grammarProgress()}>
            {(progress) => (
              <span class="checking">
                <span class="status-dot" aria-hidden="true" />
                tarkistetaan… {progress().done}/{progress().total}
              </span>
            )}
          </Show>
        </div>
        <Editor
          text={text()}
          onText={onText}
          misspellings={misspellings()}
          grammarDiagnostics={grammarDiagnostics()}
          styleDiagnostics={styleDiagnostics()}
          focused={focused()}
          stateIdx={stateIdx()}
          onFocus={(w) => setFocused(w)}
          onClose={() => setFocused(null)}
        />
      </Show>
    </div>
  );
}
