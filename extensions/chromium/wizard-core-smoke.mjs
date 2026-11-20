// Worker-core smoke test for the saved-word layer.
//
// The compiled `dist/spell-worker/spell-core.js` fetches its wasm relative to
// its own URL. Node has no `file://` fetch, so install one before importing
// the worker. The real extension never needs this: it fetches an extension
// URL.
//
// Run from `extensions/firefox/` after `npm run build`:
//   node wizard-core-smoke.mjs

import { readFile } from 'node:fs/promises';

const realFetch = globalThis.fetch;
globalThis.fetch = async (input) => {
  const url = input instanceof URL
    ? input
    : new URL(typeof input === 'string' ? input : input.url, import.meta.url);
  if (url.protocol === 'file:') {
    const bytes = await readFile(url);
    return new Response(bytes, {
      headers: { 'Content-Type': 'application/wasm' },
    });
  }
  return realFetch(input);
};

const { createSpellCore } = await import('./dist/spell-worker/spell-core.js');
const core = createSpellCore();
await core.init();

let failures = 0;
function check(label, ok, detail = '') {
  console.log(`  ${ok ? 'OK ' : 'BAD'} ${label}${detail ? `: ${detail}` : ''}`);
  if (!ok) failures++;
}

const custom = '# torille-user-words v1\npöräkki\t5*A\tsubstantiivi\n';
const pushed = await core.handle({ type: 'set-user-words', text: custom });
check('set-user-words', pushed?.type === 'set-user-words', JSON.stringify(pushed));

const accepted = await core.handle({ type: 'check', word: 'pöräkin' });
check('custom form accepted', accepted?.type === 'check' && accepted.ok === true,
  JSON.stringify(accepted));

const typo = await core.handle({ type: 'check', word: 'pöräkkö' });
check('non-custom typo rejected', typo?.type === 'check' && typo.ok === false,
  JSON.stringify(typo));

const suggested = await core.handle({
  type: 'suggest', word: 'pöräkkö', max: 5,
});
check('custom suggestion',
  suggested?.type === 'suggest' && suggested.suggestions.includes('pöräkin'),
  JSON.stringify(suggested));

const batch = await core.handle({
  type: 'suggest-batch', inputId: 1, items: [{ word: 'pöräkkö', max: 5 }],
});
check('custom suggestion batch',
  batch?.type === 'suggest-batch'
    && batch.results?.[0]?.includes('pöräkin'),
  JSON.stringify(batch));

const created = await core.handle({ type: 'create-state' });
const idx = created?.idx;
check('create-state', typeof idx === 'number', String(idx));

await core.handle({ type: 'update-state', idx, text: 'pöräkki pöräkkö' });
const withState = await core.handle({
  type: 'suggest-with-state', idx, word: 'pöräkkö', max: 5,
});
check('custom suggestion with state',
  withState?.type === 'suggest-with-state'
    && withState.suggestions.includes('pöräkin'),
  JSON.stringify(withState));

const analyzed = await core.handle({
  type: 'analyze-text', idx, text: 'pöräkki pöräkkö', max: 5,
});
const hits = Array.isArray(analyzed?.hits)
  ? analyzed.hits.map((hit) => hit.word)
  : [];
check(
  'analyze-text filters the custom word',
  !hits.includes('pöräkki') && hits.includes('pöräkkö'),
  JSON.stringify(hits),
);

await core.handle({ type: 'set-user-words', text: '' });
const cleared = await core.handle({ type: 'check', word: 'pöräkin' });
check('cleared list rejects the form',
  cleared?.type === 'check' && cleared.ok === false, JSON.stringify(cleared));

await core.handle({
  type: 'set-user-words',
  text: '# torille-user-words v1\npöräkki\t99\tsubstantiivi\n',
});
const indeclLemma = await core.handle({ type: 'check', word: 'pöräkki' });
check('indeclinable accepts the lemma',
  indeclLemma?.type === 'check' && indeclLemma.ok === true,
  JSON.stringify(indeclLemma));
const indeclInflected = await core.handle({ type: 'check', word: 'pöräkin' });
check('indeclinable does not invent forms',
  indeclInflected?.type === 'check' && indeclInflected.ok === false,
  JSON.stringify(indeclInflected));
