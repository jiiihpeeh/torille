// Manual test page for the torille browser extension.
//
// This page deliberately does NOT start the demo's built-in checker: it
// exists to exercise the *installed extension* — the content script's
// scanner, the per-textbox overlays, the dynamically-created inputs, the
// per-frame behaviour and the context-menu toggles.
//
// Open it from the demo server:
//
//   http://127.0.0.1:8765/extension-test.html
//
// It is a separate Solid entry point (`extension-test.tsx`) so it stays
// out of `main.tsx` and never loads the wasm bundle.

import { createSignal, For, onCleanup, onMount, Show } from 'solid-js';
import type { JSX } from 'solid-js';
import { render } from 'solid-js/web';

type BoxKind = 'textarea' | 'input' | 'editable';

type DynamicBox = {
  id: number;
  kind: BoxKind;
  text: string;
};

const SAMPLES = [
  'kirjoitusvirhee talossa.',
  'koirra juoksee taloo.',
  'xyzzy ei ole suomea.',
  'riisryynipuuro ja selittämmin.',
  'Tämä lause on oikein kirjoitettu.',
];

function sample(): string {
  return SAMPLES[Math.floor(Math.random() * SAMPLES.length)];
}

function allControls(): HTMLElement[] {
  return Array.from(
    document.querySelectorAll<HTMLElement>(
      'textarea, input[type="text"], input[type="search"], input[type="email"], ' +
        'input[type="url"], input:not([type]), [contenteditable="true"], ' +
        '[contenteditable=""]',
    ),
  );
}

function replaceText(el: HTMLElement, text: string): void {
  if (el.isContentEditable) {
    el.textContent = text;
  } else {
    (el as HTMLInputElement | HTMLTextAreaElement).value = text;
  }
  el.dispatchEvent(new Event('input', { bubbles: true }));
}

function countMisspells(): number {
  let n = 0;
  for (const host of document.querySelectorAll('.torille-overlay-host')) {
    n += host.shadowRoot?.querySelectorAll('.misspell').length ?? 0;
  }
  const highlights = (CSS as unknown as {
    highlights?: { get?: (name: string) => { size?: number } | undefined };
  }).highlights;
  const hl = highlights?.get?.('torille-misspell');
  if (hl && typeof hl.size === 'number') n += hl.size;
  return n;
}

function AutoTextarea(props: {
  id?: string;
  class?: string;
  value?: string;
  placeholder?: string;
  rows?: number;
}) {
  let ref: HTMLTextAreaElement | undefined;
  return (
    <textarea
      ref={ref}
      id={props.id}
      class={props.class}
      rows={props.rows}
      placeholder={props.placeholder}
      onInput={() => {
        if (!ref) return;
        ref.style.height = 'auto';
        ref.style.height = ref.scrollHeight + 'px';
      }}
    >
      {props.value}
    </textarea>
  );
}

function Card(props: { label: string; children: JSX.Element }) {
  return (
    <div class="card">
      <label>{props.label}</label>
      {props.children}
    </div>
  );
}

function DynamicBoxView(props: { box: DynamicBox }) {
  return (
    <Card label={`Dynamic #${props.box.id}`}>
      {props.box.kind === 'textarea' ? (
        <AutoTextarea value={props.box.text} rows={2} />
      ) : props.box.kind === 'input' ? (
        <input type="text" value={props.box.text} />
      ) : (
        <div class="editable" contentEditable={true}>
          {props.box.text}
        </div>
      )}
    </Card>
  );
}

