// The extension's "Omat sanat" page.
//
// The page does not load the wasm. Every engine call is a
// `runtime.sendMessage` to the background, which forwards it to the spell
// worker; the saved-word text lives in `browser.storage.local` and the
// background pushes it to every worker as a set of accepted surfaces.

export {};

declare const browser: any;
declare const chrome: any;

const api = typeof browser !== 'undefined' ? browser : chrome;

const USER_WORDS_KEY = 'torille-user-words';
const SESSION_KEY = 'torille-wizard-session';
const WORD_KEY = 'torille-wizard-word';
const POS_KEY = 'torille-wizard-pos';
const INDECLINABLE_KEY = 'torille-wizard-indeclinable';

type Candidate = {
  class: string;
  hits: number;
  misses: number;
  unknown: number;
  recorded: boolean;
  tail_prior: number;
  prior: number;
};

type Outcome = { key: string; form: string; status: string };

type Report = {
  word: string;
  pos: string;
  recorded: string[];
  candidates: Candidate[];
  total_candidates: number;
  outcomes: Outcome[];
  question: string | null;
  tie: boolean;
  error?: string | null;
};

type Paradigm = {
  word: string;
  class: string;
  cells: Record<string, string | null>;
  error?: string | null;
};

type UserWord = { word: string; class: string; pos: string };
type UserWordsResult = { words: UserWord[]; error: string | null };
type TextResult = { text: string; error: string | null };

const CORE_KEYS = [
  'nom_sg', 'gen_sg', 'par_sg', 'ill_sg',
  'nom_pl', 'gen_pl', 'par_pl', 'ill_pl',
];

const VERB_KEYS = [
  'pres_1sg', 'pres_3sg', 'past_3sg', 'cond_3sg', 'imp_2sg', 'pres_1pl',
];

function keysFor(pos: string): string[] {
  return pos === 'verbi' ? VERB_KEYS : CORE_KEYS;
}

const CELL_LABELS: Record<string, string> = {
  nom_sg: 'nominatiivi (yksikkö)',
  gen_sg: 'genetiivi (yksikkö)',
  par_sg: 'partitiivi (yksikkö)',
  ill_sg: 'illatiivi (yksikkö)',
  nom_pl: 'nominatiivi (monikko)',
  gen_pl: 'genetiivi (monikko)',
  par_pl: 'partitiivi (monikko)',
  ill_pl: 'illatiivi (monikko)',
  pres_1sg: 'preesens, 1. persoona (yksikkö)',
  pres_2sg: 'preesens, 2. persoona (yksikkö)',
  pres_3sg: 'preesens, 3. persoona (yksikkö)',
  past_3sg: 'imperfekti, 3. persoona (yksikkö)',
  cond_3sg: 'konditionaali, 3. persoona (yksikkö)',
  imp_2sg: 'imperatiivi, 2. persoona (yksikkö)',
  pres_1pl: 'preesens, 1. persoona (monikko)',
};

const state = {
  word: '',
  pos: 'substantiivi',
  answers: new Map<string, string>(),
  report: null as Report | null,
  selected: null as string | null,
  sheet: new Map<string, Map<string, string>>(),
  saved: [] as UserWord[],
  savedText: '',
  ready: false,
  questionCollapsed: false,
};

function byId<T extends HTMLElement>(id: string): T {
  return document.getElementById(id) as T;
}

function clear(node: HTMLElement): void {
  while (node.firstChild) node.removeChild(node.firstChild);
}

function message(text: string, error = false, ok = false): void {
  const node = byId<HTMLParagraphElement>('message');
  node.textContent = text;
  node.dataset.error = error ? 'true' : 'false';
  node.dataset.ok = ok ? 'true' : 'false';
}

async function send<T>(payload: unknown): Promise<T> {
  const reply = await api.runtime.sendMessage(payload);
  if (reply && typeof reply === 'object' && reply.error) {
    throw new Error(String(reply.error));
  }
  return reply as T;
}

function parseSheet(text: string): Map<string, Map<string, string>> {
  const sheet = new Map<string, Map<string, string>>();
  for (const raw of text.split('\n')) {
    const line = raw.endsWith('\r') ? raw.slice(0, -1) : raw;
    if (!line || line.startsWith('#')) continue;
    const [cls, key, value = ''] = line.split('\t');
    if (!cls || !key) continue;
    let section = sheet.get(cls);
    if (!section) {
      section = new Map();
      sheet.set(cls, section);
    }
    section.set(key, value);
  }
  return sheet;
}

function answersText(): string {
  let out = '# torille-wizard-answers v1\n';
  for (const [key, form] of state.answers) out += `${key}\t${form}\n`;
  return out;
}