await core.handle({ type: 'set-user-words', text: '' });

const sheet = await core.handle({ type: 'wizard-sheet' });
check('wizard-sheet', sheet?.type === 'wizard-sheet' && sheet.sheet.includes('1\tmodel\t'),
  sheet?.type);

const report = await core.handle({
  type: 'wizard-infer',
  word: 'mökki',
  pos: 'noun',
  answers: '# torille-wizard-answers v1\ngen_sg\tmökin\npar_sg\tmökkiä\nill_sg\tmökkiin\n',
});
check('wizard-infer', report?.type === 'wizard-infer'
  && report.report?.candidates?.[0]?.class === '5*A',
  JSON.stringify(report?.report?.candidates?.[0]));

const paradigm = await core.handle({
  type: 'wizard-paradigm', word: 'mökki', class: '5*A', pos: 'noun',
});
check('wizard-paradigm', paradigm?.type === 'wizard-paradigm'
  && paradigm.paradigm?.cells?.gen_sg === 'mökin',
  JSON.stringify(paradigm?.paradigm?.cells));

const verb = await core.handle({
  type: 'wizard-infer',
  word: 'puhua',
  pos: 'verb',
  answers: [
    '# torille-wizard-answers v1',
    'pres_1sg\tpuhun',
    'pres_3sg\tpuhuu',
    'past_3sg\tpuhui',
    '',
  ].join('\n'),
});
check('verb infer', verb?.type === 'wizard-infer'
  && verb.report?.candidates?.[0]?.class === '52',
  JSON.stringify(verb?.report?.candidates?.[0]));

const verbParadigm = await core.handle({
  type: 'wizard-paradigm', word: 'puhua', class: '52', pos: 'verb',
});
check('verb paradigm', verbParadigm?.type === 'wizard-paradigm'
  && verbParadigm.paradigm?.cells?.pres_1sg === 'puhun'
  && verbParadigm.paradigm?.cells?.imp_2sg === 'puhu',
  JSON.stringify(verbParadigm?.paradigm?.cells));

const knownPlural = await core.handle({
  type: 'wizard-infer',
  word: 'housut',
  pos: 'noun',
  answers: '# torille-wizard-answers v1\n',
});
check('known plural-only asks the plural pool',
  knownPlural?.type === 'wizard-infer'
    && knownPlural.report?.recorded_plural_only === true
    && knownPlural.report?.question === 'gen_pl',
  JSON.stringify(knownPlural?.report?.question));

const pluralReport = await core.handle({
  type: 'wizard-infer',
  word: 'kiikarit',
  pos: 'noun',
  number: 'pl',
  answers: '# torille-wizard-answers v1\n',
});
check('plural question pool', pluralReport?.type === 'wizard-infer'
  && pluralReport.report?.question === 'gen_pl',
  JSON.stringify(pluralReport?.report?.question));

await core.handle({
  type: 'set-user-words',
  text: '# torille-user-words v2\npöräkki\t5\tsubstantiivi\t-\n',
});
const pluralKnown = await core.handle({ type: 'check', word: 'pöräkkien' });
check('the plural is a form without the filter',
  pluralKnown?.type === 'check' && pluralKnown.ok === true,
  JSON.stringify(pluralKnown));

await core.handle({
  type: 'set-user-words',
  text: '# torille-user-words v2\npöräkki\t5\tsubstantiivi\tnumber=sg\n',
});
const singularAccepted = await core.handle({ type: 'check', word: 'pöräkkin' });
check('singular-only accepts the singular',
  singularAccepted?.type === 'check' && singularAccepted.ok === true,
  JSON.stringify(singularAccepted));
const pluralRejected = await core.handle({ type: 'check', word: 'pöräkkien' });
check('singular-only rejects the plural',
  pluralRejected?.type === 'check' && pluralRejected.ok === false,
  JSON.stringify(pluralRejected));
await core.handle({ type: 'set-user-words', text: '' });

if (failures > 0) {
  console.log(`\n${failures} check(s) failed`);
  process.exit(1);
}
console.log('\nall checks passed');
