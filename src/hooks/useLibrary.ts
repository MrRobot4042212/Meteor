'use client';

import { useCallback, useEffect, useRef, useState } from 'react';
import { listen } from '@tauri-apps/api/event';
import type { Game, Category, PlayStat } from '@/lib/types';
import {
  getLibrary,
  cachedLibrary,
  resolveCover,
  listCategories,
  appIcon,
  allPlaytime,
} from '@/lib/tauri';

/** Resolve covers a few at a time to stay under IGDB's ~4 req/s rate limit. */
const COVER_CONCURRENCY = 3;

export function useLibrary() {
  const [games, setGames] = useState<Game[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  // Explicitly-created categories with icons (persist even with zero games).
  const [categoryMeta, setCategoryMeta] = useState<Category[]>([]);
  // Play stats per game id, for sorting by played/recent (live-updated).
  const [playtimes, setPlaytimes] = useState<Record<string, PlayStat>>({});
  // First-run splash: true until the library loads and the initial cover pass
  // finishes. `coverProgress` drives the splash's progress bar.
  const [booting, setBooting] = useState(true);
  const [coverProgress, setCoverProgress] = useState({ done: 0, total: 0 });
  const booted = useRef(false);
  // Bumped on each refresh so a stale in-flight cover pass can bail out.
  const runId = useRef(0);

  const resolveCovers = useCallback(async (list: Game[], myRun: number) => {
    // IGDB is a *games* database, so resolving covers for apps yields wrong art
    // (e.g. the Brave browser → the movie "Brave"). Apps use their exe icon
    // instead; only fetch covers for entries that still lack one and aren't apps.
    const pending = list.filter((g) => !g.cover_url && g.source !== 'app');
    setCoverProgress({ done: 0, total: pending.length });
    let i = 0;
    let done = 0;

    const worker = async () => {
      while (i < pending.length) {
        const game = pending[i++];
        if (runId.current !== myRun) return; // a newer refresh superseded us
        try {
          const url = await resolveCover(game.name);
          if (url && runId.current === myRun) {
            setGames((prev) =>
              prev.map((g) => (g.id === game.id ? { ...g, cover_url: url } : g)),
            );
          }
        } catch {
          // Leave the placeholder; one missing cover shouldn't break the rest.
        } finally {
          done++;
          if (runId.current === myRun) setCoverProgress({ done, total: pending.length });
        }
      }
    };

    await Promise.all(
      Array.from({ length: Math.min(COVER_CONCURRENCY, pending.length) }, worker),
    );
  }, []);

  // Resolve real exe icons for apps that have no cover and no known brand logo.
  // Local extraction (no network/rate limit), so a higher concurrency is fine.
  const resolveIcons = useCallback(async (list: Game[], myRun: number) => {
    const pending = list.filter(
      (g) => g.source === 'app' && !g.cover_url && !g.icon && g.executable,
    );
    let i = 0;
    const worker = async () => {
      while (i < pending.length) {
        const game = pending[i++];
        if (runId.current !== myRun) return;
        try {
          const path = await appIcon(game.executable as string);
          if (path && runId.current === myRun) {
            setGames((prev) =>
              prev.map((g) => (g.id === game.id ? { ...g, icon: path } : g)),
            );
          }
        } catch {
          // No icon is fine: the card falls back to the letter placeholder.
        }
      }
    };
    await Promise.all(Array.from({ length: Math.min(6, pending.length) }, worker));
  }, []);

  const refresh = useCallback(
    async (showSplash = false) => {
      const myRun = ++runId.current;
      // Splash shows on the very first run, or whenever explicitly requested
      // (e.g. the "volver a escanear" button) so a re-scan feels like a reload.
      const splash = showSplash || !booted.current;
      if (splash) {
        setBooting(true);
        setCoverProgress({ done: 0, total: 0 });
      }
      setLoading(true);
      setError(null);
      try {
        const list = await getLibrary();
        if (runId.current !== myRun) return;
        setGames(list);
        setLoading(false);
        resolveIcons(list, myRun);
        const pass = resolveCovers(list, myRun);
        if (splash) {
          pass.finally(() => {
            if (runId.current === myRun) {
              booted.current = true;
              setBooting(false);
            }
          });
        }
      } catch (e) {
        if (runId.current === myRun) {
          setError(String(e));
          setLoading(false);
          booted.current = true;
          setBooting(false);
        }
      }
    },
    [resolveCovers, resolveIcons],
  );

  const refreshCategories = useCallback(async () => {
    try {
      setCategoryMeta(await listCategories());
    } catch {
      // Non-fatal: the sidebar just won't show empty categories.
    }
  }, []);

  const refreshPlaytimes = useCallback(async () => {
    try {
      setPlaytimes(await allPlaytime());
    } catch {
      // Non-fatal: sorting by playtime just falls back to zeros.
    }
  }, []);

  // Keep play stats fresh: reload when the global watcher closes a session.
  useEffect(() => {
    const un = listen('playtime-updated', () => refreshPlaytimes());
    return () => {
      un.then((f) => f());
    };
  }, [refreshPlaytimes]);

  useEffect(() => {
    let cancelled = false;
    (async () => {
      // Paint the last cached library instantly (no splash), then refresh in the
      // background. If there's no cache, fall through to the splash-on-first-run.
      try {
        const cache = await cachedLibrary();
        if (!cancelled && cache.length) {
          setGames(cache);
          setLoading(false);
          booted.current = true;
          setBooting(false);
        }
      } catch {
        // No cache: refresh() below will show the first-run splash.
      }
      if (!cancelled) refresh();
    })();
    refreshCategories();
    refreshPlaytimes();
    return () => {
      cancelled = true;
    };
  }, [refresh, refreshCategories, refreshPlaytimes]);

  return {
    games,
    loading,
    error,
    playtimes,
    refresh,
    setGames,
    categoryMeta,
    refreshCategories,
    booting,
    coverProgress,
  };
}