function saveSession(): void {
  try {
    localStorage.setItem(SESSION_KEY, answersText());
    localStorage.setItem(WORD_KEY, state.word);
    localStorage.setItem(POS_KEY, state.pos);
    localStorage.setItem(INDECLINABLE_KEY, byId<HTMLInputElement>('indeclinable').checked ? '1' : '');
  } catch {
    /* private mode */
  }
}

async function readUserWords(): Promise<string> {
  const raw = await api.storage.local.get(USER_WORDS_KEY);
  const text = raw[USER_WORDS_KEY];
  return typeof text === 'string' ? text : '';
}

async function writeUserWords(text: string): Promise<void> {
  await api.storage.local.set({ [USER_WORDS_KEY]: text });
}

async function infer(): Promise<void> {
  if (!state.ready || !state.word) return;
  try {
    const reply = await send<{ report: Report }>({
      type: 'torille:wizard-infer',
      word: state.word,
      pos: state.pos,
      answers: answersText(),
    });
    if (reply.report.error) {
      message(String(reply.report.error), true);
      return;
    }
    state.report = reply.report;
    state.selected = null;
    render();
  } catch (error) {
    message(String(error), true);
  }
}

function setQuestionCollapsed(collapsed: boolean): void {
  state.questionCollapsed = collapsed;
  byId<HTMLElement>('question-body').hidden = collapsed;
  const button = byId<HTMLButtonElement>('question-toggle');
  button.textContent = collapsed ? 'Näytä' : 'Piilota';
  button.setAttribute('aria-expanded', String(!collapsed));
}

function render(): void {
  const report = state.report;
  if (!report) return;

  const recorded = byId<HTMLParagraphElement>('recorded');
  recorded.textContent = report.recorded.length
    ? `Sanalista: ${report.recorded.join(', ')}`
    : 'Ei sanalistassa — luokka päätellään vastauksista.';

  const questionCard = byId<HTMLElement>('question-card');
  const candidatesCard = byId<HTMLElement>('candidates-card');
  const resultCard = byId<HTMLElement>('result-card');
  const examplesCard = byId<HTMLElement>('examples-card');
  examplesCard.hidden = true;
  candidatesCard.hidden = false;
  renderCandidates();
  renderOutcomes();

  // The best candidate is usable before the questions end: the tail
  // prior can settle a tie on the first answer, and the user may want to
  // save without answering all five.
  const best = report.candidates[0];
  resultCard.hidden = !best;
  if (best) {
    byId('best').textContent = best.class;
    byId('tie').textContent = report.tie
      ? 'Useampi luokka tuottaa samat muodot tälle sanalle.'
      : '';
    state.selected = state.selected ?? best.class;
    void preview(state.selected);
  }

  if (report.question) {
    questionCard.hidden = false;
    setQuestionCollapsed(false);
    byId('question-label').textContent =
      CELL_LABELS[report.question] ?? report.question;
    const hint = example(report.question);
    byId('question-hint').textContent = hint ? `esim. ${hint}` : '';
    const input = byId<HTMLInputElement>('answer');
    input.value = '';
    input.focus();
  } else {
    questionCard.hidden = true;
  }
}

function renderCandidates(): void {
  const report = state.report;
  const body = byId<HTMLTableSectionElement>('candidates');
  clear(body);
  byId('candidates-count').textContent = report
    ? `${report.candidates.length} / ${report.total_candidates}`
    : '';
  if (!report) return;
  for (const candidate of report.candidates.slice(0, 8)) {
    const row = document.createElement('tr');
    if (candidate.class === state.selected) row.className = 'selected';
    const cls = document.createElement('td');
    cls.textContent = candidate.class;
    if (candidate.recorded) {
      const star = document.createElement('span');
      star.className = 'star';
      star.textContent = ' ★';
      star.title = 'sanalistan luokka';
      cls.appendChild(star);
    }
    const hits = document.createElement('td');
    hits.textContent = String(candidate.hits);
    const misses = document.createElement('td');
    misses.textContent = String(candidate.misses);
    const tail = document.createElement('td');
    tail.textContent = String(candidate.tail_prior);
    const prior = document.createElement('td');
    prior.textContent = String(candidate.prior);
    const pick = document.createElement('td');
    const button = document.createElement('button');
    button.className = 'ghost';
    button.textContent = 'valitse';
    button.addEventListener('click', () => selectClass(candidate.class));
    pick.appendChild(button);
    row.append(cls, hits, misses, tail, prior, pick);
    body.appendChild(row);
  }
}

