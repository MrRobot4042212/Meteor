'use client';

import { useState } from 'react';
import type { Game } from '@/lib/types';
import { PlayIcon, TrashIcon } from './icons';

function fallbackChain(game: Game): string[] {
  const urls: string[] = [];
  if (game.cover_url) urls.push(game.cover_url);
  if (game.source === 'steam' && game.app_id) {
    urls.push(
      `https://cdn.cloudflare.steamstatic.com/steam/apps/${game.app_id}/header.jpg`,
    );
  }
  return urls;
}

export function GameCard({
  game,
  onLaunch,
  onRemove,
}: {
  game: Game;
  onLaunch: (g: Game) => void;
  onRemove?: (g: Game) => void;
}) {
  const chain = fallbackChain(game);
  const [stage, setStage] = useState(0);
  const src = chain[stage];
  const exhausted = stage >= chain.length;

  return (
    <div className="group relative aspect-[2/3] overflow-hidden rounded-xl2 border border-line bg-elevated shadow-card transition-transform duration-200 ease-out hover:-translate-y-1 hover:border-accent/40 hover:shadow-glow">
      {!exhausted && src ? (
        // eslint-disable-next-line @next/next/no-img-element
        <img
          src={src}
          alt={game.name}
          loading="lazy"
          onError={() => setStage((s) => s + 1)}
          className="h-full w-full object-cover"
        />
      ) : (
        <div className="flex h-full w-full flex-col items-center justify-center gap-2 bg-gradient-to-b from-elevated to-surface p-3 text-center">
          <span className="font-display text-3xl font-bold text-accent/80">
            {game.name.charAt(0).toUpperCase()}
          </span>
          <span className="line-clamp-3 text-xs text-muted">{game.name}</span>
        </div>
      )}

      {/* Hover overlay */}
      <div className="pointer-events-none absolute inset-0 flex flex-col justify-end bg-gradient-to-t from-void via-void/40 to-transparent opacity-0 transition-opacity duration-200 group-hover:opacity-100">
        <div className="pointer-events-auto p-3">
          <p className="mb-2 line-clamp-2 text-sm font-medium text-ink">
            {game.name}
          </p>
          <button
            onClick={() => onLaunch(game)}
            className="flex w-full items-center justify-center gap-2 rounded-lg bg-accent py-2 text-sm font-semibold text-void transition-colors hover:bg-accent-soft"
          >
            <PlayIcon className="h-4 w-4" />
            Jugar
          </button>
        </div>
      </div>

      {/* Remove (manual only) */}
      {onRemove && (
        <button
          onClick={() => onRemove(game)}
          title="Quitar de la biblioteca"
          className="pointer-events-auto absolute right-2 top-2 grid h-8 w-8 place-items-center rounded-lg bg-void/70 text-muted opacity-0 backdrop-blur transition group-hover:opacity-100 hover:text-accent"
        >
          <TrashIcon className="h-4 w-4" />
        </button>
      )}

      {/* Source dot */}
      <span
        className={`absolute left-2 top-2 h-2 w-2 rounded-full ${
          game.source === 'steam' ? 'bg-sky-400' : 'bg-accent'
        }`}
        title={game.source === 'steam' ? 'Steam' : 'App manual'}
      />
    </div>
  );
}
