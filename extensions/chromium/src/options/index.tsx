// Solid settings page for the torille extension. Built by esbuild with
// esbuild-plugin-solid (see build.mjs); the static shell lives in
// options.html and only provides #app plus the stylesheet.
import { For, Show, createSignal, onMount } from 'solid-js';
import { render } from 'solid-js/web';

type Settings = {
  underlineStyle: string;
  underlineColor: string;
  grammarUnderlines: boolean;
  grammarErrorColor: string;
  grammarWarningColor: string;
  grammarSuggestionColor: string;
  styleUnderlines: boolean;
  styleColor: string;
  suggestionsMax: number;
  popupTrigger: string;
  popupBg: string;
  popupFg: string;
  popupAccent: string;
  popupAccentFg: string;
  ignoreCodeBlocks: boolean;
  ignoreInlineCode: boolean;
  ignoreBbcode: boolean;
  ignoreUrls: boolean;
  ignoreCodeEditors: boolean;
};

const DEFAULTS: Settings = {
  underlineStyle: 'wavy',
  underlineColor: '#cc3333',
  grammarUnderlines: true,
  grammarErrorColor: '#c2410c',
  grammarWarningColor: '#d97706',
  grammarSuggestionColor: '#b45309',
  styleUnderlines: true,
  styleColor: '#2563eb',
  suggestionsMax: 8,
  popupTrigger: 'click',
  popupBg: '#ffffff',
  popupFg: '#222222',
  popupAccent: '#0077cc',
  popupAccentFg: '#ffffff',
  ignoreCodeBlocks: true,
  ignoreInlineCode: true,
  ignoreBbcode: true,
  ignoreUrls: true,
  ignoreCodeEditors: true,
};

const KEYS: Record<keyof Settings, string> = {
  underlineStyle: 'torille-underline-style',
  underlineColor: 'torille-underline-color',
  grammarUnderlines: 'torille-grammar-underlines',
  grammarErrorColor: 'torille-grammar-error-color',
  grammarWarningColor: 'torille-grammar-warning-color',
  grammarSuggestionColor: 'torille-grammar-suggestion-color',
  styleUnderlines: 'torille-style-underlines',
  styleColor: 'torille-style-color',
  suggestionsMax: 'torille-suggestions-max',
  popupTrigger: 'torille-popup-trigger',
  popupBg: 'torille-popup-bg',
  popupFg: 'torille-popup-fg',
  popupAccent: 'torille-popup-accent',
  popupAccentFg: 'torille-popup-accent-fg',
  ignoreCodeBlocks: 'torille-ignore-code-blocks',
  ignoreInlineCode: 'torille-ignore-inline-code',
  ignoreBbcode: 'torille-ignore-bbcode',
  ignoreUrls: 'torille-ignore-urls',
  ignoreCodeEditors: 'torille-ignore-code-editors',
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
    grammarUnderlines: raw[KEYS.grammarUnderlines] !== false,
    grammarErrorColor: color('grammarErrorColor'),
    grammarWarningColor: color('grammarWarningColor'),
    grammarSuggestionColor: color('grammarSuggestionColor'),
    styleUnderlines: raw[KEYS.styleUnderlines] !== false,
    styleColor: color('styleColor'),
    suggestionsMax: Number.isFinite(max)
      ? Math.min(MAX, Math.max(MIN, Math.round(max))) : DEFAULTS.suggestionsMax,
    popupTrigger: ['click', 'hover'].includes(trigger)
      ? trigger : DEFAULTS.popupTrigger,
    popupBg: color('popupBg'),
    popupFg: color('popupFg'),
    popupAccent: color('popupAccent'),
    popupAccentFg: color('popupAccentFg'),
    ignoreCodeBlocks: raw[KEYS.ignoreCodeBlocks] !== false,
    ignoreInlineCode: raw[KEYS.ignoreInlineCode] !== false,
    ignoreBbcode: raw[KEYS.ignoreBbcode] !== false,
    ignoreUrls: raw[KEYS.ignoreUrls] !== false,
    ignoreCodeEditors: raw[KEYS.ignoreCodeEditors] !== false,
  };
}

