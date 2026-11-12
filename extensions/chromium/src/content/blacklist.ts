import { api, type DebugFn } from './shared';

// ─────────────────────────────────────────────────────────────────────
// §6  Blacklist factory
// ─────────────────────────────────────────────────────────────────────

// Per-host blacklist. Loaded once on boot; refreshed whenever the
// storage changes (options page edit or context-menu site toggle).
// When a host comes off the list we re-scan the document so any
// inputs the scanner skipped on first pass get attached.
export function createBlacklist(opts: {
  dbg: DebugFn;
  refreshOverlays: (opts: { reScan: boolean }) => void;
}): { load: () => Promise<void>; isSiteBlacklisted: () => boolean } {
  const { refreshOverlays } = opts;
  const myHostname = location.hostname;
  let blacklist = new Set<string>();

  function isSiteBlacklisted(): boolean {
    return blacklist.has(myHostname);
  }

  async function load(): Promise<void> {
    const cur = await api.storage.local.get('blacklist');
    blacklist = new Set(Array.isArray(cur.blacklist) ? cur.blacklist : []);
    opts.dbg('content', 'blacklist loaded',
        { count: blacklist.size, blacklisted: isSiteBlacklisted() });
    // First refresh: scan only if the host isn't blacklisted. If it
    // is, `refreshOverlays` will hide whatever the scanner found
    // via MutationObserver before the load resolved (unlikely but
    // possible on a SPA mount).
    refreshOverlays({ reScan: !isSiteBlacklisted() });
  }

  if (api.storage?.onChanged) {
    api.storage.onChanged.addListener((changes, area) => {
      if (area === 'local' && changes.blacklist) {
        blacklist = new Set((changes.blacklist.newValue as string[] | undefined) || []);
        opts.dbg('content', 'storage.onChanged blacklist',
            { newCount: blacklist.size });
        refreshOverlays({ reScan: !isSiteBlacklisted() });
      }
    });
  }

  return { load, isSiteBlacklisted };
}