function renderOutcomes(): void {
  const node = byId('outcomes');
  clear(node);
  if (!state.report) return;
  for (const outcome of state.report.outcomes) {
    const chip = document.createElement('span');
    chip.className = `chip ${outcome.status}`;
    chip.textContent = `${outcome.key}: ${outcome.form || '—'}`;
    node.appendChild(chip);
  }
}

function example(key: string): string {
  const top = state.report?.candidates[0];
  const base = (top?.class ?? '1').split('*')[0].split(',')[0];
  const section = state.sheet.get(base);
  const model = section?.get('model');
  const form = section?.get(key);
  return model && form ? `${model} → ${form}` : '';
}

function selectClass(cls: string): void {
  state.selected = cls;
  renderCandidates();
  void preview(cls);
}

function showExamples(): void {
  const report = state.report;
  if (!report) return;
  const node = byId('examples');
  clear(node);
  const seen = new Set<string>();
  for (const candidate of report.candidates.slice(0, 3)) {
    const base = candidate.class.split('*')[0].split(',')[0];
    if (seen.has(base)) continue;
    seen.add(base);
    const section = state.sheet.get(base);
    if (!section) continue;
    const keys = section.get('pos') === 'verb' ? VERB_KEYS : CORE_KEYS;
    const details = document.createElement('details');
    details.open = true;
    const summary = document.createElement('summary');
    summary.textContent = `Luokka ${base} · ${section.get('model') ?? ''}`;
    details.appendChild(summary);
    const grid = document.createElement('dl');
    grid.className = 'grid';
    for (const key of keys) {
      const dt = document.createElement('dt');
      dt.textContent = CELL_LABELS[key] ?? key;
      const dd = document.createElement('dd');
      dd.textContent = section.get(key) ?? '—';
      grid.append(dt, dd);
    }
    details.appendChild(grid);
    node.appendChild(details);
  }
  byId('examples-card').hidden = false;
  byId('examples-card').scrollIntoView({ behavior: 'smooth', block: 'nearest' });
}

function showForm(form: string): string {
  const first = state.word.charAt(0);
  if (first && first === first.toLocaleUpperCase('fi')
      && first !== first.toLocaleLowerCase('fi')) {
    return first + form.slice(1);
  }
  return form;
}

async function preview(cls: string): Promise<void> {
  if (cls === '99') {
    const grid = byId<HTMLDListElement>('paradigm');
    clear(grid);
    const dt = document.createElement('dt');
    dt.textContent = 'perusmuoto';
    const dd = document.createElement('dd');
    dd.textContent = state.word;
    grid.append(dt, dd);
    byId('selected-note').textContent = 'Taipumaton: vain perusmuoto.';
    byId('save').textContent = 'Tallenna 99';
    return;
  }
  try {
    const reply = await send<{ paradigm: Paradigm }>({
      type: 'torille:wizard-paradigm',
      word: state.word,
      class: cls,
      pos: state.pos,
    });
    const result = reply.paradigm;
    if (result.error) {
      message(String(result.error), true);
      return;
    }
    const grid = byId<HTMLDListElement>('paradigm');
    clear(grid);
    for (const key of keysFor(state.pos)) {
      const dt = document.createElement('dt');
      dt.textContent = CELL_LABELS[key] ?? key;
      const dd = document.createElement('dd');
      const form = result.cells[key];
      dd.textContent = form ? showForm(form) : '—';
      grid.append(dt, dd);
    }
    const best = state.report?.candidates[0]?.class;
    byId('selected-note').textContent =
      cls === best
        ? 'Taivutus: paras ehdokas.'
        : `Taivutus: valittu ${cls}${best ? ` (paras ${best})` : ''}.`;
    byId('save').textContent = `Tallenna ${cls}`;
  } catch (error) {
    message(String(error), true);
  }
}

function submitAnswer(): void {
  const question = state.report?.question;
  if (!question) return;
  const input = byId<HTMLInputElement>('answer');
  const form = input.value.trim();
  state.answers.set(question, form);
  saveSession();
  void infer();
}

function skipAnswer(): void {
  const question = state.report?.question;
  if (!question) return;
  state.answers.set(question, '');
  saveSession();
  void infer();
}

function indeclinableReport(): Report {
  return {
    word: state.word,
    pos: state.pos,
    recorded: [],
    candidates: [{
      class: '99',
      hits: 0,
      misses: 0,
      unknown: 0,
      recorded: false,
      tail_prior: 0,
      prior: 0,
    }],
    total_candidates: 1,
    outcomes: [],
    question: null,
    tie: false,
  };
}

