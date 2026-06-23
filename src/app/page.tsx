'use client';

import { useEffect, useMemo, useState } from 'react';
import { listen } from '@tauri-apps/api/event';
import { useLibrary } from '@/hooks/useLibrary';
import {
  launchGame,
  removeGame,
  hideGame,
  setFavorite,
  setCategories,
  openPath,
  removeCategory,
  setCategoryOrder,
} from '@/lib/tauri';
import type { Game, Category } from '@/lib/types';
import { Sidebar, type Filter } from '@/components/Sidebar';
import { GameCard } from '@/components/GameCard';
import { ContextMenu, type MenuItem } from '@/components/ContextMenu';
import { BulkCategoryDialog } from '@/components/BulkCategoryDialog';
import { AddAppDialog } from '@/components/AddAppDialog';
import { SettingsDialog } from '@/components/SettingsDialog';
import { CoverDialog } from '@/components/CoverDialog';
import { CategoryDialog } from '@/components/CategoryDialog';
import { NewCategoryDialog } from '@/components/NewCategoryDialog';
import { EditCategoryDialog } from '@/components/EditCategoryDialog';
import { Splash } from '@/components/Splash';
import { Spotlight } from '@/components/Spotlight';
import { Footer } from '@/components/Footer';
import { ConfirmDialog } from '@/components/ConfirmDialog';
import { DetailView } from '@/components/DetailView';
import { Home } from '@/components/Home';
import { UpdatePrompt } from '@/components/UpdatePrompt';
import { SOURCE_ORDER } from '@/lib/sources';
import { fuzzyScore } from '@/lib/fuzzy';
import {
  SearchIcon,
  PlusIcon,
  RefreshIcon,
  PlayIcon,
  StarIcon,
  TagIcon,
  ImageIcon,
  EyeOffIcon,
  TrashIcon,
  FolderIcon,
  PencilIcon,
} from '@/components/icons';

type SortKey = 'name' | 'played' | 'recent';
const SORT_LABELS: Record<SortKey, string> = {
  name: 'Nombre (A-Z)',
  played: 'Más jugados',
  recent: 'Jugados recientemente',
};

