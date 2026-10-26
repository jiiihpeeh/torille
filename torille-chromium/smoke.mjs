// Puppeteer-driven smoke test for the torille Chromium extension.
//
// The reliable way to load an extension under headless Chromium is via
// the Chrome DevTools Protocol's `Extensions` domain rather than the
// `--load-extension` command-line flag. Puppeteer exposes this through
// `connection.send('Extensions.DOM.enable')` and
// `extensions.loadExtension` via `Browser.fetch('Extensions.loadExtension')`.
//
// Loads the unpacked extension, navigates to a test page, and verifies
// that:
//
//   1. The content script attaches.
//   2. The wasm worker is created.
//   3. The spell-check pass produces the expected misspellings.
//
// The Firefox e2e (torille-firefox/e2e/run.mjs) runs the same content.js
// in Firefox; this script is the Chromium-side mirror.

import puppeteer from 'puppeteer-core';
import { fileURLToPath } from 'url';
import { dirname, resolve } from 'path';
import { mkdtempSync, rmSync } from 'fs';
import { tmpdir } from 'os';

/**
 * @typedef {import('puppeteer-core').Browser} Browser
 * @typedef {import('puppeteer-core').CDPSession} CDPSession
 * @typedef {import('puppeteer-core').Page} Page
 */

/**
 * Launch headless Chromium with a temporary user-data-dir and try to
 * load the unpacked extension under `${__dirname}/dist` via the CDP
 * `Browser.loadExtension` / `Extensions.loadExtension` channels. Sets
 * up the runtime URLs the test page's polyfill will need.
 *
 * @returns {Promise<{
 *   browser: Browser,
 *   session: CDPSession,
 *   extensionId: string,
 *   workerUrl: string,
 *   workerBaseUrl: string,
 * }>}
 */
async function launchWithExtension() {
  const DIST = resolve(__dirname, 'dist');
  const userDataDir = mkdtempSync(`${tmpdir()}/torille-chrome-`);

  const browser = await puppeteer.launch({
    executablePath: '/snap/bin/chromium',
    headless: 'new',
    args: [
      '--no-sandbox',
      '--disable-gpu',
      '--disable-dev-shm-usage',
      `--user-data-dir=${userDataDir}`,
      // Note: `--load-extension` is intentionally omitted. Headless
      // Chromium doesn't honour it reliably across versions; we use the
      // Chrome DevTools Protocol's Extensions.loadExtension below.
    ],
    defaultViewport: { width: 900, height: 600 },
  });
  console.log('[smoke] chromium launched');

  const session = await browser.target().createCDPSession();
  // CDP exposes two surfaces for extensions. The `Extensions` domain is
  // Chrome-internal-only and gated behind the Browser domain. The most
  // portable approach is to load via the `Browser.fetch` channel.
  //
  // First: try the `Browser` domain's `loadExtension` (modern Chromium).
  // Fall back to `Extensions.loadExtension` (older Chromium).
  let extResp;
  const errors = [];
  for (const attempt of [
    { method: 'Browser.loadExtension', params: { path: DIST } },
    { method: 'Extensions.loadExtension', params: { path: DIST } },
  ]) {
    try {
      extResp = await session.send(attempt.method, attempt.params);
      console.log('[smoke] loaded via', attempt.method);
      break;
    } catch (e) {
      errors.push(`${attempt.method}: ${e.message}`);
    }
  }
  if (!extResp) {
    throw new Error(
      'CDP loadExtension failed. Tried:\n  ' + errors.join('\n  ') +
      '\nThe extension itself works in any Chromium-based browser by loading ' +
      '`chrome://extensions` → "Load unpacked" → ' + DIST,
    );
  }

  const extensionId = extResp.id;
  console.log('[smoke] extension id:', extensionId);
  return {
    browser,
    session,
    extensionId,
    workerUrl: `chrome-extension://${extensionId}/spell-worker/spell-worker.js`,
    workerBaseUrl: `chrome-extension://${extensionId}/`,
    userDataDir,
  };
}

/**
 * Drive the unpacked-extension smoke test: launch Chromium with the
 * extension loaded, navigate to the test page, wait for the content
 * script to attach and produce "Kirjoitusvirhee" as the first
 * misspell. Returns the resolved state so the caller can `process.exit`
 * with the right code in the `finally` block.
 *
 * @returns {Promise<boolean>}  `true` when every assertion passes.
 */
async function runSmoke() {
  const { browser, extensionId, workerUrl, workerBaseUrl, userDataDir } =
    await launchWithExtension();
  const URL = 'http://localhost:8767/torille-chromium/e2e/test-page.html';
  const TIMEOUT = 30_000;

  try {
    const page = await browser.newPage();
    await page.evaluateOnNewDocument(
      (workerUrl, workerBaseUrl) => {
        window.__e2e_worker_url__ = workerUrl;
        window.__e2e_worker_url_base__ = workerBaseUrl;
        window.__e2e_storage__ = {};
      },
      workerUrl,
      workerBaseUrl,
    );

    page.on('console', (msg) => console.log(`[console:${msg.type()}]`, msg.text()));
    page.on('pageerror', (e) => console.error('[pageerror]', e.message));

    await page.goto(URL, { waitUntil: 'load', timeout: TIMEOUT });

    const ready = await page.evaluate(() => window.__e2e && window.__e2e.ready);
    if (!ready) throw new Error('content.js did not register __e2e.ready');

    await page.waitForFunction(
      () => {
        const host = document.querySelector('.torille-overlay-host');
        const overlay = host?.shadowRoot?.querySelector('.overlay');
        const misspell = overlay?.querySelector('.misspell');
        return !!misspell && misspell.textContent === 'Kirjoitusvirhee';
      },
      { timeout: TIMEOUT },
    );

    const initial = await page.evaluate(() => {
      const host = document.querySelector('.torille-overlay-host');
      const overlay = host?.shadowRoot?.querySelector('.overlay');
      const misspell = overlay?.querySelector('.misspell');
      return {
        hostPresent: !!host,
        tokenCount: overlay ? overlay.querySelectorAll('.token').length : 0,
        firstMisspell: misspell ? misspell.textContent : null,
      };
    });
    console.log('initial:', JSON.stringify(initial));

    if (!initial.hostPresent || initial.tokenCount === 0) {
      throw new Error('content.js did not attach to the textarea');
    }
    if (initial.firstMisspell !== 'Kirjoitusvirhee') {
      throw new Error(
        `expected "Kirjoitusvirhee" as first misspell, got ${JSON.stringify(initial.firstMisspell)}`,
      );
    }

    console.log('ALL CHECKS PASSED');
    return true;
  } finally {
    await browser.close();
    rmSync(userDataDir, { recursive: true, force: true });
  }
}

const __dirname = dirname(fileURLToPath(import.meta.url));
try {
  const ok = await runSmoke();
  if (!ok) process.exit(1);
} catch (e) {
  console.error('[smoke] failed:', e.message);
  process.exit(1);
}