function start(): void {
  const word = byId<HTMLInputElement>('word').value.trim();
  if (!word) {
    message('Kirjoita sana ensin.', true);
    return;
  }
  state.word = word;
  state.pos = byId<HTMLSelectElement>('pos').value;
  const indeclinable = byId<HTMLInputElement>('indeclinable').checked;
  state.answers.clear();
  state.selected = null;
  message('');
  saveSession();
  if (indeclinable) {
    state.report = indeclinableReport();
    render();
    return;
  }
  void infer();
}

async function refreshSaved(): Promise<void> {
  try {
    state.savedText = await readUserWords();
    const reply = await send<{ result: UserWordsResult }>({
      type: 'torille:wizard-user-words',
      text: state.savedText,
    });
    const result = reply.result;
    if (result.error) {
      message(String(result.error), true);
      state.saved = [];
    } else {
      state.saved = result.words.slice().sort((a, b) =>
        a.word.localeCompare(b.word, 'fi'));
    }
    renderSaved();
  } catch (error) {
    message(String(error), true);
  }
}

function renderSaved(): void {
  const node = byId('saved');
  clear(node);
  if (state.saved.length === 0) {
    const p = document.createElement('p');
    p.className = 'muted';
    p.textContent = 'Ei tallennettuja sanoja.';
    node.appendChild(p);
    return;
  }
  for (const entry of state.saved) {
    const row = document.createElement('div');
    row.className = 'saved-row';
    const label = document.createElement('div');
    const word = document.createElement('span');
    word.className = 'saved-word';
    word.textContent = entry.word;
    const detail = document.createElement('span');
    detail.className = 'muted';
    detail.textContent = `  ${entry.class} · ${entry.pos}`;
    label.append(word, detail);
    const remove = document.createElement('button');
    remove.className = 'danger';
    remove.textContent = 'poista';
    remove.addEventListener('click', () => void removeSaved(entry.word));
    row.append(label, remove);
    node.appendChild(row);
  }
}

async function saveWord(): Promise<void> {
  const report = state.report;
  const cls = state.selected ?? report?.candidates[0]?.class;
  if (!report || !cls) {
    message('Ei tallennettavaa luokkaa.', true);
    return;
  }
  const pos = state.pos;
  try {
    const reply = await send<{ result: TextResult }>({
      type: 'torille:wizard-user-words-add',
      text: state.savedText,
      word: report.word,
      class: cls,
      pos,
    });
    const result = reply.result;
    if (result.error) {
      message(String(result.error), true);
      return;
    }
    state.savedText = result.text;
    await writeUserWords(result.text);
    await refreshSaved();
    message(`${report.word} → ${cls} tallennettu.`, false, true);
  } catch (error) {
    message(String(error), true);
  }
}

async function removeSaved(word: string): Promise<void> {
  try {
    const reply = await send<{ result: TextResult }>({
      type: 'torille:wizard-user-words-remove',
      text: state.savedText,
      word,
    });
    const result = reply.result;
    if (result.error) {
      message(String(result.error), true);
      return;
    }
    state.savedText = result.text;
    await writeUserWords(result.text);
    await refreshSaved();
  } catch (error) {
    message(String(error), true);
  }
}

function exportSaved(): void {
  const text = state.savedText || '# torille-user-words v1\n';
  const blob = new Blob([text], { type: 'text/tab-separated-values' });
  const url = URL.createObjectURL(blob);
  const link = document.createElement('a');
  link.href = url;
  link.download = 'torille-user-words.txt';
  link.click();
  URL.revokeObjectURL(url);
}

async function importSaved(file: File): Promise<void> {
  try {
    const text = await file.text();
    const reply = await send<{ result: UserWordsResult }>({
      type: 'torille:wizard-user-words',
      text,
    });
    const result = reply.result;
    if (result.error) {
      message(`Tuonti epäonnistui: ${result.error}`, true);
      return;
    }
    state.savedText = text;
    await writeUserWords(text);
    await refreshSaved();
    message(`Tuotu ${result.words.length} sanaa.`, false, true);
  } catch (error) {
    message(String(error), true);
  }
}