function App() {
  const [boxes, setBoxes] = createSignal<DynamicBox[]>([]);
  const [inputs, setInputs] = createSignal(0);
  const [hosts, setHosts] = createSignal(0);
  const [misses, setMisses] = createSignal(0);
  const [debug, setDebug] = createSignal(false);
  const [cover, setCover] = createSignal(false);
  const [log, setLog] = createSignal<string[]>([]);
  let nextId = 1;
  let shadowHost: HTMLDivElement | undefined;

  function note(message: string): void {
    const stamp = new Date().toLocaleTimeString();
    setLog((lines) => [...lines, `[${stamp}] ${message}`].slice(-80));
  }

  function add(kind: BoxKind, count = 1): void {
    const added: DynamicBox[] = [];
    for (let i = 0; i < count; i++) {
      added.push({ id: nextId++, kind, text: sample() });
    }
    setBoxes((cur) => [...cur, ...added]);

    if (count === 1) {
      note(`added dynamic ${kind}`);
    } else {
      note(`added ${count} dynamic boxes`);
    }
  }

  function addMixed(count: number): void {
    const kinds: BoxKind[] = ['textarea', 'input', 'editable'];
    const added: DynamicBox[] = [];
    for (let i = 0; i < count; i++) {
      added.push({
        id: nextId++,
        kind: kinds[i % kinds.length],
        text: sample(),
      });
    }
    setBoxes((cur) => [...cur, ...added]);
    note(`added ${count} mixed dynamic boxes`);
  }

  function clearDynamic(): void {
    setBoxes([]);
    note('removed all dynamic boxes');
  }

  function fillAll(): void {
    const controls = allControls();
    for (const el of controls) replaceText(el, sample());
    note(`filled ${controls.length} boxes`);
  }

  function clearAll(): void {
    const controls = allControls();
    for (const el of controls) replaceText(el, '');
    note(`cleared ${controls.length} boxes`);
  }

  function withDebug(value: boolean): void {
    try {
      if (value) localStorage.setItem('torille-debug', '1');
      else localStorage.removeItem('torille-debug');
      location.reload();
    } catch (e) {
      note('could not write localStorage: ' + String(e));
    }
  }

  onMount(() => {
    try {
      setDebug(localStorage.getItem('torille-debug') === '1');
    } catch {
      /* storage can be blocked */
    }

    // A box inside an open shadow root. The content script does not
    // pierce shadow roots, so this one is expected to stay unmarked:
    // it makes that limitation visible instead of silently surprising.
    if (shadowHost) {
      const root = shadowHost.attachShadow({ mode: 'open' });
      root.innerHTML = `
        <label style="display:block;font-weight:600;font-size:.9em;margin-bottom:.3rem">
          Inside shadow root (expected: no underline)
        </label>
        <textarea style="width:100%;min-height:4.5em;padding:.55rem .65rem;
          border:1px solid #d0d0d8;border-radius:6px;font:inherit">kirjoitusvirhee taloo</textarea>
      `;
    }

    function refreshStatus(): void {
      setInputs(allControls().length);
      setHosts(
        document.querySelectorAll('.torille-overlay-host, .torille-ce-host')
          .length,
      );
      setMisses(countMisspells());
      try {
        setDebug(localStorage.getItem('torille-debug') === '1');
      } catch {
        /* ignore */
      }
    }
    refreshStatus();
    const timer = window.setInterval(refreshStatus, 500);
    onCleanup(() => window.clearInterval(timer));

    note('page ready — install/load the torille extension, then type');
  });

  return (
    <>
      <h1>torille extension — manual test page</h1>

      <p>
        A Solid page for exercising the installed browser extension. It does
        not start the demo's built-in checker, so every underline you see here
        comes from the extension.
      </p>

      <div class="note">
        <strong>Open over HTTP, not <code>file://</code>.</strong> Content
        scripts do not run on <code>file://</code> pages by default. From the
        demo server this page lives at{' '}
        <code>http://127.0.0.1:8765/extension-test.html</code>.
      </div>

      <div class="note">
        <strong>What should happen:</strong> supported boxes get a shadow-DOM
        overlay with red wavy underlines under misspellings (try{' '}
        <code>kirjoitusvirhee</code>, <code>taloo</code>, <code>koirra</code>,{' '}
        <code>xyzzy</code>). Click an underlined word to see suggestions.
        Password and number fields should be left alone.
      </div>

      <div class="note">
        <strong>Search fields are off by default.</strong> Enable{' '}
        <em>Tarkasta hakukentät</em> in the extension's options page to check
        search boxes such as <code>input type=search</code>, YouTube's search
        bar, and fields whose name or placeholder says “search”.
      </div>

      <div class="toolbar">
        <button type="button" onClick={() => withDebug(true)}>
          Enable debug log + reload
        </button>
        <button type="button" onClick={() => withDebug(false)}>
          Disable debug + reload
        </button>
        <button class="primary" type="button" onClick={fillAll}>
          Fill every box with errors
        </button>
        <button type="button" onClick={clearAll}>
          Clear every box
        </button>
        <button type="button" onClick={() => setCover(!cover())}>
          Toggle cover banner (z-index test)
        </button>
      </div>

      <h2>1. Plain fields</h2>
      <div class="grid">
        <Card label="Textarea (block)">
          <AutoTextarea value="Kirjoitusvirhee talossa. Koira juoksee. Tämä lause on oikein." />
        </Card>
        <Card label="Tall textarea (scrolls)">
          <AutoTextarea
            class="tall"
            value={`Ensimmäinen rivi on oikein.
Toisella rivillä on kirjoitusvirhee.
Kolmannella rivillä on taloo ja koirra.
Neljännellä rivillä on xyzzy.
Viidennellä rivillä on riisryynipuuro.
Kuudennella rivillä on selittämmin.
Seitsemännellä rivillä on pitkä teksti, joka kietoutuu usealle riville, jotta näet menevätkö alleviivaukset tekstin mukana eivätkä jää jälkeen.

Kahdeksas rivi.
Yhdeksäs rivi.
Kymmenes rivi.`}
          />
        </Card>
        <Card label="input type=text">
          <input type="text" value="kirjoitusvirhee taloo" />
        </Card>
        <Card label="input type=search (enable search fields)">
          <input type="search" value="koirra xyzzy" />
        </Card>
        <Card label="input type=email">
          <input type="email" value="matti.meikalainen@esimerkki.fi" />
        </Card>
        <Card label="input type=url">
          <input type="url" value="https://esimerkki.fi/kirjoitusvirhee" />
        </Card>
        <Card label="input without type">
          <input value="taloo" />
        </Card>
        <Card label="input type=password (must be ignored)">
          <input type="password" value="salasana" />
        </Card>
        <Card label="input type=number (must be ignored)">
          <input type="number" value="12345" />
        </Card>
        <Card label="Empty input (type here)">
          <input type="text" placeholder="Kirjoita tähän virheellisesti..." />
        </Card>
      </div>

      <h2>2. Contenteditable</h2>
      <div class="grid">
        <Card label="contenteditable div">
          <div class="editable" contentEditable={true}>
            Kirjoitusvirhee talossa. Koira juoksee.
          </div>
        </Card>
        <Card label={'contenteditable="" (empty attribute)'}>
          <div
            class="editable"
            ref={(el) => el.setAttribute('contenteditable', '')}
          >
            taloo ja koirra ja xyzzy
          </div>
        </Card>
        <Card label="contenteditable=false (must be ignored)">
          <div class="editable" contentEditable={false}>
            kirjoitusvirhee taloo
          </div>
        </Card>
      </div>

      <h2>3. Layout edge cases</h2>
      <div class="grid">
        <Card label="Inside a flex row with a button">
          <div class="flex-row">
            <input type="text" value="kirjoitusvirhee taloo" />
            <button type="button">Send</button>
          </div>
        </Card>
        <Card label="Inside a label element">
          <label class="inline-field">
            <span>Nimi</span>
            <input type="text" value="koirra xyzzy" />
          </label>
        </Card>
        <Card label="Auto-growing textarea">
          <AutoTextarea rows={1} value="kirjoitusvirhee taloo" />
        </Card>
        <Card label="YouTube-like search input (enable search fields)">
          <input
            type="search"
            value="youtube search bar"
            style={{
              height: '40px',
              'line-height': '24px',
              padding: '2px 6px',
              'border-radius': '20px',
            }}
          />
        </Card>
        <Card label="Tall search input (enable search fields)">
          <input
            type="search"
            value="size limit for firefox extensions"
            style={{
              height: '38px',
              'line-height': '19px',
            }}
          />
        </Card>
        <Card label="Input with a custom font">
          <input
            type="text"
            value="kirjoitusvirhee taloo"
            style={{
              font: '20px/1.8 Georgia, serif',
              'letter-spacing': '.04em',
            }}
          />
        </Card>
        <Card label="Input with a datalist dropdown">
          <input type="text" list="torille-test-words" value="koirra xyzzy" />
          <datalist id="torille-test-words">
            <option value="koira" />
            <option value="talo" />
            <option value="kirjoitusvirhe" />
          </datalist>
        </Card>
        <Card label="select dropdown (must be ignored)">
          <select>
            <option>kirjoitusvirhee</option>
            <option>taloo</option>
            <option>koirra</option>
          </select>
        </Card>
        <Card label="Scrolled textarea">
          <AutoTextarea
            class="tall"
            value={`Rivi 1.
Rivi 2.
Rivi 3.
Rivi 4.
Rivi 5.
Rivi 6.
Rivi 7.
Rivi 8.
Rivi 9.
Rivi 10 — kirjoitusvirhee.
Rivi 11.
Rivi 12.`}
          />
        </Card>
      </div>

      <h2>4. Dynamic boxes</h2>
      <p class="muted">
        These are created after page load, like an SPA or a comment box that
        mounts on click. The extension's MutationObserver should attach to each
        one.
      </p>
      <div class="toolbar">
        <button type="button" onClick={() => add('textarea')}>
          Add textarea
        </button>
        <button type="button" onClick={() => add('input')}>
          Add input
        </button>
        <button type="button" onClick={() => add('editable')}>
          Add contenteditable
        </button>
        <button class="primary" type="button" onClick={() => addMixed(3)}>
          Add mixed (1 of each)
        </button>
        <button type="button" onClick={() => addMixed(20)}>
          Add 20 mixed
        </button>
        <button
          type="button"
          onClick={() => {
            note('delayed add armed (1 s)');
            window.setTimeout(() => add('textarea'), 1000);
          }}
        >
          Add after 1 s
        </button>
        <button type="button" onClick={clearDynamic}>
          Remove all dynamic
        </button>
      </div>
      <div class="grid">
        <For each={boxes()}>{(box) => <DynamicBoxView box={box} />}</For>
      </div>

      <h2>5. Dynamic inside a shadow root (known limitation)</h2>
      <p class="muted">
        The scanner does not pierce shadow roots, so the box inside this host is
        expected <strong>not</strong> to be underlined. It is here to make the
        limitation visible, not as a pass/fail check.
      </p>
      <div class="shadow-host" ref={(el) => (shadowHost = el)} />

      <h2>6. Iframe</h2>
      <p class="muted">
        The manifest runs content scripts in all frames. The textarea inside
        this same-origin iframe should behave like the top-level ones.
      </p>
      <iframe
        src="extension-test-frame.html"
        title="iframe test"
        style={{
          width: '100%',
          height: '180px',
          border: '1px solid var(--line)',
          'border-radius': '8px',
        }}
      />

      <h2>7. Diagnostics</h2>
      <pre id="log">{log().join('\n')}</pre>

      <Show when={cover()}>
        <div class="cover-banner">
          Cover banner (z-index: 10) — underlines must stay underneath
        </div>
      </Show>

      <div class="status">
        <span>
          inputs: <strong>{inputs()}</strong>
        </span>
        <span>
          overlay hosts: <strong>{hosts()}</strong>
        </span>
        <span>
          underlines: <strong>{misses()}</strong>
        </span>
        <span>
          debug: <strong>{debug() ? 'on' : 'off'}</strong>
        </span>
      </div>
    </>
  );
}

render(() => <App />, document.getElementById('app')!);
