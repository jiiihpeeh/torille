// End-to-end test for the torille Firefox content-script extension.
//
// Loads a stub HTML page (e2e/test-page.html) that polyfills
// `browser.runtime.getURL` to point at the real extension files
// served by the http.server on :8766. The content script runs in
// that page exactly as it would inside a Firefox tab, except the
// polyfilled API and the localhost wasm fetch instead of
// moz-extension:// fetches.
//
// We then drive the textarea via DOM events and read the shadow-
// root overlay's misspell elements.

import puppeteer from 'puppeteer-core';

/**
 * @typedef {import('puppeteer-core').Browser} Browser
 * @typedef {import('puppeteer-core').Page} Page
 */

/** URL served by `python3 -m http.server 8766` rooted at extensions/firefox/. */
const PAGE_URL = 'http://localhost:8766/e2e/test-page.html';

/**
 * @typedef {Object} InitialState
 * @property {boolean} hostFound          True if content.js wrote the overlay host.
 * @property {number}  tokenCount         Number of word tokens in the shadow overlay.
 * @property {string[]} misspellTexts    Texts of every misspell span on first render.
 * @property {string|null} spellcheck     `spellcheck` attribute on the textarea.
 * @property {string|null} hostDisplay    Inline style.display on the overlay host.
 */

/**
 * @typedef {Object} AfterTypeState
 * @property {number}    tokenCount
 * @property {string[]}  misspellTexts
 */

/**
 * @typedef {Object} AfterClickState
 * @property {boolean}   popupVisible    `true` if the suggestion popup is open.
 * @property {string[]}  suggestions     Texts of every suggestion button.
 */

/**
 * @typedef {Object} ToggleEffect
 * @property {boolean}                  ok         `false` if no listener registered.
 * @property {string}                  [reason]   Why the toggle didn't run, when ok=false.
 * @property {{disabled: boolean}|undefined} reply  Content-script reply payload.
 * @property {string|null}             hostDisplay
 */

/**
 * @typedef {Object} BlacklistState
 * @property {string|null}  hostDisplay
 * @property {number}       overlayHasContent  Number of children in the shadow overlay.
 */

/**
 * @typedef {Object} ContentEditableState
 * @property {boolean}      ceAttached         A contenteditable was found.
 * @property {boolean}      popupVisible
 * @property {string[]}     suggestions
 * @property {string|null}  ceContent          textContent of the contenteditable.
 */

/**
 * @typedef {Object} ContentEditableAfter
 * @property {string|null}  ceContent
 * @property {boolean}      popupHidden
 */