function renderBrowse(): void {
  const query = byId<HTMLInputElement>('browse-search').value.trim().toLowerCase();
  const rows: { class: string; section: Map<string, string>; lemmas: number }[] = [];
  for (const [cls, section] of state.sheet) {
    const model = section.get('model') ?? '';
    const lemmas = Number(section.get('lemmas') ?? 0);
    if (query && !cls.includes(query) && !model.toLowerCase().includes(query)) {
      continue;
    }
    rows.push({ class: cls, section, lemmas });
  }
  rows.sort((a, b) => {
    if (b.lemmas !== a.lemmas) return b.lemmas - a.lemmas;
    return Number(a.class) - Number(b.class);
  });
  const node = byId('browse-list');
  clear(node);
  for (const row of rows) {
    const details = document.createElement('details');
    const summary = document.createElement('summary');
    const model = row.section.get('model') ?? '';
    summary.textContent = `${row.class} · ${model} · ${row.lemmas} lemmaa`;
    details.appendChild(summary);
    const grid = document.createElement('dl');
    grid.className = 'grid';
    const keys = row.section.get('pos') === 'verb' ? VERB_KEYS : CORE_KEYS;
    for (const key of keys) {
      const dt = document.createElement('dt');
      dt.textContent = CELL_LABELS[key] ?? key;
      const dd = document.createElement('dd');
      dd.textContent = row.section.get(key) ?? '—';
      grid.append(dt, dd);
    }
    details.appendChild(grid);
    node.appendChild(details);
  }
  if (rows.length === 0) {
    const p = document.createElement('p');
    p.className = 'muted';
    p.textContent = 'Ei osumia.';
    node.appendChild(p);
  }
}

function setTab(tab: 'find' | 'browse'): void {
  byId('tab-find').setAttribute('aria-selected', String(tab === 'find'));
  byId('tab-browse').setAttribute('aria-selected', String(tab === 'browse'));
  byId<HTMLElement>('pane-find').hidden = tab !== 'find';
  byId<HTMLElement>('pane-browse').hidden = tab !== 'browse';
}

function wire(): void {
  byId('start').addEventListener('click', start);
  byId('submit').addEventListener('click', submitAnswer);
  byId('skip').addEventListener('click', skipAnswer);
  byId('examples').addEventListener('click', showExamples);
  byId('question-toggle').addEventListener('click', () =>
    setQuestionCollapsed(!state.questionCollapsed));
  byId('examples-close').addEventListener('click', () => {
    byId<HTMLElement>('examples-card').hidden = true;
  });
  byId('save').addEventListener('click', () => void saveWord());
  byId('export').addEventListener('click', exportSaved);
  byId('tab-find').addEventListener('click', () => setTab('find'));
  byId('tab-browse').addEventListener('click', () => setTab('browse'));
  byId('browse-search').addEventListener('input', renderBrowse);
  byId<HTMLInputElement>('answer').addEventListener('keydown', (event) => {
    if (event.key === 'Enter') submitAnswer();
  });
  byId<HTMLInputElement>('word').addEventListener('keydown', (event) => {
    if (event.key === 'Enter') start();
  });
  byId<HTMLInputElement>('import-file').addEventListener('change', (event) => {
    const input = event.currentTarget as HTMLInputElement;
    const file = input.files?.[0];
    if (file) void importSaved(file);
    input.value = '';
  });
}

function restoreSession(): void {
  try {
    const word = localStorage.getItem(WORD_KEY);
    if (!word) return;
    state.word = word;
    const rawPos = localStorage.getItem(POS_KEY) ?? '';
    const savedPos = rawPos === 'noun'
      ? 'substantiivi'
      : rawPos === 'adjective'
        ? 'adjektiivi'
        : rawPos === 'verb'
          ? 'verbi'
          : rawPos;
    state.pos = [
      'substantiivi', 'adjektiivi', 'numeraali', 'järjestysluku',
      'pronomini', 'erisnimi', 'verbi',
    ].includes(savedPos) ? savedPos : 'substantiivi';
    const text = localStorage.getItem(SESSION_KEY) ?? '';
    for (const raw of text.split('\n')) {
      const line = raw.endsWith('\r') ? raw.slice(0, -1) : raw;
      if (!line || line.startsWith('#')) continue;
      const [key, form = ''] = line.split('\t');
      if (key) state.answers.set(key, form);
    }
    byId<HTMLInputElement>('word').value = state.word;
    byId<HTMLSelectElement>('pos').value = state.pos;
    const indeclinable = localStorage.getItem(INDECLINABLE_KEY) === '1';
    byId<HTMLInputElement>('indeclinable').checked = indeclinable;
    if (indeclinable) {
      state.report = indeclinableReport();
      render();
    } else {
      void infer();
    }
  } catch {
    /* ignore a corrupt session */
  }
}

async function main(): Promise<void> {
  wire();
  setTab('find');
  message('Ladataan sana-aineistoa…');
  const reply = await send<{ sheet: string }>({ type: 'torille:wizard-sheet' });
  state.sheet = parseSheet(reply.sheet);
  state.ready = true;
  await refreshSaved();
  renderBrowse();
  message('');
  restoreSession();
}

void main().catch((error: unknown) => {
  message(`Lataus epäonnistui: ${String(error)}`, true);
});
