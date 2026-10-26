// Verify the popup's slow path (suggestWithState) is memoized.
// Fires the same word twice; if the second call doesn't hit the
// wasm::find_bases_with path (i.e. shows a cache hit instead of
// a fresh wasm:suggest-with-state line), memoization works.
import puppeteer from 'puppeteer-core';

const browser = await puppeteer.launch({
  executablePath: '/usr/bin/google-chrome',
  headless: 'new',
  args: ['--no-sandbox', '--disable-dev-shm-usage'],
  protocolTimeout: 600000,
});
const page = await browser.newPage();
page.setDefaultTimeout(600000);

const consoleLines = [];
page.on('console', (m) => {
  const t = m.text();
  if (t.includes('cache:suggest-with-state') || t.includes('wasm:suggest-with-state')) {
    console.log('[browser]', t);
  }
});
page.on('pageerror', (e) => console.log('[pageerror]', e));

await page.goto('http://127.0.0.1:8765/', { waitUntil: 'load' });

const result = await page.evaluate(async () => {
  const w = new Worker(new URL('/dist/spell-worker.js', location.origin), {
    type: 'module',
  });
  await new Promise((resolve, reject) => {
    const t = setTimeout(() => reject(new Error('ready timeout')), 30000);
    function onMsg(e) {
      if (e.data?.type === 'ready') { clearTimeout(t); w.removeEventListener('message', onMsg); resolve(); }
    }
    w.addEventListener('message', onMsg);
    w.addEventListener('error', (e) => { clearTimeout(t); reject(new Error('worker error: ' + (e.message || 'undef'))); });
  });

  function call(payload, timeoutMs = 60000) {
    const id = Math.floor(Math.random() * 1000000);
    return new Promise((resolve) => {
      const t = setTimeout(() => resolve({ id, type: 'timeout', ms: timeoutMs }), timeoutMs);
      function onMsg(e) {
        if (e.data && e.data.id === id) {
          clearTimeout(t);
          w.removeEventListener('message', onMsg);
          resolve(e.data);
        }
      }
      w.addEventListener('message', onMsg);
      w.postMessage({ id, ...payload });
    });
  }

  // First, create the state and seed it with a proper noun so
  // the inflection-aware path actually has something to find.
  const cs = await call({ type: 'create-state' });
  const idx = cs.idx;
  await call({ type: 'update-state', idx, text: 'Helsinki on iso. Menin Helsinkiin eilen.' });

  const out = [];

  // Cold call -- should fire the wasm path (no log, but the
  // timing will be ~500ms+ for the full find_bases_with walk).
  const t0 = performance.now();
  const c1 = await call({ type: 'suggest-with-state', idx, word: 'helsnki', max: 5 });
  out.push(`cold call took ${(performance.now() - t0).toFixed(0)}ms -> ${c1.suggestions.length} suggestions`);

  // Hot call -- should hit the cache instantly (microseconds).
  const t1 = performance.now();
  const c2 = await call({ type: 'suggest-with-state', idx, word: 'Helsnki', max: 5 });
  out.push(`hot call took ${(performance.now() - t1).toFixed(1)}ms -> ${c2.suggestions.length} suggestions`);

  // A different word, same state -- fresh call.
  const t2 = performance.now();
  const c3 = await call({ type: 'suggest-with-state', idx, word: 'taloo', max: 5 });
  out.push(`different word call took ${(performance.now() - t2).toFixed(0)}ms -> ${c3.suggestions.length} suggestions`);

  // Same word again -- should hit cache.
  const t3 = performance.now();
  const c4 = await call({ type: 'suggest-with-state', idx, word: 'taloo', max: 5 });
  out.push(`same word again took ${(performance.now() - t3).toFixed(1)}ms -> ${c4.suggestions.length} suggestions`);

  // Sanity: update-state should invalidate the per-idx cache.
  const t4 = performance.now();
  await call({ type: 'update-state', idx, text: 'Helsinki on iso. Menin Espooseen eilen.' });
  const c5 = await call({ type: 'suggest-with-state', idx, word: 'helsnki', max: 5 });
  out.push(`after-update call took ${(performance.now() - t4).toFixed(0)}ms -> ${c5.suggestions.length} suggestions`);

  return out;
});

console.log('\n=== TEST RESULTS ===');
for (const l of result) console.log(l);

await browser.close();
