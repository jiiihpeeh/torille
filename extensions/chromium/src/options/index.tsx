// Solid settings page for the torille extension. Built by esbuild with
// esbuild-plugin-solid (see build.mjs); the static shell lives in
// options.html and only provides #app plus the stylesheet.
import { For, Show, createSignal, onMount } from 'solid-js';
import { render } from 'solid-js/web';

type Settings = {
  underlineStyle: string;
  underlineColor: string;
  suggestionsMax: number;
  popupTrigger: string;
  popupBg: string;
  popupFg: string;
  popupAccent: string;
  popupAccentFg: string;
};

const DEFAULTS: Settings = {
  underlineStyle: 'wavy',
  underlineColor: '#cc3333',
  suggestionsMax: 8,
  popupTrigger: 'click',
  popupBg: '#ffffff',
  popupFg: '#222222',
  popupAccent: '#0077cc',
  popupAccentFg: '#ffffff',
};

const KEYS: Record<keyof Settings, string> = {
  underlineStyle: 'torille-underline-style',
  underlineColor: 'torille-underline-color',
  suggestionsMax: 'torille-suggestions-max',
  popupTrigger: 'torille-popup-trigger',
  popupBg: 'torille-popup-bg',
  popupFg: 'torille-popup-fg',
  popupAccent: 'torille-popup-accent',
  popupAccentFg: 'torille-popup-accent-fg',
};

const HEX = /^#[0-9a-f]{6}$/i;
const MAX = 20;
const MIN = 3;

declare const browser: any;
declare const chrome: any;

const api = typeof browser !== 'undefined' ? browser : chrome;

function normalizeHost(raw: string): string {
  let host = raw.trim().toLowerCase();
  if (host.startsWith('http://') || host.startsWith('https://')) {
    try { host = new URL(host).hostname; } catch { /* keep */ }
  }
  const slash = host.indexOf('/');
  if (slash >= 0) host = host.slice(0, slash);
  if (host.endsWith('.')) host = host.slice(0, -1);
  return host;
}

function readSettings(raw: Record<string, unknown>): Settings {
  const pick = (key: keyof Settings, fallback: string): string => {
    const value = raw[KEYS[key]];
    return typeof value === 'string' && value ? value : fallback;
  };
  const style = pick('underlineStyle', DEFAULTS.underlineStyle);
  const trigger = pick('popupTrigger', DEFAULTS.popupTrigger);
  const max = Number(raw[KEYS.suggestionsMax] ?? DEFAULTS.suggestionsMax);
  const color = (key: keyof Settings): string => {
    const value = pick(key, DEFAULTS[key] as string);
    return HEX.test(value) ? value : (DEFAULTS[key] as string);
  };
  return {
    underlineStyle: ['wavy', 'solid', 'dashed', 'dotted', 'none'].includes(style)
      ? style : DEFAULTS.underlineStyle,
    underlineColor: color('underlineColor'),
    suggestionsMax: Number.isFinite(max)
      ? Math.min(MAX, Math.max(MIN, Math.round(max))) : DEFAULTS.suggestionsMax,
    popupTrigger: ['click', 'hover'].includes(trigger)
      ? trigger : DEFAULTS.popupTrigger,
    popupBg: color('popupBg'),
    popupFg: color('popupFg'),
    popupAccent: color('popupAccent'),
    popupAccentFg: color('popupAccentFg'),
  };
}

function valuesOf(settings: Settings): Record<string, string | number> {
  const out: Record<string, string | number> = {};
  for (const key of Object.keys(KEYS) as (keyof Settings)[]) {
    out[KEYS[key]] = settings[key];
  }
  return out;
}

