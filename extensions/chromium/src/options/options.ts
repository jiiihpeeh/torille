// Options page: manage the per-host blacklist and the extension-wide
// debug flag.
//
// File layout:
//   §1  Element refs + browser shim + dbg helper
//   §2  Blacklist (read / write / add / remove / render)
//   §3  Debug toggle (read / write)
//   §4  Storage event dispatch (blacklist + debug)
//   §5  Bootstrap

(() => {
  'use strict';

// ───────────────────────────────────────────────────────────────────────
// §1  Element refs + browser shim + dbg helper
// ───────────────────────────────────────────────────────────────────────

const listEl = document.getElementById('list') as HTMLUListElement;
const emptyEl = document.getElementById('empty') as HTMLDivElement;
const form = document.getElementById('add') as HTMLFormElement;
const input = document.getElementById('host') as HTMLInputElement;
const debugEl = document.getElementById('debug') as HTMLInputElement;
const searchboxesEl = document.getElementById('searchboxes') as HTMLInputElement;

// Appearance / popup settings. Keys are shared with the content script
// and the Firefox build; changing them here applies live to open tabs.
const SETTINGS = {
  underlineStyle: 'torille-underline-style',
  underlineColor: 'torille-underline-color',
  suggestionsMax: 'torille-suggestions-max',
  popupTrigger: 'torille-popup-trigger',
  popupBg: 'torille-popup-bg',
  popupFg: 'torille-popup-fg',
  popupAccent: 'torille-popup-accent',
  popupAccentFg: 'torille-popup-accent-fg',
} as const;

const DEFAULTS = {
  underlineStyle: 'wavy',
  underlineColor: '#cc3333',
  suggestionsMax: 8,
  popupTrigger: 'click',
  popupBg: '#ffffff',
  popupFg: '#222222',
  popupAccent: '#0077cc',
  popupAccentFg: '#ffffff',
} as const;

const underlineStyleEl = document.getElementById('underline-style') as HTMLSelectElement;
const underlineColorEl = document.getElementById('underline-color') as HTMLInputElement;
const suggestionsMaxEl = document.getElementById('suggestions-max') as HTMLInputElement;
const popupTriggerEl = document.getElementById('popup-trigger') as HTMLSelectElement;
const popupBgEl = document.getElementById('popup-bg') as HTMLInputElement;
const popupFgEl = document.getElementById('popup-fg') as HTMLInputElement;
const popupAccentEl = document.getElementById('popup-accent') as HTMLInputElement;
const popupAccentFgEl = document.getElementById('popup-accent-fg') as HTMLInputElement;
const settingsStatusEl = document.getElementById('settings-status') as HTMLDivElement;
const resetSettingsEl = document.getElementById('reset-settings') as HTMLButtonElement;

const api: typeof browser = (typeof browser !== 'undefined') ? browser : chrome;

const dbg = (...args: unknown[]): void => {
  if (typeof console !== 'undefined') console.log('[torille:options]', ...args);
};

// ───────────────────────────────────────────────────────────────────────
// §2  Blacklist
// ───────────────────────────────────────────────────────────────────────

// Strip scheme/path/trailing-dot from whatever the user pasted.
// Hostnames are case-insensitive.
function normalize(host: string): string {
  let h = host.trim().toLowerCase();
  if (h.startsWith('http://') || h.startsWith('https://')) {
    try { h = new URL(h).hostname; } catch (_) { /* keep as-is */ }
  }
  const slash = h.indexOf('/');
  if (slash >= 0) h = h.slice(0, slash);
  if (h.endsWith('.')) h = h.slice(0, -1);
  return h;
}

const readBlacklist = async (): Promise<string[]> => {
  const cur = await api.storage.local.get('blacklist');
  return Array.isArray(cur.blacklist) ? cur.blacklist : [];
};
const writeBlacklist = async (list: string[]): Promise<void> => {
  await api.storage.local.set({ blacklist: list });
};

function renderBlacklist(list: string[]): void {
  listEl.textContent = '';
  if (!list.length) {
    emptyEl.hidden = false;
    return;
  }
  emptyEl.hidden = true;
  for (const host of list) {
    const li = document.createElement('li');
    const span = document.createElement('span');
    span.textContent = host;
    li.appendChild(span);
    const btn = document.createElement('button');
    btn.type = 'button';
    btn.className = 'ghost danger';
    btn.textContent = 'Poista';
    btn.addEventListener('click', async () => {
      const cur = await readBlacklist();
      await writeBlacklist(cur.filter((h) => h !== host));
    });
    li.appendChild(btn);
    listEl.appendChild(li);
  }
}

async function addHost(raw: string): Promise<boolean> {
  const host = normalize(raw);
  if (!host || !host.includes('.')) return false;
  const cur = await readBlacklist();
  if (cur.includes(host)) return true; // already there
  await writeBlacklist([...cur, host]);
  return true;
}

form.addEventListener('submit', async (e: Event) => {
  e.preventDefault();
  const ok = await addHost(input.value);
  if (!ok) { input.focus(); return; }
  input.value = '';
});

// ───────────────────────────────────────────────────────────────────────
// §3  Search-box and debug toggles
// ───────────────────────────────────────────────────────────────────────

const SEARCH_KEY = 'torille-searchboxes';

const readSearchboxes = async (): Promise<boolean> => {
  const cur = await api.storage.local.get(SEARCH_KEY);
  return cur[SEARCH_KEY] === true;
};
const writeSearchboxes = async (on: boolean): Promise<void> => {
  await api.storage.local.set({ [SEARCH_KEY]: !!on });
};

function renderSearchboxes(checked: boolean): void {
  searchboxesEl.checked = !!checked;
}

searchboxesEl.addEventListener('change', async () => {
  await writeSearchboxes(searchboxesEl.checked);
  dbg('searchboxes toggle ->', searchboxesEl.checked);
});

const DEBUG_KEY = 'torille-debug';

const readDebug = async (): Promise<boolean> => {
  const cur = await api.storage.local.get(DEBUG_KEY);
  return cur[DEBUG_KEY] === true;
};
const writeDebug = async (on: boolean): Promise<void> => {
  await api.storage.local.set({ [DEBUG_KEY]: !!on });
};

function renderDebug(checked: boolean): void {
  debugEl.checked = !!checked;
}

debugEl.addEventListener('change', async () => {
  await writeDebug(debugEl.checked);
  dbg('debug toggle ->', debugEl.checked);
});

// ───────────────────────────────────────────────────────────────────────
// §3b Appearance and popup settings
// ───────────────────────────────────────────────────────────────────────

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

const isHexColor = (value: unknown): value is string =>
  typeof value === 'string' && /^#[0-9a-f]{6}$/i.test(value);

const readSettings = async (): Promise<Settings> => {
  const keyList = Object.values(SETTINGS);
  const cur = await api.storage.local.get(keyList);
  const style = String(cur[SETTINGS.underlineStyle] ?? DEFAULTS.underlineStyle);
  const trigger = String(cur[SETTINGS.popupTrigger] ?? DEFAULTS.popupTrigger);
  const max = Number(cur[SETTINGS.suggestionsMax] ?? DEFAULTS.suggestionsMax);
  return {
    underlineStyle: ['wavy', 'solid', 'dashed', 'dotted', 'none'].includes(style)
      ? style : DEFAULTS.underlineStyle,
    underlineColor: isHexColor(cur[SETTINGS.underlineColor])
      ? cur[SETTINGS.underlineColor] : DEFAULTS.underlineColor,
    suggestionsMax: Number.isFinite(max)
      ? Math.min(20, Math.max(3, Math.round(max))) : DEFAULTS.suggestionsMax,
    popupTrigger: ['click', 'hover'].includes(trigger)
      ? trigger : DEFAULTS.popupTrigger,
    popupBg: isHexColor(cur[SETTINGS.popupBg])
      ? cur[SETTINGS.popupBg] : DEFAULTS.popupBg,
    popupFg: isHexColor(cur[SETTINGS.popupFg])
      ? cur[SETTINGS.popupFg] : DEFAULTS.popupFg,
    popupAccent: isHexColor(cur[SETTINGS.popupAccent])
      ? cur[SETTINGS.popupAccent] : DEFAULTS.popupAccent,
    popupAccentFg: isHexColor(cur[SETTINGS.popupAccentFg])
      ? cur[SETTINGS.popupAccentFg] : DEFAULTS.popupAccentFg,
  };
};

function renderSettings(s: Settings): void {
  underlineStyleEl.value = s.underlineStyle;
  underlineColorEl.value = s.underlineColor;
  suggestionsMaxEl.value = String(s.suggestionsMax);
  popupTriggerEl.value = s.popupTrigger;
  popupBgEl.value = s.popupBg;
  popupFgEl.value = s.popupFg;
  popupAccentEl.value = s.popupAccent;
  popupAccentFgEl.value = s.popupAccentFg;
}

function flashSaved(message: string, error = false): void {
  settingsStatusEl.textContent = message;
  settingsStatusEl.dataset.error = error ? '1' : '';
  if (!error) {
    window.setTimeout(() => {
      if (settingsStatusEl.textContent === message) settingsStatusEl.textContent = '';
    }, 1500);
  }
}

async function saveSettings(): Promise<void> {
  const max = Math.min(
    20,
    Math.max(3, Math.round(Number(suggestionsMaxEl.value) || DEFAULTS.suggestionsMax)),
  );
  suggestionsMaxEl.value = String(max);
  const values = {
    [SETTINGS.underlineStyle]: underlineStyleEl.value,
    [SETTINGS.underlineColor]: underlineColorEl.value,
    [SETTINGS.suggestionsMax]: max,
    [SETTINGS.popupTrigger]: popupTriggerEl.value,
    [SETTINGS.popupBg]: popupBgEl.value,
    [SETTINGS.popupFg]: popupFgEl.value,
    [SETTINGS.popupAccent]: popupAccentEl.value,
    [SETTINGS.popupAccentFg]: popupAccentFgEl.value,
  };
  try {
    await api.storage.local.set(values);
    flashSaved('Tallennettu');
    dbg('settings saved', values);
  } catch (err) {
    flashSaved('Tallennus epäonnistui', true);
    dbg('settings save failed', err);
  }
}

for (const el of [underlineStyleEl, suggestionsMaxEl, popupTriggerEl]) {
  el.addEventListener('change', () => { void saveSettings(); });
}
for (const el of [underlineColorEl, popupBgEl, popupFgEl, popupAccentEl, popupAccentFgEl]) {
  el.addEventListener('input', () => { void saveSettings(); });
}

resetSettingsEl.addEventListener('click', async () => {
  renderSettings({ ...DEFAULTS });
  await saveSettings();
  flashSaved('Oletusasetukset palautettu');
  dbg('settings reset to defaults');
});

// ───────────────────────────────────────────────────────────────────────
// §4  Storage event dispatch
// ───────────────────────────────────────────────────────────────────────

// Storage-onChanged writes from any source (the background page
// toggling the current host, an extra options tab editing the flag)
// should be reflected in the UI.
api.storage.onChanged.addListener((changes, area) => {
  if (area !== 'local') return;
  if (changes.blacklist) {
    renderBlacklist((changes.blacklist.newValue as string[] | undefined) || []);
    dbg('storage.onChanged blacklist ->', changes.blacklist.newValue || []);
  }
  if (changes[DEBUG_KEY]) {
    renderDebug(!!changes[DEBUG_KEY].newValue);
    dbg('storage.onChanged debug ->', changes[DEBUG_KEY].newValue);
  }
  if (changes[SEARCH_KEY]) {
    renderSearchboxes(!!changes[SEARCH_KEY].newValue);
    dbg('storage.onChanged searchboxes ->', changes[SEARCH_KEY].newValue);
  }
  if (Object.values(SETTINGS).some((key) => key in changes)) {
    void readSettings().then(renderSettings);
  }
});

// ───────────────────────────────────────────────────────────────────────
// §5  Bootstrap
// ───────────────────────────────────────────────────────────────────────

Promise.all([readBlacklist(), readDebug(), readSearchboxes(), readSettings()]).then(
  ([list, debugOn, searchboxesOn, settings]) => {
    renderBlacklist(list);
    renderDebug(debugOn);
    renderSearchboxes(searchboxesOn);
    renderSettings(settings);
    dbg('loaded', {
      size: list.length, debug: debugOn, searchboxes: searchboxesOn,
    });
  },
);
})();