function valuesOf(settings: Settings): Record<string, string | number | boolean> {
  const out: Record<string, string | number | boolean> = {};
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
        <h2>Kieliopin ja tyylin alleviivaukset</h2>
        <p class="lead">
          Nämä alleviivaukset piirretään vasta <strong>Tarkista kielioppi</strong>- tai{' '}
          <strong>Tarkista tyyli</strong> -komennon jälkeen. Ne tyhjenevät, kun tekstiä muokataan.
        </p>
        <label class="toggle">
          <input
            id="grammar-underlines"
            type="checkbox"
            checked={settings().grammarUnderlines}
            onChange={(e) => patch('grammarUnderlines', e.currentTarget.checked)}
          />
          <span>Kieliopin alleviivaukset</span>
        </label>
        <div class="settings-grid colors">
          <div class="row">
            <label for="grammar-error-color">Virheen väri</label>
            <input type="color" id="grammar-error-color"
              value={settings().grammarErrorColor}
              onInput={(e) => patch('grammarErrorColor', e.currentTarget.value)} />
          </div>
          <div class="row">
            <label for="grammar-warning-color">Varoituksen väri</label>
            <input type="color" id="grammar-warning-color"
              value={settings().grammarWarningColor}
              onInput={(e) => patch('grammarWarningColor', e.currentTarget.value)} />
          </div>
          <div class="row">
            <label for="grammar-suggestion-color">Ehdotuksen väri</label>
            <input type="color" id="grammar-suggestion-color"
              value={settings().grammarSuggestionColor}
              onInput={(e) => patch('grammarSuggestionColor', e.currentTarget.value)} />
          </div>
        </div>
        <label class="toggle">
          <input
            id="style-underlines"
            type="checkbox"
            checked={settings().styleUnderlines}
            onChange={(e) => patch('styleUnderlines', e.currentTarget.checked)}
          />
          <span>Tyylin alleviivaukset</span>
        </label>
        <div class="settings-grid single">
          <div class="row">
            <label for="style-color">Tyylin väri</label>
            <input type="color" id="style-color"
              value={settings().styleColor}
              onInput={(e) => patch('styleColor', e.currentTarget.value)} />
          </div>
        </div>
      </section>

      <section class="card">
        <h2>Tarkistuksen asetukset</h2>
        <label class="toggle">
          <input
            id="ignore-code-blocks"
            type="checkbox"
            checked={settings().ignoreCodeBlocks}
            onChange={(e) => patch('ignoreCodeBlocks', e.currentTarget.checked)}
          />
          <span>Ohita koodilohkot
            <small>Ohittaa <code>```</code>- ja <code>~~~</code>-merkinnöillä rajatut koodilohkot sekä HTML-elementtien <code>code</code>, <code>pre</code>, <code>kbd</code>, <code>samp</code>, <code>script</code> ja <code>style</code> sisällöt. Koodilohkot eivät myöskään vaikuta virkkeiden rajojen tunnistamiseen.</small>
          </span>
        </label>
        <label class="toggle">
          <input
            id="ignore-inline-code"
            type="checkbox"
            checked={settings().ignoreInlineCode}
            onChange={(e) => patch('ignoreInlineCode', e.currentTarget.checked)}
          />
          <span>Ohita rivinsisäinen koodi
            <small>Ohittaa tekstin seassa olevat <code>koodinpätkät</code> eikä tarkista niiden sisältöä.</small>
          </span>
        </label>
        <label class="toggle">
          <input
            id="ignore-bbcode"
            type="checkbox"
            checked={settings().ignoreBbcode}
            onChange={(e) => patch('ignoreBbcode', e.currentTarget.checked)}
          />
          <span>Ohita BBCode-merkinnät
            <small>Jättää koodi-, URL-, kuva-, sähköposti- ja mediasisällöt tarkistuksen ulkopuolelle. Lainaukset ja tekstin muotoilut, kuten <code>[quote]</code>, <code>[b]</code> ja <code>[i]</code>, tarkistetaan normaalisti.</small>
          </span>
        </label>
        <label class="toggle">
          <input
            id="ignore-urls"
            type="checkbox"
            checked={settings().ignoreUrls}
            onChange={(e) => patch('ignoreUrls', e.currentTarget.checked)}
          />
          <span>Ohita verkko-osoitteet ja sähköpostiosoitteet
            <small>Ei tarkista verkko-osoitteita tai sähköpostiosoitteita, kuten <code>https://example.fi/talo</code> tai <code>matti@example.fi</code>. Myös www-osoitteet sekä BBCode-merkintöjen URL- ja sähköpostikentät ohitetaan.</small>
          </span>
        </label>
        <label class="toggle">
          <input
            id="ignore-code-editors"
            type="checkbox"
            checked={settings().ignoreCodeEditors}
            onChange={(e) => patch('ignoreCodeEditors', e.currentTarget.checked)}
          />
          <span>Ohita koodieditorit
            <small>Poistaa tarkistuksen käytöstä CodeMirror-, Monaco- ja Ace-koodieditoreissa. Oletuksena käytössä on ohitus.</small>
          </span>
        </label>
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
            <small>Ottaa tarkistuksen käyttöön myös hakukentissä. Oletuksena pois käytöstä, sillä hakukentissä käytetään usein nimiä, osoitteita ja hakusanoja, joita sanakirja ei tunnista.</small>
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
          <span>Ota debug-loki käyttöön
            <small>Kirjaa selaimen konsoliin tietoja tunnistetuista tekstikentistä, tarkistuspyynnöistä ja kontekstivalikon toiminnoista.</small>
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
