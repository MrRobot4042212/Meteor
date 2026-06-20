'use client';

import { GridIcon, SteamIcon, AppIcon } from './icons';

export type Filter = 'all' | 'steam' | 'manual';

const ITEMS: { id: Filter; label: string; Icon: typeof GridIcon }[] = [
  { id: 'all', label: 'Todo', Icon: GridIcon },
  { id: 'steam', label: 'Steam', Icon: SteamIcon },
  { id: 'manual', label: 'Mis apps', Icon: AppIcon },
];

export function Sidebar({
  filter,
  onFilter,
  counts,
}: {
  filter: Filter;
  onFilter: (f: Filter) => void;
  counts: Record<Filter, number>;
}) {
  return (
    <aside className="flex w-[208px] shrink-0 flex-col border-r border-line bg-surface/60 px-3 py-5">
      <div className="mb-7 flex items-center gap-2.5 px-2">
        <span className="grid h-8 w-8 place-items-center rounded-lg bg-accent font-display text-sm font-bold text-void">
          N
        </span>
        <span className="font-display text-lg font-semibold tracking-tight text-ink">
          Nexo
        </span>
      </div>

      <nav className="flex flex-col gap-1">
        {ITEMS.map(({ id, label, Icon }) => {
          const active = filter === id;
          return (
            <button
              key={id}
              onClick={() => onFilter(id)}
              className={`group flex items-center gap-3 rounded-lg px-3 py-2.5 text-sm transition-colors ${
                active
                  ? 'bg-elevated text-ink'
                  : 'text-muted hover:bg-elevated/50 hover:text-ink'
              }`}
            >
              <Icon
                className={`h-[18px] w-[18px] ${active ? 'text-accent' : ''}`}
              />
              <span className="flex-1 text-left">{label}</span>
              <span className="text-xs tabular-nums text-muted">{counts[id]}</span>
            </button>
          );
        })}
      </nav>

      <div className="mt-auto px-3 text-[11px] leading-relaxed text-muted/70">
        MVP · Steam + apps manuales
      </div>
    </aside>
  );
}
