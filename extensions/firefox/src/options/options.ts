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
});

// ───────────────────────────────────────────────────────────────────────
// §5  Bootstrap
// ───────────────────────────────────────────────────────────────────────

Promise.all([readBlacklist(), readDebug(), readSearchboxes()]).then(
  ([list, debugOn, searchboxesOn]) => {
    renderBlacklist(list);
    renderDebug(debugOn);
    renderSearchboxes(searchboxesOn);
    dbg('loaded', {
      size: list.length, debug: debugOn, searchboxes: searchboxesOn,
    });
  },
);
})();