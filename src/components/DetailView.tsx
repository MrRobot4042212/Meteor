'use client';

import { useEffect, useState } from 'react';
import { listen } from '@tauri-apps/api/event';
import type { Game, GameDetails, PlayStat, Session } from '@/lib/types';
import { gameDetails, getPlaytime, dirSize, openPath, userScreenshots } from '@/lib/tauri';
import { coverSrc } from '@/lib/cover';
import { SOURCE_META } from '@/lib/sources';
import { translateGenre, translateMode } from '@/lib/i18n';
import {
  ArrowLeftIcon,
  PlayIcon,
  StarIcon,
  TagIcon,
  ImageIcon,
  FolderIcon,
  EyeOffIcon,
  TrashIcon,
  ClockIcon,
  CloseIcon,
} from './icons';

function formatPlaytime(seconds: number): string {
  if (seconds < 60) return 'Sin registro';
  const h = Math.floor(seconds / 3600);
  const m = Math.floor((seconds % 3600) / 60);
  if (h === 0) return `${m} min`;
  return m === 0 ? `${h} h` : `${h} h ${m} min`;
}

/** Seconds played within the last `days` days (from session history). */
function recentSeconds(history: Session[], days: number): number {
  const cutoff = Date.now() / 1000 - days * 86400;
  return history.reduce((acc, s) => acc + (s.end >= cutoff ? s.end - s.start : 0), 0);
}

function formatSize(bytes: number): string {
  if (bytes >= 1024 ** 3) return `${(bytes / 1024 ** 3).toFixed(1)} GB`;
  if (bytes >= 1024 ** 2) return `${Math.round(bytes / 1024 ** 2)} MB`;
  return `${Math.round(bytes / 1024)} KB`;
}

function formatLastPlayed(ts: number): string {
  const diff = Date.now() / 1000 - ts;
  const day = 86400;
  if (diff < day) return 'Hoy';
  if (diff < 2 * day) return 'Ayer';
  if (diff < 30 * day) return `Hace ${Math.floor(diff / day)} días`;
  return new Date(ts * 1000).toLocaleDateString();
}

