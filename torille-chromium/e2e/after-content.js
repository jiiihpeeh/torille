// Test-page bootstrap: marks the page ready once content.js has run
// and prints a one-line confirmation to the on-page log so the smoke
// script's page screenshot has something visible to diff against.
//
// Loaded after polyfill.js + the compiled content.js via
// <script src="./after-content.js"></script>; takes no arguments
// and exports nothing. The only side effects are:
//   * window.__e2e.ready = true   -- polled by the smoke script
//     via page.evaluate().
//   * window.__msgs / window.__msg_listeners: empty arrays the
//     smoke script fills when routing in-channel mocks.
//   * document.getElementById('log').textContent gets the line
//     "content.js loaded" appended for visual diffing.
'use strict';
window.__e2e = window.__e2e || {};
window.__e2e.ready = true;
const logEl = document.getElementById('log');
if (logEl) logEl.textContent += 'content.js loaded\n';
window.__msgs = window.__msgs || [];
window.__msg_listeners = window.__msg_listeners || [];