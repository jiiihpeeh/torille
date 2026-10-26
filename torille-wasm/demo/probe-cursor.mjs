// Verify the cursor (caret) in the textarea is vertically
// aligned with the corresponding text in the underlay.
//
// For each test case, the probe sets the textarea's value,
// moves the caret to the end, and measures the caret's
// position (via a mirror div positioned at the textarea's
// exact viewport coordinates) vs the underlay's text at
// the same character index. They should be within 2 px on
// both x and y.
//
// Run from this directory after `node build.mjs`:
//
//   node probe-cursor.mjs

import puppeteer from 'puppeteer-core';

const browser = await puppeteer.launch({
  executablePath: '/usr/bin/google-chrome',
  headless: 'new',
  args: ['--no-sandbox', '--disable-dev-shm-usage'],
  protocolTimeout: 600000,
});
const page = await browser.newPage();
page.setDefaultTimeout(600000);
page.on('pageerror', (e) => console.error('[pageerror]', e));

await page.goto('http://127.0.0.1:8765/', { waitUntil: 'load' });
await page.waitForSelector('.editor textarea', { timeout: 30000 });

// `setSelectionRange` accepts character offsets; we measure
// the caret via a mirror div positioned at the textarea's
// exact viewport coordinates so its range's rect is in
// the same coordinate space as the underlay's text rect.
const MEASURE_JS = `
  (() => {
    const ta = document.querySelector('.editor textarea');
    const underlay = document.querySelector('.editor .underlay');
    const text = ta.value;
    const idx = text.length;
    ta.focus();
    ta.setSelectionRange(idx, idx);
    const div = document.createElement('div');
    const taStyle = window.getComputedStyle(ta);
    const taRect0 = ta.getBoundingClientRect();
    for (const prop of [
      'fontFamily','fontSize','lineHeight','letterSpacing',
      'wordSpacing','tabSize','textIndent','padding','border',
      'boxSizing','whiteSpace','wordWrap','overflowWrap',
    ]) {
      div.style[prop] = taStyle[prop];
    }
    div.style.position = 'absolute';
    div.style.visibility = 'hidden';
    div.style.top = taRect0.top + 'px';
    div.style.left = taRect0.left + 'px';
    div.style.width = ta.clientWidth + 'px';
    div.style.margin = '0';
    div.textContent = text;
    document.body.appendChild(div);
    let remaining = idx;
    let range = null;
    const walker = document.createTreeWalker(div, NodeFilter.SHOW_TEXT);
    while (walker.nextNode()) {
      const node = walker.currentNode;
      if (remaining <= node.nodeValue.length) {
        range = document.createRange();
        range.setStart(node, remaining);
        range.setEnd(node, remaining);
        break;
      }
      remaining -= node.nodeValue.length;
    }
    if (!range) {
      document.body.removeChild(div);
      return { error: 'range not found' };
    }
    const caretRect = range.getBoundingClientRect();
    document.body.removeChild(div);
    remaining = idx;
    range = null;
    const uwalker = document.createTreeWalker(underlay, NodeFilter.SHOW_TEXT);
    while (uwalker.nextNode()) {
      const node = uwalker.currentNode;
      if (remaining <= node.nodeValue.length) {
        range = document.createRange();
        range.setStart(node, remaining);
        range.setEnd(node, remaining);
        break;
      }
      remaining -= node.nodeValue.length;
    }
    if (!range) return { error: 'underlay range not found' };
    const uRect = range.getBoundingClientRect();
    return {
      caret: { x: caretRect.x, y: caretRect.y, h: caretRect.height },
      underlay: { x: uRect.x, y: uRect.y, h: uRect.height },
      dx: Math.abs(caretRect.x - uRect.x),
      dy: Math.abs(caretRect.y - uRect.y),
      dh: Math.abs(caretRect.height - uRect.height),
    };
  })()
`;

const SET_TEXT_JS = (text) => `
  (() => {
    const ta = document.querySelector('.editor textarea');
    const proto = Object.getPrototypeOf(ta);
    const setter = Object.getOwnPropertyDescriptor(proto, 'value').set;
    setter.call(ta, ${JSON.stringify(text)});
    ta.dispatchEvent(new InputEvent('input', { bubbles: true }));
    ta.focus();
  })()
`;

const CASES = [
  { name: 'single-line end', text: 'Tämä on lyhyt.' },
  { name: 'two-line end',    text: 'Tämä on lyhyt.\nJa toinen rivi.' },
  { name: 'three-line end',  text: 'Tämä on lyhyt.\nJa toinen rivi.\nKolmas vielä.' },
];

let allOk = true;
for (const c of CASES) {
  await page.evaluate(SET_TEXT_JS(c.text));
  await new Promise((r) => setTimeout(r, 600));
  const m = await page.evaluate(MEASURE_JS);
  if (m.error) {
    console.log('*** ' + c.name + ' FAIL: ' + m.error + ' ***');
    allOk = false;
    continue;
  }
  const ok = m.dx <= 2 && m.dy <= 2 && m.dh <= 2;
  console.log(
    (ok ? '✓ ' : '✗ ') + c.name.padEnd(20) +
    '  dx=' + m.dx.toFixed(2).padStart(5) +
    ' dy=' + m.dy.toFixed(2).padStart(5) +
    ' dh=' + m.dh.toFixed(2).padStart(5) +
    '  caret=(' + m.caret.x.toFixed(0) + ',' + m.caret.y.toFixed(0) + ')' +
    '  underlay=(' + m.underlay.x.toFixed(0) + ',' + m.underlay.y.toFixed(0) + ')',
  );
  if (!ok) allOk = false;
}

if (!allOk) {
  console.log('\n*** FAIL: at least one case exceeds 2 px ***');
  process.exit(1);
} else {
  console.log('\n*** PASS: caret aligns with underlay text in every case ***');
}
await browser.close();
