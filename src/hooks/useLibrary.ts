'use client';

import { useCallback, useEffect, useState } from 'react';
import type { Game } from '@/lib/types';
import { getLibrary } from '@/lib/tauri';

export function useLibrary() {
  const [games, setGames] = useState<Game[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const refresh = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      setGames(await getLibrary());
    } catch (e) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    refresh();
  }, [refresh]);

  return { games, loading, error, refresh, setGames };
}
