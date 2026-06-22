'use client';

import { useCallback, useEffect, useRef, useState } from 'react';
import type { Game, Category } from '@/lib/types';
import { getLibrary, resolveCover, listCategories } from '@/lib/tauri';

/** Resolve covers a few at a time to stay under IGDB's ~4 req/s rate limit. */
const COVER_CONCURRENCY = 3;

export function useLibrary() {
  const [games, setGames] = useState<Game[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  // Explicitly-created categories with icons (persist even with zero games).
  const [categoryMeta, setCategoryMeta] = useState<Category[]>([]);
  // First-run splash: true until the library loads and the initial cover pass
  // finishes. `coverProgress` drives the splash's progress bar.
  const [booting, setBooting] = useState(true);
  const [coverProgress, setCoverProgress] = useState({ done: 0, total: 0 });
  const booted = useRef(false);
  // Bumped on each refresh so a stale in-flight cover pass can bail out.
  const runId = useRef(0);

  const resolveCovers = useCallback(async (list: Game[], myRun: number) => {
    const pending = list.filter((g) => !g.cover_url);
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
    [resolveCovers],
  );

  const refreshCategories = useCallback(async () => {
    try {
      setCategoryMeta(await listCategories());
    } catch {
      // Non-fatal: the sidebar just won't show empty categories.
    }
  }, []);

  useEffect(() => {
    refresh();
    refreshCategories();
  }, [refresh, refreshCategories]);

  return {
    games,
    loading,
    error,
    refresh,
    setGames,
    categoryMeta,
    refreshCategories,
    booting,
    coverProgress,
  };
}
