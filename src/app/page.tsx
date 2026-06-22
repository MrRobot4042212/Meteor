'use client';

import { useEffect, useMemo, useState } from 'react';
import { useLibrary } from '@/hooks/useLibrary';
import { launchGame, removeGame, hideGame, setFavorite, setCategories } from '@/lib/tauri';
import type { Game, Category } from '@/lib/types';
import { Sidebar, type Filter } from '@/components/Sidebar';
import { GameCard } from '@/components/GameCard';
import { AddAppDialog } from '@/components/AddAppDialog';
import { SettingsDialog } from '@/components/SettingsDialog';
import { CoverDialog } from '@/components/CoverDialog';
import { CategoryDialog } from '@/components/CategoryDialog';
import { NewCategoryDialog } from '@/components/NewCategoryDialog';
import { Splash } from '@/components/Splash';
import { DetailView } from '@/components/DetailView';
import { SOURCE_ORDER } from '@/lib/sources';
import { SearchIcon, PlusIcon, RefreshIcon } from '@/components/icons';

export default function Page() {
  const {
    games,
    loading,
    error,
    refresh,
    setGames,
    categoryMeta,
    refreshCategories,
    booting,
    coverProgress,
  } = useLibrary();
  const [splashDone, setSplashDone] = useState(false);
  const [filter, setFilter] = useState<Filter>('all');
  const [query, setQuery] = useState('');
  const [showAdd, setShowAdd] = useState(false);
  const [showSettings, setShowSettings] = useState(false);
  const [editingCover, setEditingCover] = useState<Game | null>(null);
  const [editingCategories, setEditingCategories] = useState<Game | null>(null);
  const [showNewCategory, setShowNewCategory] = useState(false);
  const [dragging, setDragging] = useState(false);
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [toast, setToast] = useState<string | null>(null);

  // The game whose detail page is open (kept fresh from `games` by id).
  const selected = selectedId ? games.find((g) => g.id === selectedId) ?? null : null;

  // Categories shown in the sidebar: explicitly-created ones (with icons, persist
  // even when empty) plus any in use across the library, deduped by name and
  // alphabetically sorted. The explicit entry wins so its icon is kept.
  const categories = useMemo(() => {
    const byName = new Map<string, Category>();
    for (const c of categoryMeta) byName.set(c.name, c);
    for (const g of games) {
      for (const c of g.categories ?? []) {
        if (!byName.has(c)) byName.set(c, { name: c, icon: null });
      }
    }
    return [...byName.values()].sort((a, b) => a.name.localeCompare(b.name));
  }, [games, categoryMeta]);

  const categoryNames = useMemo(() => categories.map((c) => c.name), [categories]);

  const counts = useMemo<Record<string, number>>(() => {
    const base: Record<string, number> = {
      all: games.length,
      favorites: games.filter((g) => g.favorite).length,
    };
    for (const source of SOURCE_ORDER) {
      base[source] = games.filter((g) => g.source === source).length;
    }
    for (const name of categoryNames) {
      base[`cat:${name}`] = games.filter((g) => g.categories?.includes(name)).length;
    }
    return base;
  }, [games, categoryNames]);

  const visible = useMemo(() => {
    const q = query.trim().toLowerCase();
    return games
      .filter((g) => {
        if (filter === 'all') return true;
        if (filter === 'favorites') return !!g.favorite;
        if (filter.startsWith('cat:')) return g.categories?.includes(filter.slice(4)) ?? false;
        return g.source === filter;
      })
      .filter((g) => (q ? g.name.toLowerCase().includes(q) : true));
  }, [games, filter, query]);

  // If the active filter disappears from the sidebar (its last game left the
  // favorites/category), fall back to "Todo" instead of an empty, hidden view.
  useEffect(() => {
    if (filter.startsWith('cat:') && !categoryNames.includes(filter.slice(4))) {
      setFilter('all');
    }
  }, [filter, categoryNames]);

  // Esc closes the detail page.
  useEffect(() => {
    if (!selected) return;
    const onKey = (e: KeyboardEvent) => {
      if (e.key === 'Escape') setSelectedId(null);
    };
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  }, [selected]);

  function flash(msg: string) {
    setToast(msg);
    window.setTimeout(() => setToast(null), 2500);
  }

  // Re-scan from scratch, showing the splash again (reset any earlier skip).
  function handleRescan() {
    setSplashDone(false);
    refresh(true);
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

  async function handleHide(game: Game) {
    setGames((prev) => prev.filter((g) => g.id !== game.id));
    try {
      await hideGame(game.id);
    } catch {
      refresh();
    }
  }

  async function handleToggleFavorite(game: Game) {
    const next = !game.favorite;
    setGames((prev) =>
      prev.map((g) => (g.id === game.id ? { ...g, favorite: next } : g)),
    );
    try {
      await setFavorite(game.id, next);
    } catch {
      refresh();
    }
  }

  // A game card was dragged onto Favoritos or a category in the sidebar.
  async function handleDropGame(target: Filter, gameId: string) {
    const game = games.find((g) => g.id === gameId);
    if (!game) return;

    if (target === 'favorites') {
      if (game.favorite) return;
      setGames((prev) =>
        prev.map((g) => (g.id === gameId ? { ...g, favorite: true } : g)),
      );
      flash(`${game.name} → Favoritos`);
      try {
        await setFavorite(gameId, true);
      } catch {
        refresh();
      }
      return;
    }

    if (target.startsWith('cat:')) {
      const name = target.slice(4);
      const current = game.categories ?? [];
      if (current.some((c) => c.toLowerCase() === name.toLowerCase())) return;
      const next = [...current, name];
      setGames((prev) =>
        prev.map((g) => (g.id === gameId ? { ...g, categories: next } : g)),
      );
      flash(`${game.name} → ${name}`);
      try {
        await setCategories(gameId, next);
      } catch {
        refresh();
      }
    }
  }

  return (
    <div className="flex h-screen w-screen overflow-hidden bg-void text-ink">
      {booting && !splashDone && (
        <Splash progress={coverProgress} onSkip={() => setSplashDone(true)} />
      )}

      <Sidebar
        filter={filter}
        onFilter={setFilter}
        counts={counts}
        categories={categories}
        onAddCategory={() => setShowNewCategory(true)}
        onDropGame={handleDropGame}
        isDragging={dragging}
        onOpenSettings={() => setShowSettings(true)}
      />

      <main className="flex min-w-0 flex-1 flex-col">
        {selected ? (
          <DetailView
            game={selected}
            onBack={() => setSelectedId(null)}
            onLaunch={handleLaunch}
            onToggleFavorite={handleToggleFavorite}
            onEditCover={setEditingCover}
            onEditCategories={setEditingCategories}
            onRemove={
              selected.source === 'manual'
                ? (g) => {
                    handleRemove(g);
                    setSelectedId(null);
                  }
                : undefined
            }
            onHide={
              selected.source !== 'manual'
                ? (g) => {
                    handleHide(g);
                    setSelectedId(null);
                  }
                : undefined
            }
          />
        ) : (
          <>
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
            onClick={handleRescan}
            title="Volver a escanear"
            className="grid h-10 w-10 place-items-center rounded-lg border border-line bg-surface text-muted transition hover:text-ink"
          >
            <RefreshIcon className={`h-[18px] w-[18px] ${loading ? 'animate-spin' : ''}`} />
          </button>

          <button
            onClick={() => setShowAdd(true)}
            className="flex items-center gap-2 rounded-lg bg-accent px-4 py-2.5 text-sm font-semibold text-white transition hover:bg-accent-soft"
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
              action={{ label: 'Reintentar', onClick: handleRescan }}
            />
          ) : visible.length === 0 ? (
            <Empty
              title={query ? 'Sin resultados' : 'Biblioteca vacía'}
              body={
                query
                  ? 'Prueba con otro término de búsqueda.'
                  : 'Abre tus tiendas (Steam, Epic, GOG…) al menos una vez o añade una app manualmente.'
              }
              action={query ? undefined : { label: 'Añadir app', onClick: () => setShowAdd(true) }}
            />
          ) : (
            <div className="grid grid-cols-[repeat(auto-fill,minmax(240px,1fr))] gap-6">
              {visible.map((game) => (
                <GameCard
                  key={game.id}
                  game={game}
                  onLaunch={handleLaunch}
                  onRemove={game.source === 'manual' ? handleRemove : undefined}
                  onHide={game.source === 'manual' ? undefined : handleHide}
                  onEditCover={setEditingCover}
                  onToggleFavorite={handleToggleFavorite}
                  onEditCategories={setEditingCategories}
                  onDragStateChange={setDragging}
                  onOpen={(g) => setSelectedId(g.id)}
                />
              ))}
            </div>
          )}
            </section>
          </>
        )}
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

      {showSettings && (
        <SettingsDialog
          onClose={() => setShowSettings(false)}
          onChanged={() => {
            flash('Actualizando biblioteca…');
            refresh();
          }}
        />
      )}

      {editingCover && (
        <CoverDialog
          game={editingCover}
          onClose={() => setEditingCover(null)}
          onSaved={(id, url) => {
            setGames((prev) =>
              prev.map((g) => (g.id === id ? { ...g, cover_url: url } : g)),
            );
            flash(url ? 'Carátula actualizada' : 'Carátula restablecida');
          }}
        />
      )}

      {editingCategories && (
        <CategoryDialog
          game={editingCategories}
          allCategories={categoryNames}
          onClose={() => setEditingCategories(null)}
          onSaved={(id, cats) => {
            setGames((prev) =>
              prev.map((g) => (g.id === id ? { ...g, categories: cats } : g)),
            );
            flash('Categorías actualizadas');
          }}
        />
      )}

      {showNewCategory && (
        <NewCategoryDialog
          existing={categoryNames}
          onClose={() => setShowNewCategory(false)}
          onCreated={async (name) => {
            // Await the reload so `categories` includes the new name before we
            // switch the filter — otherwise the empty-filter guard would bounce
            // us back to "Todo" on the next render.
            await refreshCategories();
            setFilter(`cat:${name}`);
            flash(`Categoría «${name}» creada`);
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
    <div className="grid grid-cols-[repeat(auto-fill,minmax(240px,1fr))] gap-6">
      {Array.from({ length: 12 }).map((_, i) => (
        <div
          key={i}
          className="aspect-[2/3] animate-pulse rounded-2xl border border-line bg-elevated"
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
            className="rounded-lg bg-accent px-4 py-2.5 text-sm font-semibold text-white hover:bg-accent-soft"
          >
            {action.label}
          </button>
        )}
      </div>
    </div>
  );
}