export function DetailView({
  game,
  onBack,
  onLaunch,
  onToggleFavorite,
  onEditCover,
  onEditCategories,
  onRemove,
  onHide,
}: {
  game: Game;
  onBack: () => void;
  onLaunch: (g: Game) => void;
  onToggleFavorite: (g: Game) => void;
  onEditCover: (g: Game) => void;
  onEditCategories: (g: Game) => void;
  onRemove?: (g: Game) => void;
  onHide?: (g: Game) => void;
}) {
  const [details, setDetails] = useState<GameDetails | null | undefined>(undefined);
  const [play, setPlay] = useState<PlayStat>({ seconds: 0, history: [] });
  const [size, setSize] = useState<number | null | undefined>(undefined);
  const [shots, setShots] = useState<string[]>([]);
  const [shot, setShot] = useState<string | null>(null);

  useEffect(() => {
    let alive = true;
    setDetails(undefined);
    setShots([]);
    gameDetails(game.name)
      .then((d) => alive && setDetails(d))
      .catch(() => alive && setDetails(null));
    getPlaytime(game.id)
      .then((p) => alive && setPlay(p))
      .catch(() => {});
    // The user's own screenshots (Steam / Game Bar), not promotional art.
    userScreenshots(game)
      .then((s) => alive && setShots(s))
      .catch(() => {});
    return () => {
      alive = false;
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [game.id, game.name]);

  useEffect(() => {
    let alive = true;
    setSize(undefined);
    if (game.install_dir) {
      dirSize(game.install_dir)
        .then((s) => alive && setSize(s))
        .catch(() => alive && setSize(null));
    } else {
      setSize(null);
    }
    return () => {
      alive = false;
    };
  }, [game.id, game.install_dir]);

  // Refresh playtime live when a session this game ends.
  useEffect(() => {
    const un = listen<string>('playtime-updated', (e) => {
      if (e.payload === game.id) getPlaytime(game.id).then(setPlay).catch(() => {});
    });
    return () => {
      un.then((f) => f());
    };
  }, [game.id]);

  const logo = game.cover_url ? null : game.icon ? coverSrc(game.icon) ?? null : null;
  const cover = coverSrc(game.cover_url);
  const backdrop = (shots[0] ? coverSrc(shots[0]) : undefined) ?? cover;
  const meta = SOURCE_META[game.source];
  const SourceIcon = meta.Icon;

  return (
    <div className="relative h-full overflow-y-auto bg-background">
      {/* Backdrop */}
      <div className="pointer-events-none absolute inset-x-0 top-0 h-[420px] overflow-hidden">
        {backdrop && (
          // eslint-disable-next-line @next/next/no-img-element
          <img
            src={backdrop}
            alt=""
            className="h-full w-full scale-110 object-cover opacity-25 blur-2xl"
          />
        )}
        <div className="absolute inset-0 bg-gradient-to-b from-background/40 via-background/70 to-background" />
      </div>

      {/* Back bar */}
      <div className="sticky top-0 z-20 flex items-center gap-3 bg-background/80 px-6 py-3 backdrop-blur">
        <button
          onClick={onBack}
          className="flex items-center gap-2 px-2 py-1.5 text-sm text-muted transition hover:text-ink"
        >
          <ArrowLeftIcon className="h-[18px] w-[18px]" />
          Biblioteca
        </button>
      </div>

      <div className="relative mx-auto max-w-5xl px-6 pb-16 pt-16">
        {/* Hero */}
        <div className="flex flex-col gap-6 sm:flex-row">
          <div className="aspect-[2/3] w-44 shrink-0 self-center overflow-hidden border border-line bg-elevated shadow-card sm:self-start">
            {logo ? (
              <div className="grid h-full w-full place-items-center bg-gradient-to-b from-elevated to-surface p-7">
                {/* eslint-disable-next-line @next/next/no-img-element */}
                <img src={logo} alt={game.name} className="max-h-[60%] max-w-[80%] object-contain drop-shadow-lg" />
              </div>
            ) : cover ? (
              // eslint-disable-next-line @next/next/no-img-element
              <img src={cover} alt={game.name} className="h-full w-full object-cover" />
            ) : (
              <div className="grid h-full w-full place-items-center text-5xl font-bold text-accent/70">
                {game.name.charAt(0).toUpperCase()}
              </div>
            )}
          </div>

          <div className="min-w-0 flex-1">
            <div className="mb-2 flex items-center gap-2 text-xs uppercase tracking-wide text-muted">
              <SourceIcon className="h-4 w-4" />
              {meta.label}
            </div>
            <h1 className="font-display text-3xl font-bold leading-tight text-ink">
              {game.name}
            </h1>

            {/* Meta row */}
            <div className="mt-2 flex flex-wrap items-center gap-x-3 gap-y-1 text-sm text-muted">
              {details?.release_year && <span>{details.release_year}</span>}
              {details?.rating != null && (
                <span className="text-ink">
                  ★ {details.rating}/100
                  {details.rating_count ? (
                    <span className="text-muted"> ({details.rating_count})</span>
                  ) : null}
                </span>
              )}
              {details?.developer && <span>· {details.developer}</span>}
            </div>

            {/* Genres */}
            {details?.genres && details.genres.length > 0 && (
              <div className="mt-3 flex flex-wrap gap-2">
                {details.genres.map((g) => (
                  <span key={g} className="border border-line px-2 py-0.5 text-xs text-muted">
                    {translateGenre(g)}
                  </span>
                ))}
              </div>
            )}

            {/* Actions */}
            <div className="mt-5 flex flex-wrap items-center gap-2">
              <button
                onClick={() => onLaunch(game)}
                className="flex items-center gap-2 bg-accent px-6 py-2.5 text-sm font-semibold text-white transition hover:bg-accent-soft"
              >
                <PlayIcon className="h-4 w-4" />
                Jugar
              </button>
              <IconBtn
                title={game.favorite ? 'Quitar de favoritos' : 'Marcar favorito'}
                active={game.favorite}
                onClick={() => onToggleFavorite(game)}
              >
                <StarIcon className="h-[18px] w-[18px]" fill={game.favorite ? 'currentColor' : 'none'} />
              </IconBtn>
              <IconBtn title="Categorías" onClick={() => onEditCategories(game)}>
                <TagIcon className="h-[18px] w-[18px]" />
              </IconBtn>
              <IconBtn title="Cambiar carátula" onClick={() => onEditCover(game)}>
                <ImageIcon className="h-[18px] w-[18px]" />
              </IconBtn>
              {game.install_dir && (
                <IconBtn
                  title="Abrir carpeta"
                  onClick={() => openPath(game.install_dir as string)}
                >
                  <FolderIcon className="h-[18px] w-[18px]" />
                </IconBtn>
              )}
              {onHide && (
                <IconBtn title="Ocultar" onClick={() => onHide(game)}>
                  <EyeOffIcon className="h-[18px] w-[18px]" />
                </IconBtn>
              )}
              {onRemove && (
                <IconBtn title="Quitar" onClick={() => onRemove(game)}>
                  <TrashIcon className="h-[18px] w-[18px]" />
                </IconBtn>
              )}
            </div>

            {/* Categories assigned */}
            {game.categories && game.categories.length > 0 && (
              <div className="mt-4 flex flex-wrap gap-2">
                {game.categories.map((c) => (
                  <span key={c} className="bg-elevated px-2 py-0.5 text-xs text-ink">
                    {c}
                  </span>
                ))}
              </div>
            )}
          </div>
        </div>

        {/* Activity metrics */}
        <div className="mt-8 grid grid-cols-2 gap-3 sm:grid-cols-3 lg:grid-cols-6">
          <Metric label="Tiempo jugado" value={formatPlaytime(play.seconds)} icon={<ClockIcon className="h-4 w-4" />} />
          <Metric
            label="Últimos 14 días"
            value={formatPlaytime(recentSeconds(play.history, 14))}
          />
          <Metric label="Sesiones" value={play.history.length > 0 ? String(play.history.length) : '—'} />
          <Metric
            label="Última vez"
            value={play.last_played ? formatLastPlayed(play.last_played) : 'Nunca'}
          />
          <Metric
            label="Tamaño"
            value={size === undefined ? '…' : size === null ? '—' : formatSize(size)}
          />
          <Metric
            label="Modos"
            value={details?.modes?.length ? details.modes.map(translateMode).join(', ') : '—'}
          />
        </div>

        {/* Summary */}
        <Section title="Resumen">
          {details === undefined ? (
            <p className="text-sm text-muted">Cargando información…</p>
          ) : details?.summary ? (
            <p className="max-w-3xl text-sm leading-relaxed text-ink/90">{details.summary}</p>
          ) : (
            <p className="text-sm text-muted">No hay descripción disponible para este título.</p>
          )}
          {(details?.developer || details?.publisher) && (
            <div className="mt-4 flex flex-wrap gap-x-8 gap-y-2 text-sm">
              {details?.developer && (
                <span className="text-muted">
                  Desarrollador: <span className="text-ink">{details.developer}</span>
                </span>
              )}
              {details?.publisher && (
                <span className="text-muted">
                  Editor: <span className="text-ink">{details.publisher}</span>
                </span>
              )}
            </div>
          )}
        </Section>

        {/* User screenshots (Steam / Game Bar), not promotional art */}
        <Section title="Mis capturas">
          {shots.length > 0 ? (
            <div className="grid grid-cols-2 gap-3 lg:grid-cols-3">
              {shots.map((s) => (
                <button
                  key={s}
                  onClick={() => setShot(s)}
                  className="group overflow-hidden border border-line bg-elevated"
                >
                  {/* eslint-disable-next-line @next/next/no-img-element */}
                  <img
                    src={coverSrc(s)}
                    alt=""
                    loading="lazy"
                    className="aspect-video w-full object-cover transition group-hover:scale-105"
                  />
                </button>
              ))}
            </div>
          ) : (
            <p className="text-sm text-muted">
              No hay capturas tuyas de este juego. Se buscan en las capturas de Steam
              (F12) y en la galería de Windows Game Bar (Win+Alt+Impr Pant).
            </p>
          )}
        </Section>

        {/* Files */}
        <Section title="Archivos">
          <dl className="max-w-3xl divide-y divide-line text-sm">
            <Row label="Ruta" value={game.install_dir ?? '—'} />
            {game.executable && <Row label="Ejecutable" value={game.executable} />}
            <Row
              label="Tamaño en disco"
              value={size === undefined ? 'Calculando…' : size === null ? '—' : formatSize(size)}
            />
          </dl>
          {game.install_dir && (
            <button
              onClick={() => openPath(game.install_dir as string)}
              className="mt-4 flex items-center gap-2 border border-line px-3 py-2 text-sm text-ink transition hover:border-accent/50"
            >
              <FolderIcon className="h-4 w-4" />
              Abrir carpeta
            </button>
          )}
        </Section>
      </div>

      {/* Screenshot lightbox */}
      {shot && (
        <div
          className="fixed inset-0 z-50 grid place-items-center bg-void/90 p-8"
          onClick={() => setShot(null)}
        >
          <button className="absolute right-6 top-6 text-muted hover:text-ink">
            <CloseIcon className="h-6 w-6" />
          </button>
          {/* eslint-disable-next-line @next/next/no-img-element */}
          <img src={coverSrc(shot)} alt="" className="max-h-full max-w-full border border-line" />
        </div>
      )}
    </div>
  );
}

function IconBtn({
  children,
  title,
  active,
  onClick,
}: {
  children: React.ReactNode;
  title: string;
  active?: boolean;
  onClick: () => void;
}) {
  return (
    <button
      title={title}
      onClick={onClick}
      className={`grid h-10 w-10 place-items-center border transition ${
        active
          ? 'border-accent text-accent'
          : 'border-line text-muted hover:border-accent/50 hover:text-ink'
      }`}
    >
      {children}
    </button>
  );
}

function Metric({
  label,
  value,
  icon,
}: {
  label: string;
  value: string;
  icon?: React.ReactNode;
}) {
  return (
    <div className="border border-line bg-surface/50 p-4">
      <div className="flex items-center gap-1.5 text-[11px] uppercase tracking-wide text-muted">
        {icon}
        {label}
      </div>
      <div className="mt-1 truncate font-display text-lg font-semibold text-ink" title={value}>
        {value}
      </div>
    </div>
  );
}

function Section({ title, children }: { title: string; children: React.ReactNode }) {
  return (
    <section className="mt-10">
      <h2 className="mb-4 font-display text-sm font-semibold uppercase tracking-wide text-muted">
        {title}
      </h2>
      {children}
    </section>
  );
}

function Row({ label, value }: { label: string; value: string }) {
  return (
    <div className="flex gap-4 py-2">
      <dt className="w-32 shrink-0 text-muted">{label}</dt>
      <dd className="min-w-0 flex-1 break-all text-ink/90">{value}</dd>
    </div>
  );
}
