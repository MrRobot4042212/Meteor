'use client';

import { useState } from 'react';
import type { GameSource, Category } from '@/lib/types';
import { SOURCE_META, SOURCE_ORDER } from '@/lib/sources';
import { CATEGORY_ICONS } from '@/lib/categoryIcons';
import { GridIcon, GearIcon, StarIcon, TagIcon, PlusIcon } from './icons';

export type Filter = 'all' | 'favorites' | GameSource | `cat:${string}`;

type NavItem = { id: Filter; label: string; Icon: typeof GridIcon };

export function Sidebar({
  filter,
  onFilter,
  counts,
  categories,
  onAddCategory,
  onDropGame,
  isDragging = false,
  onOpenSettings,
}: {
  filter: Filter;
  onFilter: (f: Filter) => void;
  counts: Record<string, number>;
  /** User categories (with icons), alphabetically sorted. */
  categories: Category[];
  onAddCategory: () => void;
  /** A game card was dropped onto a droppable target (Favoritos / a category). */
  onDropGame: (target: Filter, gameId: string) => void;
  /** True while a game card is being dragged, to invite valid drop targets. */
  isDragging?: boolean;
  onOpenSettings: () => void;
}) {
  // Which droppable target the dragged card is currently hovering, for highlight.
  const [dragOver, setDragOver] = useState<Filter | null>(null);

  // Drop handlers for a target id; only attached to Favoritos and categories.
  const dropProps = (id: Filter) => ({
    onDragOver: (e: React.DragEvent) => {
      e.preventDefault();
      e.dataTransfer.dropEffect = 'copy';
      if (dragOver !== id) setDragOver(id);
    },
    onDragLeave: () => setDragOver((d) => (d === id ? null : d)),
    onDrop: (e: React.DragEvent) => {
      e.preventDefault();
      const gameId = e.dataTransfer.getData('text/plain');
      setDragOver(null);
      if (gameId) onDropGame(id, gameId);
    },
  });

  // Group 1 — the library itself: everything + favorites. Favoritos is always
  // shown so it can act as a permanent drop target (drag a game in to favorite).
  const libraryItems: NavItem[] = [
    { id: 'all', label: 'Todo', Icon: GridIcon },
    { id: 'favorites', label: 'Favoritos', Icon: StarIcon },
  ];

  // Group 2 — the app's built-in store filters (only non-empty ones).
  const storeItems: NavItem[] = SOURCE_ORDER.filter((s) => counts[s] > 0).map((s) => ({
    id: s as Filter,
    label: SOURCE_META[s].label,
    Icon: SOURCE_META[s].Icon,
  }));

  // Base row styling, plus drop-target states: a quiet dashed "ready" hint while
  // any drag is in progress, and a strong accent highlight when hovered over.
  const itemClass = (id: Filter, droppable: boolean) => {
    const active = filter === id;
    const over = dragOver === id;
    const ready = droppable && isDragging && !over;
    return [
      'group flex items-center gap-3 px-3 py-2.5 text-sm transition-all duration-150',
      over
        ? 'bg-accent/15 text-ink ring-2 ring-inset ring-accent scale-[1.02]'
        : ready
          ? 'text-ink bg-accent/5 outline-dashed outline-1 outline-accent/40 -outline-offset-2'
          : active
            ? 'bg-elevated text-ink'
            : 'text-muted hover:bg-elevated/50 hover:text-ink',
    ].join(' ');
  };

  const renderItem = ({ id, label, Icon }: NavItem, droppable = false) => {
    const active = filter === id;
    const over = dragOver === id;
    return (
      <button
        key={id}
        onClick={() => onFilter(id)}
        {...(droppable ? dropProps(id) : {})}
        className={itemClass(id, droppable)}
      >
        <Icon
          className={`h-[18px] w-[18px] transition-transform ${
            active || over ? 'text-accent' : ''
          } ${over ? 'scale-125' : ''}`}
        />
        <span className="flex-1 truncate text-left">{label}</span>
        <span className="text-xs tabular-nums text-muted">{counts[id] ?? 0}</span>
      </button>
    );
  };

  const Heading = ({ children }: { children: React.ReactNode }) => (
    <p className="px-3 pb-1 pt-5 text-[11px] font-semibold uppercase tracking-wide text-muted/70">
      {children}
    </p>
  );

  return (
    <aside className="flex w-[208px] shrink-0 flex-col overflow-y-auto border-r border-line bg-sidebar px-3 py-5">
      <div className="mb-3 flex items-center gap-2.5 px-2">
          <svg xmlns="http://www.w3.org/2000/svg" width="24px" height="24px" viewBox="0 0 256 251">
  <path d="M0 0h256v251H0z" fill="none" />
  <path fill="#df4f4f" d="M.439.438L219.3 232.266s7.457 5.259 13.158-.877c5.702-6.135 1.316-12.27 1.316-12.27zM69.738 22.35l166.668 179.677s7.456 5.26 13.158-.876c5.702-6.135 1.316-12.27 1.316-12.27zM21.053 69.242L187.72 248.919s7.456 5.259 13.158-.877c5.702-6.135 1.316-12.27 1.316-12.27zM128.32 41.194l116.442 125.53s5.21 3.674 9.193-.612c3.983-4.287.9１９-8．５７３．９１９-8．５７３zm-9１．２２８ 8２．３８９l１１６．４４１ １２５．５３s５．２１ ３．６７４ ９．１９３-.６１３c３．９８３-４．２８６．９１９-８．５７２．９１９-８．５７２zM１８８．１６ ６８．３６５l５２．７７５ ５７．０６７s２．５７７ １．７２２ ４．５４７-.２８７s．４５５-４．０１７．４５５-４．０１７zM６６．２２９ １８１．４３l５２．７７５ ５７．０６７s２．５７７ １．７２２ ４．５４７-.２８６s．４５５-４．０１７．４５５-４．０１７z" />
</svg>
        <span className="font-display text-lg font-semibold tracking-tight text-ink">
          Meteor
        </span>
      </div>

      {/* Group: Biblioteca (Favoritos is a drop target) */}
      <Heading>Biblioteca</Heading>
      <nav className="flex flex-col gap-1">
        {libraryItems.map((it) => renderItem(it, it.id === 'favorites'))}
      </nav>

      {/* Group: Proveedores (built-in / default) */}
      {storeItems.length > 0 && (
        <>
          <Heading>Proveedores</Heading>
          <nav className="flex flex-col gap-1">{storeItems.map((it) => renderItem(it))}</nav>
        </>
      )}

      {/* Group: Categorías (user-created, drop targets), separated from defaults */}
      <div className="flex items-center justify-between pr-1">
        <Heading>Categorías</Heading>
        <button
          onClick={onAddCategory}
          title="Nueva categoría"
          className="mt-3 grid h-5 w-5 place-items-center text-muted transition hover:bg-elevated hover:text-accent"
        >
          <PlusIcon className="h-3.5 w-3.5" />
        </button>
      </div>
      {categories.length > 0 ? (
        <nav className="flex flex-col gap-1">
          {categories.map(({ name, icon }) => {
            const id = `cat:${name}` as Filter;
            const over = dragOver === id;
            const Icon = (icon && CATEGORY_ICONS[icon]) || TagIcon;
            return (
              <button
                key={id}
                onClick={() => onFilter(id)}
                {...dropProps(id)}
                className={itemClass(id, true)}
              >
                <Icon
                  className={`h-[18px] w-[18px] transition-transform ${
                    filter === id || over ? 'text-accent' : ''
                  } ${over ? 'scale-125' : ''}`}
                />
                <span className="flex-1 truncate text-left">{name}</span>
                <span className="text-xs tabular-nums text-muted">{counts[id] ?? 0}</span>
              </button>
            );
          })}
        </nav>
      ) : (
        <p className="px-3 py-1 text-xs leading-relaxed text-muted/60">
          Crea tus propias categorías con el botón +.
        </p>
      )}

      <div className="mt-auto flex flex-col gap-1 pt-5">
        <button
          onClick={onAddCategory}
          className="flex items-center gap-3 px-3 py-2.5 text-sm text-muted transition-colors hover:bg-elevated/50 hover:text-ink"
        >
          <PlusIcon className="h-[18px] w-[18px]" />
          <span>Nueva categoría</span>
        </button>
        <button
          onClick={onOpenSettings}
          className="flex items-center gap-3 px-3 py-2.5 text-sm text-muted transition-colors hover:bg-elevated/50 hover:text-ink"
        >
          <GearIcon className="h-[18px] w-[18px]" />
          <span>Ajustes</span>
        </button>
      </div>
    </aside>
  );
}
