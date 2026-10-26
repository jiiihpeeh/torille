// Headless smoke test for the demo in a real browser.
//
// Boots Firefox (or Chrome) via puppeteer-core, navigates to
// http://localhost:8765/, and reports:
//   * the first console error / page error that surfaces
//   * whether the worker is up (`postMessage({type: 'ready'}`)
//     round-trips back)
//   * whether a `check` call returns a sensible answer
//
// The current Firefox behaviour to verify is whether the
// static-import of the wasm via the shim
// (`import * as wasm from "./torille_wasm_bg.wasm"`) actually
// works, or if it needs a runtime `WebAssembly.compileStreaming`
// path because Firefox returns `NS_ERROR_CORRUPTED_CONTENT` for
// the ESM-wasm import.

import puppeteer from 'puppeteer-core';

const URL = process.env.URL || 'http://localhost:8765/';
const ENGINE = process.env.ENGINE || 'firefox';

(async () => {
  const launchOpts =
    ENGINE === 'firefox'
      ? { browser: 'firefox', executablePath: '/usr/bin/firefox', headless: true }
      : { executablePath: '/usr/bin/google-chrome', headless: 'new', args: ['--no-sandbox'] };

  console.log(`Launching ${ENGINE}...`);
  const browser = await puppeteer.launch(launchOpts);
  const page = await browser.newPage();

  const consoleLines = [];
  const pageErrors = [];
  const failedRequests = [];
  page.on('console', (msg) => {
    consoleLines.push(`[${msg.type()}] ${msg.text()}`);
  });
  page.on('pageerror', (err) => {
    pageErrors.push(err.message);
  });
  page.on('requestfailed', (req) => {
    failedRequests.push(`${req.url()} -- ${req.failure()?.errorText}`);
  });

  console.log(`Navigating to ${URL}`);
  // Cache-bust the navigation; the previous bundle (with the broken
  // ESM-wasm path) is cached on disk from earlier runs and Chrome
  // would reuse it for a `Worker` import in the same session.
  await page.setCacheEnabled(false);
  await page.goto(URL, { waitUntil: 'load', timeout: 15000 });

  // Give the worker time to spin up and the wasm to load.
  await new Promise((r) => setTimeout(r, 3000));

  // Wait for the demo's misspellings resource to settle. We want
  // to see "N misspellings" with N > 0 (the example text has
  // `taloo`, `koirra`, `xyzzy` as obvious typos), so the demo
  // is observably working end-to-end.
  console.log('Waiting for misspellings to settle...');
  await page.waitForFunction(
    () => {
      const el = document.querySelector('.status');
      if (!el) return false;
      const txt = el.textContent || '';
      const m = txt.match(/(\d+) misspelling/);
      return m && parseInt(m[1], 10) > 0;
    },
    { timeout: 15000 },
  ).catch((e) => {
    console.log('waitForFunction timed out or errored:', e.message);
  });

  // Try to read the demo's status element to see what it thinks happened.
  const demoState = await page.evaluate(() => {
    const app = document.getElementById('app');
    return {
      appText: app ? app.innerText : null,
      hasWorker: typeof Worker !== 'undefined',
    };
  });
  console.log('Demo state:', JSON.stringify(demoState));

  // Try a `check` round-trip via the demo's worker.
  const checkResult = await page.evaluate(async () => {
    // The demo doesn't expose a global hook; build one in-page by
    // asking the Solid app's worker via a probe message. The worker
    // posts { type: 'ready' } on init and { type: 'check', id, ok }
    // for check requests.
    return new Promise((resolve) => {
      // Find the worker. The Solid app doesn't expose it directly,
      // so we have to dig into the module graph. Simpler: just
      // see if `check` works on a known-good word by inspecting
      // the overlay markup after typing.
      setTimeout(() => {
        const ta = document.querySelector('textarea');
        if (ta) {
          ta.value = 'taloo';
          ta.dispatchEvent(new Event('input', { bubbles: true }));
          setTimeout(() => {
            const miss = document.querySelectorAll('.misspell');
            resolve({
              ta: !!ta,
              misspellCount: miss.length,
              misspellText: Array.from(miss).map((m) => m.textContent),
            });
          }, 1500);
        } else {
          resolve({ ta: false });
        }
      }, 200);
    });
  });
  console.log('Check result:', JSON.stringify(checkResult));

  console.log('\n=== CONSOLE ===');
  for (const line of consoleLines) console.log(line);
  console.log('\n=== PAGE ERRORS ===');
  for (const err of pageErrors) console.log(err);
  console.log('\n=== FAILED REQUESTS ===');
  for (const r of failedRequests) console.log(r);

  // Probe a fresh worker on the page to capture the real error
  // event. The demo only surfaces `e.message` (often the literal
  // string "undefined" for module-load errors); `e.error` carries
  // the real Error.
  console.log('\n=== WORKER ERROR PROBE ===');
  const workerErr = await page.evaluate(async () => {
    return await new Promise((resolve) => {
      const w = new Worker(new URL('/dist/spell-worker.js', location.origin), {
        type: 'module',
      });
      const t = setTimeout(() => {
        w.terminate();
        resolve({ kind: 'timeout' });
      }, 5000);
      w.addEventListener('message', (e) => {
        clearTimeout(t);
        w.terminate();
        resolve({ kind: 'message', data: e.data });
      });
      w.addEventListener('error', (e) => {
        clearTimeout(t);
        w.terminate();
        resolve({
          kind: 'error',
          message: e.message,
          errorMessage: e.error?.message,
          errorStack: e.error?.stack,
          filename: e.filename,
          lineno: e.lineno,
        });
      });
    });
  });
  console.log(JSON.stringify(workerErr, null, 2));

  await browser.close();
})().catch((e) => {
  console.error('Smoke test failed:', e);
  process.exit(1);
});
