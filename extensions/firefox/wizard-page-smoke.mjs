// Browser smoke test for the extension's "Omat sanat" page.
//
// Loads the compiled page from `dist/` under a plain HTTP server, stubs the
// extension APIs, and wires `runtime.sendMessage` to the real compiled spell
// core. The background hop is typechecked separately; this test exercises the
// page, the storage round-trip and the worker-core responses.
//
// Usage, from `extensions/firefox/` after `npm run build`:
//   node wizard-page-smoke.mjs

import { spawn } from 'node:child_process';
import puppeteer from 'puppeteer-core';

const PORT = 8770;
const PAGE = `http://127.0.0.1:${PORT}/options/wizard.html`;
const ROOT = new URL('.', import.meta.url).pathname + 'dist';

const server = spawn(
  'python3',
  ['-m', 'http.server', String(PORT), '--directory', ROOT],
  { stdio: 'ignore' },
);

const sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms));

let browser;
try {
  for (let attempt = 0; attempt < 100; attempt++) {
    try {
      const response = await fetch(PAGE);
      if (response.ok) break;
    } catch {
      /* still starting */
    }
    await sleep(100);
  }

  browser = await puppeteer.launch({
    executablePath: '/usr/bin/google-chrome',
    headless: 'new',
    args: ['--no-sandbox', '--disable-gpu', '--disable-dev-shm-usage'],
    defaultViewport: { width: 1000, height: 900 },
  });
  const page = await browser.newPage();
  const errors = [];
  page.on('pageerror', (error) => errors.push(String(error)));
  page.on('console', (message) => {
    if (message.type() === 'error') errors.push(message.text());
  });

  await page.evaluateOnNewDocument(() => {
    const store = {
      async get(key) {
        const value = localStorage.getItem(`ext:${key}`);
        return { [key]: value ?? undefined };
      },
      async set(values) {
        for (const [key, value] of Object.entries(values)) {
          localStorage.setItem(`ext:${key}`, String(value));
        }
      },
    };
    let corePromise = null;
    function core() {
      if (!corePromise) {
        corePromise = import('/spell-worker/spell-core.js').then(async (mod) => {
          const instance = mod.createSpellCore();
          await instance.init();
          return instance;
        });
      }
      return corePromise;
    }
    window.browser = {
      runtime: {
        async sendMessage(message) {
          const instance = await core();
          switch (message.type) {
            case 'torille:wizard-sheet': {
              const r = await instance.handle({ type: 'wizard-sheet' });
              return { sheet: r.sheet };
            }
            case 'torille:wizard-infer': {
              const r = await instance.handle({
                type: 'wizard-infer',
                word: message.word,
                pos: message.pos,
                answers: message.answers,
              });
              return { report: r.report };
            }
            case 'torille:wizard-paradigm': {
              const r = await instance.handle({
                type: 'wizard-paradigm',
                word: message.word,
                class: message.class,
                pos: message.pos,
              });
              return { paradigm: r.paradigm };
            }
            case 'torille:wizard-user-words': {
              const r = await instance.handle({
                type: 'wizard-user-words', text: message.text,
              });
              return { result: r.result };
            }
            case 'torille:wizard-user-words-add': {
              const r = await instance.handle({
                type: 'wizard-user-words-add',
                text: message.text,
                word: message.word,
                class: message.class,
                pos: message.pos,
              });
              return { result: r.result };
            }
            case 'torille:wizard-user-words-remove': {
              const r = await instance.handle({
                type: 'wizard-user-words-remove',
                text: message.text,
                word: message.word,
              });
              return { result: r.result };
            }
            default:
              return { error: `unexpected ${message.type}` };
          }
        },
      },
      storage: { local: store },
    };
  });

  await page.goto(PAGE, { waitUntil: 'domcontentloaded' });
  await page.waitForFunction(
    () => document.getElementById('message')?.textContent === '',
    { timeout: 60_000 },
  );

  await page.type('#word', 'mökki');
  await page.click('#start');
  await page.waitForSelector('#question-card:not([hidden])', { timeout: 20_000 });

  const answers = {
    'genetiivi (yksikkö)': 'mökin',
    'genetiivi (monikko)': 'mökkien',
    'partitiivi (yksikkö)': 'mökkiä',
    'partitiivi (monikko)': 'mökkejä',
    'illatiivi (yksikkö)': 'mökkiin',
    'illatiivi (monikko)': 'mökkeihin',
    'nominatiivi (monikko)': 'mökit',
  };

  for (let i = 0; i < 5; i++) {
    const hidden = await page.$eval('#question-card', (el) => el.hidden);
    if (!hidden) break;
    const label = await page.$eval('#question-label', (el) => el.textContent ?? '');
    const answer = answers[label];
    if (!answer) throw new Error(`no answer for ${label}`);
    await page.$eval('#answer', (el) => { el.value = ''; });
    await page.type('#answer', answer);
    await page.click('#submit');
    await sleep(300);
  }

  const result = await page.evaluate(() => ({
    best: document.getElementById('best')?.textContent,
    save: document.getElementById('save')?.textContent,
    cells: Array.from(document.querySelectorAll('#paradigm dd')).map((el) => el.textContent),
  }));

  await page.click('#save');
  await sleep(500);
  const saved = await page.evaluate(() => ({
    rows: Array.from(document.querySelectorAll('.saved-row')).map((el) => el.textContent),
    text: localStorage.getItem('ext:torille-user-words'),
  }));

  // Verb flow: the same page must switch to a finite-form grid.
  await page.$eval('#word', (el) => { el.value = 'puhua'; });
  await page.select('#pos', 'verbi');
  await page.click('#start');
  await page.waitForSelector('#question-card:not([hidden])', { timeout: 20_000 });

  const verbAnswers = {
    'preesens, 1. persoona (yksikkö)': 'puhun',
    'preesens, 3. persoona (yksikkö)': 'puhuu',
    'imperfekti, 3. persoona (yksikkö)': 'puhui',
    'konditionaali, 3. persoona (yksikkö)': 'puhuisi',
    'imperatiivi, 2. persoona (yksikkö)': 'puhu',
  };
  for (let i = 0; i < 5; i++) {
    const hidden = await page.$eval('#question-card', (el) => el.hidden);
    if (hidden) break;
    const label = await page.$eval('#question-label', (el) => el.textContent ?? '');
    const answer = verbAnswers[label];
    if (!answer) throw new Error(`no verb answer for ${label}`);
    await page.$eval('#answer', (el) => { el.value = ''; });
    await page.type('#answer', answer);
    await page.click('#submit');
    await sleep(300);
  }
  const verbResult = await page.evaluate(() => ({
    best: document.getElementById('best')?.textContent,
    cells: Array.from(document.querySelectorAll('#paradigm dd')).map((el) => el.textContent),
  }));
  await page.click('#save');
  await sleep(500);
  const verbSaved = await page.evaluate(() => ({
    rows: Array.from(document.querySelectorAll('.saved-row')).map((el) => el.textContent),
    text: localStorage.getItem('ext:torille-user-words'),
  }));

  // Proper-noun flow: the nominal grid, the `erisnimi` label, and the
  // surface capitalization in the preview.
  await page.$eval('#word', (el) => { el.value = 'Pöräkki'; });
  await page.select('#pos', 'erisnimi');
  await page.click('#start');
  await page.waitForSelector('#question-card:not([hidden])', { timeout: 20_000 });
  const properAnswers = {
    'genetiivi (yksikkö)': 'pöräkin',
    'partitiivi (yksikkö)': 'pöräkkiä',
    'illatiivi (yksikkö)': 'pöräkkiin',
    'genetiivi (monikko)': 'pöräkkien',
    'nominatiivi (monikko)': 'pöräkit',
  };
  for (let i = 0; i < 5; i++) {
    const hidden = await page.$eval('#question-card', (el) => el.hidden);
    if (hidden) break;
    const label = await page.$eval('#question-label', (el) => el.textContent ?? '');
    const answer = properAnswers[label];
    if (!answer) throw new Error(`no proper-noun answer for ${label}`);
    await page.$eval('#answer', (el) => { el.value = ''; });
    await page.type('#answer', answer);
    await page.click('#submit');
    await sleep(300);
  }
  const properResult = await page.evaluate(() => ({
    best: document.getElementById('best')?.textContent,
    cells: Array.from(document.querySelectorAll('#paradigm dd')).map((el) => el.textContent),
  }));
  await page.click('#save');
  await sleep(500);
  const properSaved = await page.evaluate(() => ({
    rows: Array.from(document.querySelectorAll('.saved-row')).map((el) => el.textContent),
    text: localStorage.getItem('ext:torille-user-words'),
  }));

  await page.reload({ waitUntil: 'domcontentloaded' });
  await page.waitForFunction(
    () => document.getElementById('message')?.textContent === '',
    { timeout: 60_000 },
  );
  const afterReload = await page.evaluate(() => ({
    rows: Array.from(document.querySelectorAll('.saved-row')).map((el) => el.textContent),
  }));

  const ok =
    result.best === '5*A' &&
    result.cells?.[1] === 'mökin' &&
    saved.rows?.some((row) => row.includes('mökki')) &&
    saved.text?.includes('mökki\t5*A\tsubstantiivi') &&
    verbResult.best === '52' &&
    verbResult.cells?.includes('puhun') &&
    verbSaved.rows?.some((row) => row.includes('puhua')) &&
    verbSaved.text?.includes('puhua\t52\tverbi') &&
    properResult.best === '5*A' &&
    properResult.cells?.includes('Pöräkin') &&
    properSaved.rows?.some((row) => row.includes('Pöräkki')) &&
    properSaved.text?.includes('Pöräkki\t5*A\terisnimi') &&
    afterReload.rows?.some((row) => row.includes('mökki')) &&
    afterReload.rows?.some((row) => row.includes('puhua')) &&
    afterReload.rows?.some((row) => row.includes('Pöräkki')) &&
    errors.length === 0;

  console.log(JSON.stringify({ result, saved, verbResult, verbSaved, properResult, properSaved, afterReload, errors }, null, 2));
  if (!ok) {
    console.error('wizard page smoke failed');
    process.exitCode = 1;
  } else {
    console.log('wizard page smoke passed');
  }
} finally {
  await browser?.close();
  server.kill();
}
