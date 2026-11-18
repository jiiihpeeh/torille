import puppeteer from 'puppeteer-core';

const browser = await puppeteer.launch({
  executablePath: '/usr/bin/google-chrome',
  headless: 'new',
  args: ['--no-sandbox', '--disable-gpu', '--disable-dev-shm-usage'],
  defaultViewport: { width: 1000, height: 900 },
});

const errors = [];
try {
  const page = await browser.newPage();
  page.on('console', (msg) => {
    if (msg.type() === 'error') errors.push(msg.text());
  });
  page.on('pageerror', (err) => errors.push(String(err)));

  await page.goto('http://127.0.0.1:8765/wizard.html', {
    waitUntil: 'domcontentloaded',
  });
  await page.waitForFunction(
    () => document.getElementById('message')?.textContent === '',
    { timeout: 30_000 },
  );

  await page.type('#word', 'mökki');
  await page.click('#start');
  await page.waitForSelector('#question-card:not([hidden])', { timeout: 10_000 });

  const answers = {
    'genetiivi (yksikkö)': 'mökin',
    'genetiivi (monikko)': 'mökkien',
    'partitiivi (yksikkö)': 'mökkiä',
    'partitiivi (monikko)': 'mökkejä',
    'illatiivi (yksikkö)': 'mökkiin',
    'illatiivi (monikko)': 'mökkeihin',
    'nominatiivi (monikko)': 'mökit',
  };

  const asked = [];
  for (let i = 0; i < 5; i++) {
    const hidden = await page.$eval('#question-card', (el) => el.hidden);
    if (!hidden) break;
    const label = await page.$eval('#question-label', (el) => el.textContent ?? '');
    asked.push(label);
    const answer = answers[label];
    if (!answer) throw new Error(`no answer for ${label}`);
    await page.$eval('#answer', (el) => { el.value = ''; });
    await page.type('#answer', answer);
    await page.click('#submit');
    await new Promise((resolve) => setTimeout(resolve, 250));
  }

  const result = await page.evaluate(() => ({
    best: document.getElementById('best')?.textContent,
    tie: document.getElementById('tie')?.textContent,
    save: document.getElementById('save')?.textContent,
    paradigm: Array.from(document.querySelectorAll('#paradigm dd')).map((el) => el.textContent),
  }));

  await page.click('#save');
  await new Promise((resolve) => setTimeout(resolve, 250));
  const saved = await page.evaluate(() => ({
    rows: Array.from(document.querySelectorAll('.saved-row')).map((el) => el.textContent),
    storage: localStorage.getItem('torille-user-words'),
    message: document.getElementById('message')?.textContent,
  }));

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
    await new Promise((resolve) => setTimeout(resolve, 250));
  }
  const verbResult = await page.evaluate(() => ({
    best: document.getElementById('best')?.textContent,
    cells: Array.from(document.querySelectorAll('#paradigm dd')).map((el) => el.textContent),
  }));
  await page.click('#save');
  await new Promise((resolve) => setTimeout(resolve, 250));
  const verbSaved = await page.evaluate(() => ({
    rows: Array.from(document.querySelectorAll('.saved-row')).map((el) => el.textContent),
    storage: localStorage.getItem('torille-user-words'),
  }));

  await page.reload({ waitUntil: 'domcontentloaded' });
  await page.waitForFunction(
    () => document.getElementById('message')?.textContent === '',
    { timeout: 30_000 },
  );
  const afterReload = await page.evaluate(() => ({
    rows: Array.from(document.querySelectorAll('.saved-row')).map((el) => el.textContent),
    word: document.getElementById('word')?.value,
  }));

  console.log(JSON.stringify({ asked, result, saved, verbResult, verbSaved, afterReload, errors }, null, 2));
} finally {
  await browser.close();
}
