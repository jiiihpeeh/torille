// e2e/polyfill.js -- the WebExtensions API stub the test page loads
// instead of the real Firefox APIs. Loaded by test-page.html.
//
// Exposes a `browser` global (and `window.browser`) that mimics the
// subset of the WebExtensions API the content script uses:
//
//   browser.runtime.getURL(path) -> "http://localhost:8766/<path>"
//   browser.runtime.sendMessage(msg) -> routes torille:check /
//     torille:suggest to a real Worker against
//     http://localhost:8766/dist/spell-worker/spell-worker.js, so the
//     content script's checks exercise the same code path as in
//     production. The dev server is the one we run with
//     `python3 -m http.server` rooted at extensions/firefox/.
//   browser.runtime.onMessage.addListener(fn) -> captures the
//     content-script's toggle-pending-input handler so the test runner
//     can invoke it directly via `window.__listeners`.
//   browser.storage.local.get(key) / .set(obj) -> in-memory map with
//     `storage.onChanged` fan-out.
//
// The `Worker` constructor is wrapped to log messages; the polyfill
// uses the original constructor to actually create the wasm worker.

(function () {
  /** Base URL where the dev server serves the extension files. */
  const EXT_BASE = 'http://localhost:8766';

  /**
   * Append a message to the on-page log element and echo to stdout.
   * @param {string} m
   */
  const log = (m) => {
    const logEl = document.getElementById('log');
    if (logEl) logEl.textContent += m + '\n';
    console.log('[e2e]', m);
  };

  // Mirror worker creation only for logging; the original Worker class
  // is captured before we shadow it.
  const OrigWorker = window.Worker;
  if (OrigWorker) {
    /**
     * Replacement for `new Worker(url, opts)` that wraps the original
     * constructor and adds a `message`/`error` listener that pushes
     * every payload into the on-page log. The returned Worker behaves
     * identically to the wrapped one.
     */
    window.Worker = function (url, opts) {
      log('worker created: ' + url);
      const w = new OrigWorker(url, opts);
      if (w && w.addEventListener) {
        w.addEventListener('message', (e) => {
          log('worker msg: ' + JSON.stringify(e.data).slice(0, 120));
          if (e.data && e.data.type === 'ready') window.__e2e.workerReady = true;
          if (e.data && e.data.type === 'init_failed') window.__e2e.workerError = e.data.error;
        });
        w.addEventListener('error', (e) =>
          log('worker error: ' + (e.message || JSON.stringify(e))));
      }
      return w;
    };
  }

  // Tiny WebExtensions runtime + storage shim. The runtime.sendMessage
  // stub routes `torille:check` and `torille:suggest` to a real
  // worker against the dev server, so the content script's checks
  // exercise the same code path as in production. The dev server
  // is the one we run with `python3 -m http.server` rooted at
  // extensions/firefox/.
  /** @type {Function[]} */
  const messageListeners = [];
  /** @type {Record<string, unknown>} */
  const storageStore = { blacklist: [] };
  /** @type {Function[]} */
  const storageListeners = [];
  /** Test-runner hook: invokes the polyfilled addListener. */
  window.__listeners = messageListeners;

  /**
   * Lazily-created singleton worker. Production boots the wasm worker
   * once per browser session (the background script hosts it for
   * every tab); the polyfill mirrors that by spawning one worker for
   * the lifetime of the page so the ~150 ms wasm boot cost is paid
   * once, not per render.
   * @type {Worker|null}
   */
  let polyfillWorker = null;
  /** @type {Promise<void>|null} */
  let polyfillWorkerReady = null;

  /**
   * Boot the singleton worker if needed, returning a Promise that
   * resolves when the wasm has signalled `{type: 'ready'}`.
   */
  function ensurePolyfillWorker() {
    if (polyfillWorkerReady) return polyfillWorkerReady;
    polyfillWorker = new OrigWorker(
      EXT_BASE + '/dist/spell-worker/spell-worker.js', { type: 'module' },
    );
    polyfillWorkerReady = new Promise((resolve, reject) => {
      polyfillWorker.addEventListener('message', function onReady(e) {
        if (e.data && e.data.type === 'ready') {
          polyfillWorker.removeEventListener('message', onReady);
          resolve();
        }
      });
      polyfillWorker.addEventListener('error', (e) =>
        reject(new Error(e.message || 'worker error')));
    });
    return polyfillWorkerReady;
  }

  /**
   * Forward a `check` / `suggest` / `check-batch` / `suggest-batch`
   * payload to the shared polyfill worker, returning the worker's
   * matching `{ id }` message.
   */
  async function dispatchSpell(payload) {
    await ensurePolyfillWorker();
    const w = polyfillWorker;
    const id = (w.__spellNextId = (w.__spellNextId || 0) + 1);
    return new Promise((resolve, reject) => {
      w.addEventListener('message', function once(e) {
        const msg = e.data;
        if (msg && msg.id === id) {
          w.removeEventListener('message', once);
          resolve(msg);
        }
      });
      w.addEventListener('error', function onErr(e) {
        w.removeEventListener('message', once);
        w.removeEventListener('error', onErr);
        reject(new Error(e.message || 'worker error'));
      });
      w.postMessage({ id, ...payload });
    });
  }

  /**
   * The WebExtensions-shaped API surface the content script talks to.
   * @type {{
   *   runtime: {
   *     getURL(path: string): string,
   *     onMessage: { addListener(fn: Function): void },
   *     sendMessage(msg: any): Promise<any>,
   *   },
   *   storage: {
   *     local: {
   *       get(key: string): Promise<Record<string, unknown>>,
   *       set(obj: Record<string, unknown>): Promise<void>,
   *     },
   *     onChanged: { addListener(fn: Function): void },
   *   },
   * }}
   */
  const api = {
    runtime: {
      getURL(path) { return EXT_BASE + '/' + path; },
      onMessage: { addListener(fn) { messageListeners.push(fn); } },
      async sendMessage(msg) {
        window.__e2e.lastSent = msg;
        if (msg && msg.type === 'torille:check') {
          try {
            const r = await dispatchSpell({ type: 'check', word: msg.word });
            return { ok: r && r.ok === true };
          } catch (e) {
            return { ok: null };
          }
        }
        if (msg && msg.type === 'torille:check-fast') {
          try {
            const r = await dispatchSpell({
              type: 'check-fast', idx: msg.idx, word: msg.word,
            });
            return { ok: r ? r.ok === true : null };
          } catch (e) {
            return { ok: null };
          }
        }
        if (msg && msg.type === 'torille:suggest') {
          try {
            const r = await dispatchSpell({ type: 'suggest', word: msg.word, max: msg.max });
            return { suggestions: (r && r.suggestions) || [] };
          } catch (e) {
            return { suggestions: [] };
          }
        }
        // Batch ops: the content script's render path uses these to
        // avoid N round-trips per render. We forward as a single
        // worker call and pass the response shape through. The worker
        // implementation lives next to the single-word handlers in
        // spell-worker.ts and is exercised by the polyfill the same
        // way.
        if (msg && msg.type === 'torille:check-batch') {
          try {
            const r = await dispatchSpell({
              type: 'check-batch', inputId: msg.inputId, words: msg.items,
            });
            return { inputId: msg.inputId, gen: msg.gen,
                   ok: (r && r.ok) || [] };
          } catch (e) {
            return { inputId: msg.inputId, gen: msg.gen, ok: [] };
          }
        }
        if (msg && msg.type === 'torille:suggest-batch') {
          try {
            const r = await dispatchSpell({
              type: 'suggest-batch', inputId: msg.inputId, items: msg.items,
            });
            return { inputId: msg.inputId, gen: msg.gen,
                   results: (r && r.results) || [] };
          } catch (e) {
            return { inputId: msg.inputId, gen: msg.gen, results: [] };
          }
        }
        // Per-textbox state (the textarea render path). `create-state`
        // and `analyze-text` have replies; update/destroy/clear are
        // fire-and-forget on the worker side, so they must not wait
        // for an id that never comes.
        if (msg && msg.type === 'torille:create-state') {
          try {
            const r = await dispatchSpell({ type: 'create-state' });
            return { idx: r && r.idx };
          } catch (e) {
            throw new Error('create-state failed: ' + e.message);
          }
        }
        if (msg && msg.type === 'torille:analyze-text') {
          try {
            const r = await dispatchSpell({
              type: 'analyze-text', idx: msg.idx, text: msg.text, max: msg.max,
            });
            return {
              inputId: msg.inputId,
              gen: msg.gen,
              hits: (r && r.hits) || [],
            };
          } catch (e) {
            return { inputId: msg.inputId, gen: msg.gen, hits: null };
          }
        }
        if (msg && msg.type === 'torille:suggest-with-state') {
          try {
            const r = await dispatchSpell({
              type: 'suggest-with-state', idx: msg.idx,
              word: msg.word, max: msg.max,
            });
            return { suggestions: (r && r.suggestions) || [] };
          } catch (e) {
            return { suggestions: [] };
          }
        }
        if (msg && (msg.type === 'torille:update-state' ||
                    msg.type === 'torille:destroy-state' ||
                    msg.type === 'torille:clear-state')) {
          // Fire-and-forget: post and return immediately.
          ensurePolyfillWorker().then(() => {
            const payload = msg.type === 'torille:update-state'
              ? { type: 'update-state', idx: msg.idx, text: msg.text }
              : { type: msg.type.replace('torille:', ''), idx: msg.idx };
            polyfillWorker.postMessage(payload);
          }).catch(() => { /* worker down; content script retries */ });
          return undefined;
        }
        return undefined;
      },
    },
    storage: {
      local: {
        get(key) {
          return Promise.resolve({ [key]: storageStore[key] });
        },
        set(obj) {
          const changes = {};
          for (const k of Object.keys(obj)) {
            changes[k] = { oldValue: storageStore[k], newValue: obj[k] };
            storageStore[k] = obj[k];
          }
          for (const l of storageListeners) l(changes, 'local');
          return Promise.resolve();
        },
      },
      onChanged: { addListener(fn) { storageListeners.push(fn); } },
    },
  };
  window.browser = api;
  // Content scripts in Firefox see `browser` as a global (it's not on
  // `window`), so we expose it that way too.
  // eslint-disable-next-line no-undef
  browser = api;

  /** Test-runner introspection: flags + the most recent sent message. */
  window.__e2e = {
    ready: false,
    workerReady: false,
    workerError: null,
    contextmenuCaptured: null,
    lastSent: null,
  };

  // content.js is loaded directly via <script src="../dist/content/content.js">
  // in test-page.html. The polyfill only sets up the WebExtensions
  // shim so the content script's `browser.runtime` / `browser.storage`
  // calls succeed.
})();