/** Folder to reveal for a game: its install dir, else the exe's parent. */
function folderOf(game: Game): string | null {
  if (game.install_dir) return game.install_dir;
  const exe = game.executable;
  if (!exe) return null;
  const i = Math.max(exe.lastIndexOf('\\'), exe.lastIndexOf('/'));
  return i > 0 ? exe.slice(0, i) : null;
}

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
    playtimes,
  } = useLibrary();
  const [splashDone, setSplashDone] = useState(false);
  const [filter, setFilter] = useState<Filter>('home');
  const [query, setQuery] = useState('');
  const [sort, setSort] = useState<SortKey>('name');
  const [menu, setMenu] = useState<{ x: number; y: number; items: MenuItem[] } | null>(null);
  const [selectMode, setSelectMode] = useState(false);
  const [selectedIds, setSelectedIds] = useState<Set<string>>(new Set());
  const [showBulkCats, setShowBulkCats] = useState(false);
  const [spotlight, setSpotlight] = useState(false);
  const [confirm, setConfirm] = useState<{
    title: string;
    message: React.ReactNode;
    confirmLabel: string;
    onConfirm: () => void;
  } | null>(null);
  const [showAdd, setShowAdd] = useState(false);
  const [showSettings, setShowSettings] = useState(false);
  const [editingCover, setEditingCover] = useState<Game | null>(null);
  const [editingCategories, setEditingCategories] = useState<Game | null>(null);
  const [showNewCategory, setShowNewCategory] = useState(false);
  const [editingCategory, setEditingCategory] = useState<Category | null>(null);
  const [dragging, setDragging] = useState(false);
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [toast, setToast] = useState<string | null>(null);

  // The game whose detail page is open (kept fresh from `games` by id).
  const selected = selectedId ? games.find((g) => g.id === selectedId) ?? null : null;

  // The dashboard replaces the grid on the "Inicio" filter (unless searching).
  const showingHome = filter === 'home' && !query.trim();

  // Categories shown in the sidebar: explicitly-created ones first, in their saved
  // order (with icons, persist even when empty), then any in-use-only ones
  // appended alphabetically. The explicit entry wins so its icon/order are kept.
  const categories = useMemo(() => {
    const result: Category[] = [];
    const seen = new Set<string>();
    for (const c of categoryMeta) {
      const key = c.name.toLowerCase();
      if (!seen.has(key)) {
        result.push(c);
        seen.add(key);
      }
    }
    const inUse = new Set<string>();
    for (const g of games) for (const c of g.categories ?? []) inUse.add(c);
    const extra = [...inUse]
      .filter((c) => !seen.has(c.toLowerCase()))
      .sort((a, b) => a.localeCompare(b));
    for (const name of extra) result.push({ name, icon: null });
    return result;
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
    const inFilter = games.filter((g) => {
      // Home has no own grid; when a query is typed there we search everything.
      if (filter === 'all' || filter === 'home') return true;
      if (filter === 'favorites') return !!g.favorite;
      if (filter.startsWith('cat:')) return g.categories?.includes(filter.slice(4)) ?? false;
      return g.source === filter;
    });

    // A query takes over ordering: fuzzy-match and rank by score.
    const q = query.trim();
    if (q) {
      return inFilter
        .map((g) => ({ g, s: fuzzyScore(q, g.name) }))
        .filter((x): x is { g: Game; s: number } => x.s !== null)
        .sort((a, b) => b.s - a.s)
        .map((x) => x.g);
    }

    // Otherwise apply the chosen sort (ties broken by name).
    const secs = (id: string) => playtimes[id]?.seconds ?? 0;
    const last = (id: string) => playtimes[id]?.last_played ?? 0;
    const byName = (a: Game, b: Game) => a.name.localeCompare(b.name);
    const arr = [...inFilter];
    if (sort === 'played') arr.sort((a, b) => secs(b.id) - secs(a.id) || byName(a, b));
    else if (sort === 'recent') arr.sort((a, b) => last(b.id) - last(a.id) || byName(a, b));
    else arr.sort(byName);
    return arr;
  }, [games, filter, query, sort, playtimes]);

  // Items for the right-click context menu on a card.
  function menuItems(game: Game): MenuItem[] {
    const items: MenuItem[] = [
      { label: 'Jugar', icon: <PlayIcon className="h-4 w-4" />, onClick: () => handleLaunch(game) },
      {
        label: game.favorite ? 'Quitar de favoritos' : 'Marcar como favorito',
        icon: <StarIcon className="h-4 w-4" fill={game.favorite ? 'currentColor' : 'none'} />,
        onClick: () => handleToggleFavorite(game),
      },
      { label: 'Categorías…', icon: <TagIcon className="h-4 w-4" />, onClick: () => setEditingCategories(game) },
      { label: 'Cambiar carátula…', icon: <ImageIcon className="h-4 w-4" />, onClick: () => setEditingCover(game) },
    ];
    const folder = folderOf(game);
    if (folder) {
      items.push({
        label: 'Abrir carpeta',
        icon: <FolderIcon className="h-4 w-4" />,
        onClick: () => {
          openPath(folder).catch(() => flash('No se pudo abrir la carpeta'));
        },
      });
    }
    items.push({ type: 'separator' });
    if (game.source === 'manual') {
      items.push({ label: 'Quitar', danger: true, icon: <TrashIcon className="h-4 w-4" />, onClick: () => handleRemove(game) });
    } else {
      items.push({ label: 'Ocultar', danger: true, icon: <EyeOffIcon className="h-4 w-4" />, onClick: () => handleHide(game) });
    }
    return items;
  }

  // Items for the right-click context menu on a custom category.
  function categoryMenuItems(cat: Category): MenuItem[] {
    return [
      {
        label: 'Editar…',
        icon: <PencilIcon className="h-4 w-4" />,
        onClick: () => setEditingCategory(cat),
      },
      { type: 'separator' },
      {
        label: 'Eliminar',
        danger: true,
        icon: <TrashIcon className="h-4 w-4" />,
        onClick: () => handleDeleteCategory(cat),
      },
    ];
  }

  // If the active filter disappears from the sidebar (its last game left the
  // favorites/category), fall back to "Todo" instead of an empty, hidden view.
  useEffect(() => {
    if (filter.startsWith('cat:') && !categoryNames.includes(filter.slice(4))) {
      setFilter('all');
    }
  }, [filter, categoryNames]);

  // Global Spotlight: the Rust global shortcut emits this when triggered.
  useEffect(() => {
    const un = listen('open-spotlight', () => setSpotlight(true));
    return () => {
      un.then((f) => f());
    };
  }, []);

  // Esc closes the detail page, or exits multi-select.
  useEffect(() => {
    if (!selected && !selectMode) return;
    const onKey = (e: KeyboardEvent) => {
      if (e.key !== 'Escape') return;
      if (selected) setSelectedId(null);
      else if (selectMode) exitSelection();
    };
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  }, [selected, selectMode]);

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

  // --- Category management --------------------------------------------------
  async function handleReorderCategories(names: string[]) {
    try {
      await setCategoryOrder(names);
      await refreshCategories();
    } catch {
      flash('No se pudo reordenar las categorías');
    }
  }

  function handleDeleteCategory(cat: Category) {
    setConfirm({
      title: 'Eliminar categoría',
      message: (
        <>
          ¿Eliminar la categoría <span className="text-ink">«{cat.name}»</span>? Se quitará de
          todos los juegos (los juegos no se borran).
        </>
      ),
      confirmLabel: 'Eliminar',
      onConfirm: () => doDeleteCategory(cat),
    });
  }

  async function doDeleteCategory(cat: Category) {
    try {
      await removeCategory(cat.name);
      if (filter === `cat:${cat.name}`) setFilter('all');
      await refreshCategories();
      refresh();
      flash(`Categoría «${cat.name}» eliminada`);
    } catch {
      flash('No se pudo eliminar la categoría');
    }
  }

  // --- Multi-select ---------------------------------------------------------
  function toggleSelect(game: Game) {
    setSelectMode(true);
    setSelectedIds((prev) => {
      const next = new Set(prev);
      next.has(game.id) ? next.delete(game.id) : next.add(game.id);
      return next;
    });
  }

  function exitSelection() {
    setSelectMode(false);
    setSelectedIds(new Set());
  }

  async function bulkFavorite(value: boolean) {
    const ids = [...selectedIds];
    setGames((prev) =>
      prev.map((g) => (selectedIds.has(g.id) ? { ...g, favorite: value } : g)),
    );
    await Promise.allSettled(ids.map((id) => setFavorite(id, value)));
    flash(value ? `${ids.length} en favoritos` : `${ids.length} quitados de favoritos`);
  }

  function bulkHide() {
    const count = selectedIds.size;
    setConfirm({
      title: 'Ocultar selección',
      message: (
        <>
          ¿Ocultar <span className="text-ink">{count}</span>{' '}
          {count === 1 ? 'elemento' : 'elementos'}? Podrás restaurarlos desde Ajustes.
        </>
      ),
      confirmLabel: 'Ocultar',
      onConfirm: doBulkHide,
    });
  }

  async function doBulkHide() {
    const ids = [...selectedIds];
    setGames((prev) => prev.filter((g) => !selectedIds.has(g.id)));
    exitSelection();
    const res = await Promise.allSettled(ids.map((id) => hideGame(id)));
    if (res.some((r) => r.status === 'rejected')) refresh();
    flash(`${ids.length} ${ids.length === 1 ? 'oculto' : 'ocultos'}`);
  }

  async function bulkAddCategories(cats: string[]) {
    const ids = [...selectedIds];
    setGames((prev) =>
      prev.map((g) => {
        if (!selectedIds.has(g.id)) return g;
        const current = g.categories ?? [];
        const merged = [...current];
        for (const c of cats) {
          if (!merged.some((x) => x.toLowerCase() === c.toLowerCase())) merged.push(c);
        }
        return { ...g, categories: merged };
      }),
    );
    const games_ = games.filter((g) => selectedIds.has(g.id));
    await Promise.allSettled(
      games_.map((g) => {
        const current = g.categories ?? [];
        const merged = [...current];
        for (const c of cats) {
          if (!merged.some((x) => x.toLowerCase() === c.toLowerCase())) merged.push(c);
        }
        return setCategories(g.id, merged);
      }),
    );
    flash(`Categorías añadidas a ${ids.length}`);
  }

  function handleRemove(game: Game) {
    setConfirm({
      title: 'Quitar de la biblioteca',
      message: (
        <>
          ¿Quitar <span className="text-ink">«{game.name}»</span> de la biblioteca?
        </>
      ),
      confirmLabel: 'Quitar',
      onConfirm: () => doRemove(game),
    });
  }

  async function doRemove(game: Game) {
    if (selectedId === game.id) setSelectedId(null);
    setGames((prev) => prev.filter((g) => g.id !== game.id));
    try {
      await removeGame(game.id);
    } catch {
      refresh();
    }
  }

  function handleHide(game: Game) {
    setConfirm({
      title: 'Ocultar de la biblioteca',
      message: (
        <>
          ¿Ocultar <span className="text-ink">«{game.name}»</span>? Podrás restaurarlo desde
          Ajustes.
        </>
      ),
      confirmLabel: 'Ocultar',
      onConfirm: () => doHide(game),
    });
  }

  async function doHide(game: Game) {
    if (selectedId === game.id) setSelectedId(null);
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
    <div className="flex h-screen w-screen flex-col overflow-hidden bg-void text-ink">
      {booting && !splashDone && (
        <Splash progress={coverProgress} onSkip={() => setSplashDone(true)} />
      )}

      <div className="flex min-h-0 flex-1">
      <Sidebar
        filter={filter}
        onFilter={setFilter}
        counts={counts}
        categories={categories}
        onAddCategory={() => setShowNewCategory(true)}
        onDropGame={handleDropGame}
        isDragging={dragging}
        onOpenSettings={() => setShowSettings(true)}
        onCategoryContextMenu={(cat, x, y) => setMenu({ x, y, items: categoryMenuItems(cat) })}
        onReorderCategories={handleReorderCategories}
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
            onRemove={selected.source === 'manual' ? handleRemove : undefined}
            onHide={selected.source !== 'manual' ? handleHide : undefined}
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

          {/* Sort (disabled while searching: results are ranked by relevance).
              Hidden on the dashboard, which has no grid to sort. */}
          {!showingHome && (
            <>
              <select
                value={sort}
                onChange={(e) => setSort(e.target.value as SortKey)}
                disabled={!!query.trim()}
                title="Ordenar"
                className="h-10 rounded-lg border border-line bg-surface px-3 text-sm text-ink outline-none transition hover:text-ink focus:border-accent/50 disabled:opacity-40"
              >
                {(Object.keys(SORT_LABELS) as SortKey[]).map((k) => (
                  <option key={k} value={k}>
                    {SORT_LABELS[k]}
                  </option>
                ))}
              </select>

              <button
                onClick={() => (selectMode ? exitSelection() : setSelectMode(true))}
                className={`h-10 rounded-lg border px-3 text-sm transition ${
                  selectMode
                    ? 'border-accent bg-accent/15 text-ink'
                    : 'border-line bg-surface text-muted hover:text-ink'
                }`}
              >
                {selectMode ? 'Cancelar' : 'Seleccionar'}
              </button>
            </>
          )}

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
          {filter === 'home' && !query.trim() ? (
            <Home
              games={games}
              playtimes={playtimes}
              onOpen={(g) => setSelectedId(g.id)}
              onLaunch={handleLaunch}
            />
          ) : loading && games.length === 0 ? (
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
                  onContextMenu={(g, x, y) => setMenu({ x, y, items: menuItems(g) })}
                  selectionMode={selectMode}
                  selected={selectedIds.has(game.id)}
                  onToggleSelect={toggleSelect}
                />
              ))}
            </div>
          )}
            </section>
          </>
        )}
      </main>
      </div>

      <Footer />

      <UpdatePrompt />

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

      {editingCategory && (
        <EditCategoryDialog
          category={editingCategory}
          existing={categoryNames}
          onClose={() => setEditingCategory(null)}
          onSaved={async () => {
            const old = editingCategory.name;
            await refreshCategories();
            refresh();
            // If the active filter pointed at the renamed/merged category, it may
            // no longer exist by that exact name; fall back to "Todo".
            if (filter === `cat:${old}`) setFilter('all');
            flash('Categoría actualizada');
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

      {/* Bulk action bar (multi-select) */}
      {selectMode && selectedIds.size > 0 && (
        <div className="fixed bottom-14 left-1/2 z-50 flex -translate-x-1/2 items-center gap-2 rounded-xl2 border border-line bg-elevated px-3 py-2 shadow-card">
          <span className="px-2 text-sm font-medium text-ink">
            {selectedIds.size} seleccionados
          </span>
          <button
            onClick={() => setSelectedIds(new Set(visible.map((g) => g.id)))}
            className="rounded-lg px-3 py-1.5 text-sm text-muted transition hover:bg-surface hover:text-ink"
          >
            Todos
          </button>
          <div className="mx-1 h-6 w-px bg-line" />
          <button
            onClick={() => bulkFavorite(true)}
            className="flex items-center gap-1.5 rounded-lg px-3 py-1.5 text-sm text-muted transition hover:bg-surface hover:text-ink"
          >
            <StarIcon className="h-4 w-4" /> Favorito
          </button>
          <button
            onClick={() => setShowBulkCats(true)}
            className="flex items-center gap-1.5 rounded-lg px-3 py-1.5 text-sm text-muted transition hover:bg-surface hover:text-ink"
          >
            <TagIcon className="h-4 w-4" /> Categorías
          </button>
          <button
            onClick={bulkHide}
            className="flex items-center gap-1.5 rounded-lg px-3 py-1.5 text-sm text-muted transition hover:bg-surface hover:text-destructive"
          >
            <EyeOffIcon className="h-4 w-4" /> Ocultar
          </button>
          <div className="mx-1 h-6 w-px bg-line" />
          <button
            onClick={exitSelection}
            className="rounded-lg px-3 py-1.5 text-sm text-muted transition hover:bg-surface hover:text-ink"
          >
            Cancelar
          </button>
        </div>
      )}

      {showBulkCats && (
        <BulkCategoryDialog
          count={selectedIds.size}
          allCategories={categoryNames}
          onClose={() => setShowBulkCats(false)}
          onApply={async (cats) => {
            await bulkAddCategories(cats);
            exitSelection();
          }}
        />
      )}

      {spotlight && (
        <Spotlight
          games={games}
          onLaunch={handleLaunch}
          onClose={() => setSpotlight(false)}
        />
      )}

      {menu && (
        <ContextMenu x={menu.x} y={menu.y} items={menu.items} onClose={() => setMenu(null)} />
      )}

      {confirm && (
        <ConfirmDialog
          title={confirm.title}
          message={confirm.message}
          confirmLabel={confirm.confirmLabel}
          onConfirm={confirm.onConfirm}
          onClose={() => setConfirm(null)}
        />
      )}

      {toast && (
        <div className="fixed bottom-14 left-1/2 -translate-x-1/2 rounded-lg border border-line bg-elevated px-4 py-2.5 text-sm text-ink shadow-card">
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
