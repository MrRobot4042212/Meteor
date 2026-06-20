'use client';

import { useMemo, useState } from 'react';
import { useLibrary } from '@/hooks/useLibrary';
import { launchGame, removeGame } from '@/lib/tauri';
import type { Game } from '@/lib/types';
import { Sidebar, type Filter } from '@/components/Sidebar';
import { GameCard } from '@/components/GameCard';
import { AddAppDialog } from '@/components/AddAppDialog';
import { SearchIcon, PlusIcon, RefreshIcon } from '@/components/icons';

export default function Page() {
  const { games, loading, error, refresh, setGames } = useLibrary();
  const [filter, setFilter] = useState<Filter>('all');
  const [query, setQuery] = useState('');
  const [showAdd, setShowAdd] = useState(false);
  const [toast, setToast] = useState<string | null>(null);

  const counts = useMemo<Record<Filter, number>>(
    () => ({
      all: games.length,
      steam: games.filter((g) => g.source === 'steam').length,
      manual: games.filter((g) => g.source === 'manual').length,
    }),
    [games],
  );

  const visible = useMemo(() => {
    const q = query.trim().toLowerCase();
    return games
      .filter((g) => (filter === 'all' ? true : g.source === filter))
      .filter((g) => (q ? g.name.toLowerCase().includes(q) : true));
  }, [games, filter, query]);

  function flash(msg: string) {
    setToast(msg);
    window.setTimeout(() => setToast(null), 2500);
  }

  async function handleLaunch(game: Game) {
    try {
      await launchGame(game);
      flash(`Iniciando ${game.name}…`);
    } catch (e) {
      flash(`No se pudo iniciar: ${e}`);
    }
  }

  async function handleRemove(game: Game) {
    setGames((prev) => prev.filter((g) => g.id !== game.id));
    try {
      await removeGame(game.id);
    } catch {
      refresh();
    }
  }

  return (
    <div className="flex h-screen w-screen overflow-hidden bg-void text-ink">
      <Sidebar filter={filter} onFilter={setFilter} counts={counts} />

      <main className="flex min-w-0 flex-1 flex-col">
        {/* Top bar */}
        <header className="flex items-center gap-3 border-b border-line px-6 py-4">
          <div className="relative flex-1 max-w-md">
            <SearchIcon className="pointer-events-none absolute left-3 top-1/2 h-[18px] w-[18px] -translate-y-1/2 text-muted" />
            <input
              value={query}
              onChange={(e) => setQuery(e.target.value)}
              placeholder="Buscar en tu biblioteca…"
              className="w-full rounded-lg border border-line bg-surface py-2.5 pl-10 pr-3 text-sm text-ink outline-none placeholder:text-muted/60 focus:border-accent/50"
            />
          </div>

          <button
            onClick={refresh}
            title="Volver a escanear"
            className="grid h-10 w-10 place-items-center rounded-lg border border-line bg-surface text-muted transition hover:text-ink"
          >
            <RefreshIcon className={`h-[18px] w-[18px] ${loading ? 'animate-spin' : ''}`} />
          </button>

          <button
            onClick={() => setShowAdd(true)}
            className="flex items-center gap-2 rounded-lg bg-accent px-4 py-2.5 text-sm font-semibold text-void transition hover:bg-accent-soft"
          >
            <PlusIcon className="h-[18px] w-[18px]" />
            Añadir
          </button>
        </header>

        {/* Content */}
        <section className="min-h-0 flex-1 overflow-y-auto px-6 py-6">
          {loading && games.length === 0 ? (
            <SkeletonGrid />
          ) : error && games.length === 0 ? (
            <Empty
              title="No se pudo cargar la biblioteca"
              body={error}
              action={{ label: 'Reintentar', onClick: refresh }}
            />
          ) : visible.length === 0 ? (
            <Empty
              title={query ? 'Sin resultados' : 'Biblioteca vacía'}
              body={
                query
                  ? 'Prueba con otro término de búsqueda.'
                  : 'Abre Steam al menos una vez o añade una app manualmente.'
              }
              action={query ? undefined : { label: 'Añadir app', onClick: () => setShowAdd(true) }}
            />
          ) : (
            <div className="grid grid-cols-[repeat(auto-fill,minmax(150px,1fr))] gap-4">
              {visible.map((game) => (
                <GameCard
                  key={game.id}
                  game={game}
                  onLaunch={handleLaunch}
                  onRemove={game.source === 'manual' ? handleRemove : undefined}
                />
              ))}
            </div>
          )}
        </section>
      </main>

      {showAdd && (
        <AddAppDialog
          onClose={() => setShowAdd(false)}
          onAdded={(g) => {
            setGames((prev) =>
              [...prev, g].sort((a, b) => a.name.localeCompare(b.name)),
            );
            flash(`${g.name} añadido`);
          }}
        />
      )}

      {toast && (
        <div className="fixed bottom-6 left-1/2 -translate-x-1/2 rounded-lg border border-line bg-elevated px-4 py-2.5 text-sm text-ink shadow-card">
          {toast}
        </div>
      )}
    </div>
  );
}

function SkeletonGrid() {
  return (
    <div className="grid grid-cols-[repeat(auto-fill,minmax(150px,1fr))] gap-4">
      {Array.from({ length: 12 }).map((_, i) => (
        <div
          key={i}
          className="aspect-[2/3] animate-pulse rounded-xl2 border border-line bg-elevated"
        />
      ))}
    </div>
  );
}

function Empty({
  title,
  body,
  action,
}: {
  title: string;
  body: string;
  action?: { label: string; onClick: () => void };
}) {
  return (
    <div className="grid h-full place-items-center text-center">
      <div className="max-w-sm">
        <h3 className="mb-2 font-display text-lg font-semibold text-ink">{title}</h3>
        <p className="mb-5 text-sm leading-relaxed text-muted">{body}</p>
        {action && (
          <button
            onClick={action.onClick}
            className="rounded-lg bg-accent px-4 py-2.5 text-sm font-semibold text-void hover:bg-accent-soft"
          >
            {action.label}
          </button>
        )}
      </div>
    </div>
  );
}
