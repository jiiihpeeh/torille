// Test-page bootstrap: marks the page ready once content.js has run
// and prints a one-line confirmation to the on-page log so the run.mjs
// screen-shot has something visible to diff against.
//
// Loaded after polyfill.js + the compiled content.js via
// <script src="./after-content.js"></script>; takes no arguments
// and exports nothing. The only side effects are:
//   * window.__e2e.ready = true   -- polled by the test runner
//     via page.waitForFunction().
//   * document.getElementById('log').textContent gets the line
//     "content.js loaded" appended for visual diffing.
'use strict';
window.__e2e.ready = true;
const logEl = document.getElementById('log');
if (logEl) logEl.textContent += 'content.js loaded\n';