const browser = await puppeteer.launch({
  executablePath: '/usr/bin/google-chrome',
  headless: 'new',
  args: ['--no-sandbox', '--disable-gpu', '--disable-dev-shm-usage'],
});
try {
  const page = await browser.newPage();
  page.on('console', (msg) =>
    console.log(`[console:${msg.type()}] ${msg.text()}`));
  page.on('pageerror', (err) => console.error('[pageerror]', err.message));

  await page.goto(PAGE_URL, { waitUntil: 'domcontentloaded' });
  await page.waitForFunction(() => window.__e2e?.ready === true, { timeout: 5000 });
  // Wait for the initial render to land -- check that the overlay
  // matches the textarea's text content char-for-char. The render is
  // async (wasm checkAsync roundtrip), and we don't want the test to
  // capture state mid-render.
  await page.waitForFunction(() => {
    const ta = document.querySelector('textarea');
    const overlay = document.querySelector('.torille-overlay-host')
      ?.shadowRoot?.querySelector('.overlay');
    return ta && overlay && overlay.textContent === ta.value;
  }, { timeout: 5000 });
  await new Promise((r) => setTimeout(r, 200));

  /** @type {InitialState} */
  const initial = await page.evaluate(() => {
    const host = document.querySelector('.torille-overlay-host');
    const overlay = host?.shadowRoot?.querySelector('.overlay');
    const misspells = overlay ? Array.from(overlay.querySelectorAll('.misspell')) : [];
    return {
      hostFound: !!host,
      tokenCount: overlay ? overlay.querySelectorAll('.token, .misspell').length : 0,
      misspellTexts: misspells.map(s => s.textContent),
      spellcheck: document.querySelector('textarea')?.getAttribute('spellcheck'),
      hostDisplay: host ? host.style.display : null,
    };
  });
  console.log('initial:', JSON.stringify(initial));

  // Type a known-misspelling phrase.
  await page.evaluate(() => {
    const ta = document.querySelector('textarea');
    ta.value = 'talo taloo koirra xyzzy xyzzy';
    ta.dispatchEvent(new Event('input', { bubbles: true }));
  });
  // Wait for the overlay to actually pick up the new text instead of
  // racing the debounce + worker roundtrip. We compare textContent so
  // we don't get a false-positive from the initial render's region
  // count being >= 5.
  const TYPED = 'talo taloo koirra xyzzy xyzzy';
  await page.waitForFunction((expected) => {
    const overlay = document.querySelector('.torille-overlay-host')
      ?.shadowRoot?.querySelector('.overlay');
    return overlay && overlay.textContent === expected;
  }, { timeout: 3000 }, TYPED);

  /** @type {AfterTypeState} */
  const afterType = await page.evaluate(() => {
    const host = document.querySelector('.torille-overlay-host');
    const overlay = host?.shadowRoot?.querySelector('.overlay');
    const misspells = overlay ? Array.from(overlay.querySelectorAll('.misspell')) : [];
    return {
      tokenCount: overlay ? overlay.querySelectorAll('.token, .misspell').length : 0,
      misspellTexts: misspells.map(s => s.textContent),
    };
  });
  console.log('after typing:', JSON.stringify(afterType));

  // ----- non-ASCII offsets ------------------------------------------------
  //
  // The wasm reports UTF-8 *byte* ranges; the content script must turn
  // them into UTF-16 string indices. ASCII hides that conversion
  // completely (one byte = one code unit), so a broken conversion
  // still produces the same span text. `äiti` makes the byte and
  // UTF-16 offsets diverge by one; if the fix regresses, the span
  // under `taloo` shifts left and reads e.g. "aloo".
  const NON_ASCII = 'äiti taloo';
  await page.evaluate((value) => {
    const ta = document.querySelector('textarea');
    ta.value = value;
    ta.dispatchEvent(new Event('input', { bubbles: true }));
  }, NON_ASCII);
  await page.waitForFunction((expected) => {
    const overlay = document.querySelector('.torille-overlay-host')
      ?.shadowRoot?.querySelector('.overlay');
    return overlay && overlay.textContent === expected;
  }, { timeout: 3000 }, NON_ASCII);

  const nonAscii = await page.evaluate(() => {
    const host = document.querySelector('.torille-overlay-host');
    const overlay = host?.shadowRoot?.querySelector('.overlay');
    const misspells = overlay ? Array.from(overlay.querySelectorAll('.misspell')) : [];
    return {
      misspellTexts: misspells.map(s => s.textContent),
      overlayText: overlay ? overlay.textContent : null,
    };
  });
  console.log('non-ascii:', JSON.stringify(nonAscii));
  if (!nonAscii.misspellTexts.includes('taloo')) {
    throw new Error(
      'non-ASCII offset shift: expected misspell span "taloo", got ' +
      JSON.stringify(nonAscii.misspellTexts));
  }
  if (nonAscii.overlayText !== NON_ASCII) {
    throw new Error(
      'overlay text drifted: ' + JSON.stringify(nonAscii.overlayText));
  }

  // Click the first misspelling to verify the suggestion flow.
  await page.evaluate(() => {
    const host = document.querySelector('.torille-overlay-host');
    const overlay = host?.shadowRoot?.querySelector('.overlay');
    const misspell = overlay?.querySelector('.misspell');
    if (misspell) {
      const r = misspell.getBoundingClientRect();
      misspell.dispatchEvent(new MouseEvent('mousedown', {
        bubbles: true,
        clientX: r.left + r.width / 2,
        clientY: r.top + r.height / 2,
      }));
    }
  });
  await new Promise((r) => setTimeout(r, 600));

  /** @type {AfterClickState} */
  const afterClick = await page.evaluate(() => {
    const host = document.querySelector('.torille-overlay-host');
    const popup = host?.shadowRoot?.querySelector('.suggestions');
    const buttons = popup ? Array.from(popup.querySelectorAll('button')) : [];
    return {
      popupVisible: popup && popup.style.display !== 'none',
      suggestions: buttons.map(b => b.textContent),
    };
  });
  console.log('after click:', JSON.stringify(afterClick));

  // Close the popup by clicking outside it.
  await page.evaluate(() => {
    document.body.dispatchEvent(new MouseEvent('mousedown', { bubbles: true }));
  });
  await new Promise((r) => setTimeout(r, 200));

  // ----- Context-menu "Älä tarkasta tätä tekstiä" -----
  //
  // We can't actually click the Firefox context menu from puppeteer,
  // but we can simulate the round-trip:
  //   1. dispatch a `contextmenu` event on the textarea;
  //      the content script captures the input and stores it as
  //      `pendingInput`.
  //   2. send the same `torille:toggle-pending-input` message
  //      the background would send on contextMenus.onClicked.
  //   3. confirm the overlay is hidden.

  // First: open the popup again to verify it survives a toggle.
  await page.evaluate(() => {
    const ta = document.querySelector('textarea');
    ta.value = 'talo taloo koirra';
    ta.dispatchEvent(new Event('input', { bubbles: true }));
  });
  await new Promise((r) => setTimeout(r, 600));

  // Simulate right-click: dispatch a contextmenu event on the textarea.
  await page.evaluate(() => {
    const ta = document.querySelector('textarea');
    const r = ta.getBoundingClientRect();
    ta.dispatchEvent(new MouseEvent('contextmenu', {
      bubbles: true,
      cancelable: true,
      clientX: r.left + r.width / 2,
      clientY: r.top + r.height / 2,
    }));
  });

  // Now send the toggle message (as the background would on click).
  const toggleResult = await page.evaluate(async () => {
    // Re-import the message listener -- it was registered inside the
    // IIFE so we can't access it directly. Instead we use the
    // content script's reply: we simulate the round-trip via the
    // `runtime.sendMessage` shim by capturing its arguments.
    const calls = [];
    const orig = window.browser.runtime.sendMessage;
    window.browser.runtime.sendMessage = (m) => {
      calls.push(m);
      return orig(m);
    };
    return calls;
  });
  console.log('contextmenu calls:', JSON.stringify(toggleResult));

  // The content script should have sent a torille:contextmenu-fired
  // message via api.runtime.sendMessage. The test page's polyfill
  // stashes the most recent call in `window.__e2e.lastSent`.
  const ctxMenuState = await page.evaluate(() => ({
    lastSent: window.__e2e?.lastSent,
  }));
  console.log('after right-click:', JSON.stringify(ctxMenuState));

  // Drive the toggle via the runtime message round-trip by hand:
  // the polyfill's `messageListeners` array carries the content
  // script's handler, so we can invoke it directly.
  const toggleReply = await page.evaluate(async () => {
    // Look up the listener that handles the toggle message; any
    // listener that returns a promise with `disabled` qualifies.
    // The polyfill doesn't expose the array, so we instead call
    // sendMessage with the toggle type and wait for the content
    // script's reply via a captured promise. Cheaper path: the
    // content script's listener returns { disabled: ... } which
    // we can observe by reading `window.__e2e.lastSent`.
    // For full coverage, simulate the background's reply by
    // calling the listener manually.
    return window.__e2e?.lastSent;
  });
  console.log('toggle state:', JSON.stringify(toggleReply));

  // To actually exercise the toggle, we re-dispatch the toggle
  // message into the listener via the polyfill. The polyfill
  // currently captures `addListener` but doesn't replay messages.
  // We add a helper to the polyfill on the fly.
  /** @type {ToggleEffect} */
  const toggleEffect = await page.evaluate(async () => {
    // The content script's listener was registered with
    // `api.runtime.onMessage.addListener(fn)`. Reach into the
    // polyfill's listener array via the test page's namespace.
    // We patched the polyfill to expose it under `__listeners`.
    if (!window.__listeners || !window.__listeners.length) {
      return { ok: false, reason: 'no listeners registered' };
    }
    const fn = window.__listeners.find(
      (l) => l && typeof l === 'function',
    );
    if (!fn) return { ok: false, reason: 'no fn listener' };
    const reply = await fn({ type: 'torille:toggle-pending-input' }, { id: 1 });
    const host = document.querySelector('.torille-overlay-host');
    return {
      ok: true,
      reply,
      hostDisplay: host ? host.style.display : null,
      // The page never set spellcheck, so hiding the overlay must
      // restore "absent", not leave "false" behind.
      spellcheck: document.querySelector('textarea')?.getAttribute('spellcheck') ?? null,
    };
  });
  console.log('toggle effect:', JSON.stringify(toggleEffect));

  // Verify the overlay is now hidden.
  if (!toggleEffect.ok) throw new Error('toggle did not run: ' + toggleEffect.reason);
  if (toggleEffect.hostDisplay !== 'none') throw new Error('overlay should be hidden after toggle');
  if (toggleEffect.reply?.disabled !== true) throw new Error('reply.disabled should be true');
  if (toggleEffect.spellcheck !== null) {
    throw new Error('native spellcheck should be restored after toggle, got ' +
                    JSON.stringify(toggleEffect.spellcheck));
  }

  // Toggle again to verify round-trip both ways.
  const toggleAgain = await page.evaluate(async () => {
    const fn = window.__listeners.find((l) => typeof l === 'function');
    const reply = await fn({ type: 'torille:toggle-pending-input' }, { id: 1 });
    const host = document.querySelector('.torille-overlay-host');
    return {
      reply,
      hostDisplay: host ? host.style.display : null,
      spellcheck: document.querySelector('textarea')?.getAttribute('spellcheck') ?? null,
    };
  });
  if (toggleAgain.reply?.disabled !== false) throw new Error('reply.disabled should be false after second toggle');
  if (toggleAgain.hostDisplay === 'none') throw new Error('overlay should be visible after second toggle');
  if (toggleAgain.spellcheck !== 'false') {
    throw new Error('native spellcheck should be disabled again, got ' +
                    JSON.stringify(toggleAgain.spellcheck));
  }

  // ----- Per-host blacklist -------------------------------------------
  //
  // We can't click Firefox's context-menu from puppeteer, but we can
  // drive the same flow by writing to storage.local directly. The
  // content script listens for storage.onChanged and re-evaluates.

  // 1. Type fresh content so the overlay has something to hide.
  await page.evaluate(() => {
    const ta = document.querySelector('textarea');
    ta.value = 'talo taloo koirra';
    ta.dispatchEvent(new Event('input', { bubbles: true }));
  });
  await new Promise((r) => setTimeout(r, 600));

  // 2. Add the current hostname to the blacklist via the storage shim.
  const hostname = await page.evaluate(() => location.hostname);
  await page.evaluate((host) => {
    return browser.storage.local.set({ blacklist: [host] });
  }, hostname);
  await new Promise((r) => setTimeout(r, 400));

  /** @type {BlacklistState} */
  const blacklisted = await page.evaluate(() => {
    const host = document.querySelector('.torille-overlay-host');
    const overlay = host?.shadowRoot?.querySelector('.overlay');
    return {
      hostDisplay: host ? host.style.display : null,
      overlayHasContent: overlay ? overlay.children.length : 0,
      spellcheck: document.querySelector('textarea')?.getAttribute('spellcheck') ?? null,
    };
  });
  console.log('after blacklist:', JSON.stringify(blacklisted));
  if (blacklisted.hostDisplay !== 'none') throw new Error('overlay should be hidden when host is blacklisted');
  if (blacklisted.spellcheck !== null) {
    throw new Error('native spellcheck should be restored on a blacklisted host, got ' +
                    JSON.stringify(blacklisted.spellcheck));
  }

  // 3. Remove from the blacklist. The overlay should re-attach.
  await page.evaluate(() => browser.storage.local.set({ blacklist: [] }));
  await new Promise((r) => setTimeout(r, 600));

  const restored = await page.evaluate(() => {
    const host = document.querySelector('.torille-overlay-host');
    const overlay = host?.shadowRoot?.querySelector('.overlay');
    const misspells = overlay ? Array.from(overlay.querySelectorAll('.misspell')) : [];
    return {
      hostFound: !!host,
      hostDisplay: host ? host.style.display : null,
      misspellTexts: misspells.map(s => s.textContent),
      spellcheck: document.querySelector('textarea')?.getAttribute('spellcheck') ?? null,
    };
  });
  console.log('after un-blacklist:', JSON.stringify(restored));
  if (!restored.hostFound) throw new Error('overlay should be re-attached after un-blacklist');
  if (restored.hostDisplay === 'none') throw new Error('overlay should be visible after un-blacklist');
  if (restored.spellcheck !== 'false') {
    throw new Error('native spellcheck should be disabled again after un-blacklist, got ' +
                    JSON.stringify(restored.spellcheck));
  }

  // ----- contenteditable cursor-based watcher -----------------------------
  //
  // The contenteditable path doesn't render per-word underlines (matching
  // the reference repo); instead it watches the caret and pops a
  // suggestion menu right below the misspelling. Drive the same flow
  // the user would: focus the element, place the caret at the end of
  // a misspelled word, dispatch input, click a suggestion, verify the
  // word was replaced.

  // 1. Focus the element, clear it, type fresh content.
  await page.evaluate(() => {
    const ce = document.querySelector('#ce');
    ce.focus();
    ce.textContent = 'talo taloo koirra';
    // Place the caret at the end of "taloo" (after the second "o").
    const sel = window.getSelection();
    const range = document.createRange();
    const textNode = ce.firstChild;
    // textNode is the entire text content; the misspelling "taloo"
    // sits at offset 5..10. Place the caret at 10 (end of "taloo").
    range.setStart(textNode, 10);
    range.setEnd(textNode, 10);
    sel.removeAllRanges();
    sel.addRange(range);
    ce.dispatchEvent(new InputEvent('input', { bubbles: true }));
  });
  // Wait for the popup to appear (debounced + wasm roundtrip).
  await page.waitForFunction(() => {
    const popups = document.querySelectorAll('.torille-ce-host');
    for (const p of popups) {
      if (p.style.display !== 'none' && p.children.length > 0) return true;
    }
    return false;
  }, { timeout: 3000 });

  /** @type {ContentEditableState} */
  const ceState = await page.evaluate(() => {
    const popup = document.querySelector('.torille-ce-host');
    const buttons = popup ? Array.from(popup.querySelectorAll('button')) : [];
    let highlightRangeCount = 0;
    try {
      const hl = CSS.highlights.get('torille-misspell');
      if (hl) {
        // Range-like objects: size is the property, but as a fallback
        // we can also iterate if Symbol.iterator is present.
        if (typeof hl.size === 'number') highlightRangeCount = hl.size;
        else if (typeof hl.priority === 'number') highlightRangeCount = 1;
      }
    } catch (_) {}
    return {
      ceAttached: document.querySelectorAll('[contenteditable]').length > 0,
      popupVisible: popup && popup.style.display !== 'none',
      suggestions: buttons.map(b => b.textContent),
      ceContent: document.querySelector('#ce').textContent,
      highlightRangeCount,
    };
  });
  console.log('contenteditable state:', JSON.stringify(ceState));
  if (!ceState.popupVisible) throw new Error('contenteditable popup did not open');
  if (ceState.suggestions.length < 1) throw new Error('no suggestions for contenteditable');
  // The popup should contain "talo" as the top suggestion for "taloo".
  if (!ceState.suggestions.includes('talo')) throw new Error('talo missing from contenteditable suggestions');
  // CSS Custom Highlight should be populated with the misspelling(s).
  if (!ceState.highlightRangeCount) throw new Error('CSS highlight for contenteditable misspellings is empty');

  // 2. Click the first suggestion and verify the word was replaced.
  await page.evaluate(() => {
    const popup = document.querySelector('.torille-ce-host');
    const btn = popup.querySelector('button');
    btn.dispatchEvent(new MouseEvent('mousedown', { bubbles: true }));
  });
  await new Promise((r) => setTimeout(r, 400));

  /** @type {ContentEditableAfter} */
  const ceAfter = await page.evaluate(() => ({
    ceContent: document.querySelector('#ce').textContent,
    popupHidden: document.querySelector('.torille-ce-host').style.display === 'none',
  }));
  console.log('contenteditable after click:', JSON.stringify(ceAfter));
  if (!ceAfter.ceContent.includes('talo ')) throw new Error('taloo was not replaced with talo');
  if (ceAfter.ceContent.includes('taloo')) throw new Error('taloo still present after replace');
  if (!ceAfter.popupHidden) throw new Error('popup should hide after a suggestion is clicked');

  await page.screenshot({ path: '/tmp/opencode/extension-e2e.png', fullPage: true });

  if (!initial.hostFound) throw new Error('overlay host not attached');
  if (initial.spellcheck !== 'false') throw new Error('browser spellcheck not disabled');
  if (initial.hostDisplay === 'none') throw new Error('overlay is hidden on first load');
  if (!afterType.misspellTexts.includes('taloo')) throw new Error('taloo not flagged');
  if (!afterType.misspellTexts.includes('koirra')) throw new Error('koirra not flagged');
  if (!afterType.misspellTexts.includes('xyzzy')) throw new Error('xyzzy not flagged');
  if (!afterClick.popupVisible) throw new Error('suggestion popup did not open');
  if (afterClick.suggestions.length < 1) throw new Error('no suggestions returned');
  if (ctxMenuState.lastSent?.type !== 'torille:contextmenu-fired') throw new Error('contextmenu did not push message');
  if (ctxMenuState.lastSent?.disabled !== false) throw new Error('first contextmenu should report disabled=false');

  console.log('\nALL CHECKS PASSED');
} finally {
  await browser.close();
}