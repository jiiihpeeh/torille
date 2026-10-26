import puppeteer from 'puppeteer-core';
const browser = await puppeteer.launch({
  executablePath: '/usr/bin/google-chrome',
  headless: 'new',
  args: ['--no-sandbox', '--disable-dev-shm-usage'],
  protocolTimeout: 60000,
});
const page = await browser.newPage();
const lines = [];
page.on('console', (m) => lines.push(`[page] ${m.type()} ${m.text()}`));
page.on('pageerror', (e) => lines.push('[pageerror] ' + e));
await page.goto('http://127.0.0.1:8765/', { waitUntil: 'load' });
// Immediately inject a page-side log to confirm the listener works
await page.evaluate(() => {
  console.log('SANITY_PAGE_SIDE_LOG');
  console.error('SANITY_PAGE_SIDE_ERR');
  console.warn('SANITY_PAGE_SIDE_WARN');
});
await new Promise((r) => setTimeout(r, 4000));
for (const l of lines) console.log('  ' + l);
await browser.close();