function OptionsApp() {
  const [settings, setSettings] = createSignal<Settings>({ ...DEFAULTS });
  const [status, setStatus] = createSignal('');
  const [blacklist, setBlacklist] = createSignal<string[]>([]);
  const [debug, setDebug] = createSignal(false);
  const [searchboxes, setSearchboxes] = createSignal(false);
  const [host, setHost] = createSignal('');
  let statusTimer: number | undefined;

  function flash(message: string, error = false): void {
    setStatus(message);
    statusTimer && window.clearTimeout(statusTimer);
    if (!error) {
      statusTimer = window.setTimeout(() => setStatus(''), 1600);
    }
  }

  async function persist(next: Settings, message = 'Tallennettu'): Promise<void> {
    setSettings(next);
    try {
      await api.storage.local.set(valuesOf(next));
      flash(message);
    } catch (err) {
      console.error('[torille:options] save failed', err);
      flash('Tallennus epäonnistui', true);
    }
  }

  function patch<K extends keyof Settings>(key: K, value: Settings[K]): void {
    void persist({ ...settings(), [key]: value });
  }

  async function reset(): Promise<void> {
    await persist({ ...DEFAULTS }, 'Oletusasetukset palautettu');
  }

  async function addHost(event: Event): Promise<void> {
    event.preventDefault();
    const value = normalizeHost(host());
    if (!value || !value.includes('.')) return;
    const list = blacklist();
    if (!list.includes(value)) {
      const next = [...list, value];
      await api.storage.local.set({ blacklist: next });
      setBlacklist(next);
    }
    setHost('');
  }

  async function removeHost(value: string): Promise<void> {
    const next = blacklist().filter((item) => item !== value);
    await api.storage.local.set({ blacklist: next });
    setBlacklist(next);
  }

  onMount(async () => {
    const raw = await api.storage.local.get([
      ...Object.values(KEYS),
      'blacklist',
      'torille-debug',
      'torille-searchboxes',
    ]);
    setSettings(readSettings(raw));
    setBlacklist(Array.isArray(raw.blacklist) ? raw.blacklist : []);
    setDebug(raw['torille-debug'] === true);
    setSearchboxes(raw['torille-searchboxes'] === true);

    api.storage.onChanged.addListener((changes: Record<string, any>, area: string) => {
      if (area !== 'local') return;
      if (Object.values(KEYS).some((key) => key in changes)) {
        void api.storage.local.get([...Object.values(KEYS)]).then((cur: any) => {
          setSettings(readSettings(cur));
        });
      }
      if (changes.blacklist) {
        setBlacklist(Array.isArray(changes.blacklist.newValue)
          ? changes.blacklist.newValue : []);
      }
      if (changes['torille-debug']) {
        setDebug(changes['torille-debug'].newValue === true);
      }
      if (changes['torille-searchboxes']) {
        setSearchboxes(changes['torille-searchboxes'].newValue === true);
      }
    });
  });

  return (
    <>
      <header>
        <h1>torille <small>suomalainen oikeinkirjoitus</small></h1>
        <span class="badge">🇫🇮 Suomi</span>
      </header>

      <section class="card">
        <h2>Ulkoasu ja pudotusvalikko</h2>
        <p class="lead">
          Nämä asetukset vaikuttavat heti kaikkiin avoimiin välilehtiin.
          Valinnat tallennetaan automaattisesti.
        </p>
        <div class="settings-grid">
          <div class="row">
            <label for="underline-style">Alleviivauksen tyyli</label>
            <select
              id="underline-style"
              value={settings().underlineStyle}
              onChange={(e) => patch('underlineStyle', e.currentTarget.value)}
            >
              <option value="wavy">aaltoviiva</option>
              <option value="solid">yhtenäinen</option>
              <option value="dashed">katkoviiva</option>
              <option value="dotted">pisteviiva</option>
              <option value="none">ei alleviivausta</option>
            </select>
          </div>
          <div class="row">
            <label for="underline-color">Alleviivauksen väri</label>
            <input
              type="color"
              id="underline-color"
              value={settings().underlineColor}
              onInput={(e) => patch('underlineColor', e.currentTarget.value)}
            />
          </div>
          <div class="row">
            <label for="suggestions-max">Ehdotusten enimmäismäärä
              <small>3–20</small>
            </label>
            <input
              type="number"
              id="suggestions-max"
              min={MIN}
              max={MAX}
              step="1"
              value={settings().suggestionsMax}
              onChange={(e) => patch('suggestionsMax', Math.min(
                MAX, Math.max(MIN, Math.round(Number(e.currentTarget.value) || DEFAULTS.suggestionsMax)),
              ))}
            />
          </div>
          <div class="row">
            <label for="popup-trigger">Avaa ehdotukset</label>
            <select
              id="popup-trigger"
              value={settings().popupTrigger}
              onChange={(e) => patch('popupTrigger', e.currentTarget.value)}
            >
              <option value="click">napsauttamalla</option>
              <option value="hover">osoittamalla</option>
            </select>
          </div>
          <div class="row">
            <label for="popup-bg">Valikon tausta</label>
            <input type="color" id="popup-bg" value={settings().popupBg}
              onInput={(e) => patch('popupBg', e.currentTarget.value)} />
          </div>
          <div class="row">
            <label for="popup-fg">Valikon teksti</label>
            <input type="color" id="popup-fg" value={settings().popupFg}
              onInput={(e) => patch('popupFg', e.currentTarget.value)} />
          </div>
          <div class="row">
            <label for="popup-accent">Korostuksen tausta</label>
            <input type="color" id="popup-accent" value={settings().popupAccent}
              onInput={(e) => patch('popupAccent', e.currentTarget.value)} />
          </div>
          <div class="row">
            <label for="popup-accent-fg">Korostuksen teksti</label>
            <input type="color" id="popup-accent-fg" value={settings().popupAccentFg}
              onInput={(e) => patch('popupAccentFg', e.currentTarget.value)} />
          </div>
        </div>
        <div class="settings-actions">
          <button type="button" id="reset-settings" class="ghost" onClick={() => void reset()}>
            Palauta oletusasetukset
          </button>
        </div>
        <div id="settings-status" class="status" role="status" aria-live="polite">{status()}</div>
      </section>

      <section class="card">
        <h2>Tarkastus</h2>
        <label class="toggle">
          <input
            id="searchboxes"
            type="checkbox"
            checked={searchboxes()}
            onChange={(e) => {
              const on = e.currentTarget.checked;
              setSearchboxes(on);
              void api.storage.local.set({ 'torille-searchboxes': on });
            }}
          />
          <span>Tarkasta hakukentät
            <small>Oletuksena pois: hakukenttiin kirjoitetaan usein nimiä, osoitteita ja hakusanoja, joita sanakirja ei tunne.</small>
          </span>
        </label>
        <label class="toggle">
          <input
            id="debug"
            type="checkbox"
            checked={debug()}
            onChange={(e) => {
              const on = e.currentTarget.checked;
              setDebug(on);
              void api.storage.local.set({ 'torille-debug': on });
            }}
          />
          <span>Debug-loki päälle
            <small>Kirjoittaa konsoliin tiedot aktivoitavista kentistä, tarkistuspyynnöistä ja kontekstivalikon toiminnoista.</small>
          </span>
        </label>
      </section>

      <section class="card">
        <h2>Sivustot, joilla ei tarkasteta</h2>
        <p class="lead">
          Yksittäisellä sivulla voit ottaa tarkastuksen käyttöön tai pois
          hiiren oikean napin valikon kautta.
        </p>
        <form id="add" class="add-row" onSubmit={(e) => void addHost(e)}>
          <input
            id="host"
            type="text"
            placeholder="esimerkki.fi"
            autocomplete="off"
            required
            value={host()}
            onInput={(e) => setHost(e.currentTarget.value)}
          />
          <button type="submit" class="primary">Lisää</button>
        </form>
        <ul id="list">
          <For each={blacklist()}>{(item) => (
            <li>
              <span>{item}</span>
              <button type="button" class="ghost danger"
                onClick={() => void removeHost(item)}>Poista</button>
            </li>
          )}</For>
        </ul>
        <Show when={blacklist().length === 0}>
          <div id="empty" class="empty">Blacklist on tyhjä.</div>
        </Show>
      </section>

      <footer class="meta">
        <span>torille — suomalainen oikeinkirjoitustarkistus</span>
        <span>
          <a href="https://gitlab.com/jiiihpeeh/torille" target="_blank" rel="noopener noreferrer">
            Lähdekoodi GitLabissa
          </a>
        </span>
      </footer>
    </>
  );
}

render(() => <OptionsApp />, document.getElementById('app')!);
