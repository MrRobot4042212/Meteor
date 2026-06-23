'use client';

import { useEffect, useState } from 'react';
import { listen } from '@tauri-apps/api/event';
import type { Game, GameDetails, PlayStat, Session } from '@/lib/types';
import { gameDetails, getPlaytime, dirSize, openPath, userScreenshots } from '@/lib/tauri';
import { coverSrc } from '@/lib/cover';
import { SOURCE_META } from '@/lib/sources';
import {
  translateGenre,
  translateMode,
  translateTheme,
  translatePerspective,
} from '@/lib/i18n';
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
  AppIcon,
  GridIcon,
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

/** IGDB time-to-beat seconds → "12.5 h" / "45 min" (null if absent). */
function formatHours(seconds?: number | null): string | null {
  if (!seconds || seconds <= 0) return null;
  const h = seconds / 3600;
  if (h < 1) return `${Math.round(seconds / 60)} min`;
  return h < 10 ? `${h.toFixed(1)} h` : `${Math.round(h)} h`;
}

export function DetailView({
  game,
  onBack,
  onLaunch,
  onToggleFavorite,
  onToggleType,
  onEditCover,
  onEditCategories,
  onRemove,
  onHide,
}: {
  game: Game;
  onBack: () => void;
  onLaunch: (g: Game) => void;
  onToggleFavorite: (g: Game) => void;
  onToggleType?: (g: Game) => void;
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
  // Trailer currently playing (YouTube id), null = show the thumbnail.
  const [playId, setPlayId] = useState<string | null>(null);
  const [activeTab, setActiveTab] = useState<'galeria' | 'videos' | 'grial' | 'local'>('galeria');

  // Applications aren't games: skip all the IGDB metadata/media and screenshots.
  const isApp = game.source === 'app';

  useEffect(() => {
    let alive = true;
    setDetails(undefined);
    setShots([]);
    setPlayId(null);
    getPlaytime(game.id)
      .then((p) => alive && setPlay(p))
      .catch(() => {});
    if (isApp) {
      setDetails(null); // no game metadata for apps
    } else {
      gameDetails(game.name)
        .then((d) => alive && setDetails(d))
        .catch(() => alive && setDetails(null));
      // The user's own screenshots (Steam / Game Bar), not promotional art.
      userScreenshots(game)
        .then((s) => alive && setShots(s))
        .catch(() => {});
    }
    return () => {
      alive = false;
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [game.id, game.name, isApp]);

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
  // Promotional artwork (IGDB) makes a richer backdrop than a user screenshot.
  const art0 = details?.artworks?.[0] ?? details?.screenshots?.[0];
  const backdrop = art0 ?? (shots[0] ? coverSrc(shots[0]) : undefined) ?? cover;
  const meta = SOURCE_META[game.source];
  const SourceIcon = meta.Icon;

  const videos = details?.videos ?? [];
  const similar = (details?.similar ?? []).filter((s) => s.cover_url);
  // IGDB promotional media (artworks first, then screenshots) for the gallery.
  const gallery = [...(details?.artworks ?? []), ...(details?.screenshots ?? [])];
  const ttb = details?.time_to_beat;

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

            {/* Genres + themes */}
            {((details?.genres?.length ?? 0) > 0 || (details?.themes?.length ?? 0) > 0) && (
              <div className="mt-3 flex flex-wrap gap-2">
                {details?.genres?.map((g) => (
                  <span key={g} className="border border-line px-2 py-0.5 text-xs text-muted">
                    {translateGenre(g)}
                  </span>
                ))}
                {details?.themes?.map((t) => (
                  <span
                    key={t}
                    className="border border-accent/30 bg-accent/5 px-2 py-0.5 text-xs text-muted"
                  >
                    {translateTheme(t)}
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
              {onToggleType && (
                <IconBtn
                  title={game.source === 'app' ? 'Marcar como juego' : 'Marcar como aplicación'}
                  onClick={() => onToggleType(game)}
                >
                  {game.source === 'app' ? (
                    <GridIcon className="h-[18px] w-[18px]" />
                  ) : (
                    <AppIcon className="h-[18px] w-[18px]" />
                  )}
                </IconBtn>
              )}
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

        {!isApp && (
          <>
            {/* Summary */}
            <Section title="Resumen">
          {details === undefined ? (
            <p className="text-sm text-muted">Cargando información…</p>
          ) : details?.summary ? (
            <p className="max-w-3xl text-sm leading-relaxed text-ink/90">{details.summary}</p>
          ) : (
            <p className="text-sm text-muted">No hay descripción disponible para este título.</p>
          )}
          {(details?.developer ||
            details?.publisher ||
            details?.franchise ||
            (details?.perspectives?.length ?? 0) > 0) && (
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
              {details?.franchise && (
                <span className="text-muted">
                  Saga: <span className="text-ink">{details.franchise}</span>
                </span>
              )}
              {(details?.perspectives?.length ?? 0) > 0 && (
                <span className="text-muted">
                  Perspectiva:{' '}
                  <span className="text-ink">
                    {details!.perspectives!.map(translatePerspective).join(', ')}
                  </span>
                </span>
              )}
            </div>
          )}
        </Section>

        {/* Tabs navigation */}
        <div className="mt-10 mb-6 flex overflow-x-auto border-b border-line">
          {[
            { id: 'galeria', label: 'Galería' },
            { id: 'videos', label: 'Vídeos' },
            { id: 'grial', label: 'Santo Grial' },
            { id: 'local', label: 'Local' },
          ].map((tab) => (
            <button
              key={tab.id}
              onClick={() => setActiveTab(tab.id as any)}
              className={`whitespace-nowrap px-6 py-3 font-display text-sm font-semibold uppercase tracking-wide transition ${
                activeTab === tab.id
                  ? 'border-b-2 border-accent text-accent'
                  : 'text-muted hover:text-ink'
              }`}
            >
              {tab.label}
            </button>
          ))}
        </div>

        {/* GALERÍA */}
        {activeTab === 'galeria' && (
          <div className="space-y-10">
            {gallery.length > 0 && (
              <Section title="Media Promocional">
                <div className="grid grid-cols-2 gap-3 lg:grid-cols-3">
                  {gallery.map((src) => (
                    <button
                      key={src}
                      onClick={() => setShot(src)}
                      className="group overflow-hidden border border-line bg-elevated"
                    >
                      {/* eslint-disable-next-line @next/next/no-img-element */}
                      <img
                        src={src}
                        alt=""
                        loading="lazy"
                        className="aspect-video w-full object-cover transition group-hover:scale-105"
                      />
                    </button>
                  ))}
                </div>
              </Section>
            )}

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
          </div>
        )}

        {/* VÍDEOS */}
        {activeTab === 'videos' && (
          <div className="space-y-10">
            {videos.length > 0 ? (
              <Section title="Vídeos y Tráilers">
                <div className="max-w-3xl">
                  <div className="relative aspect-video w-full overflow-hidden border border-line bg-void">
                    {playId ? (
                      <iframe
                        src={`https://www.youtube-nocookie.com/embed/${playId}?autoplay=1&rel=0`}
                        title="Tráiler"
                        allow="accelerometer; autoplay; clipboard-write; encrypted-media; gyroscope; picture-in-picture"
                        allowFullScreen
                        className="h-full w-full"
                      />
                    ) : (
                      <button
                        onClick={() => setPlayId(videos[0])}
                        className="group h-full w-full"
                        title="Reproducir tráiler"
                      >
                        {/* eslint-disable-next-line @next/next/no-img-element */}
                        <img
                          src={`https://img.youtube.com/vi/${videos[0]}/hqdefault.jpg`}
                          alt="Tráiler"
                          className="h-full w-full object-cover opacity-80 transition group-hover:opacity-100"
                        />
                        <span className="absolute inset-0 grid place-items-center">
                          <span className="grid h-16 w-16 place-items-center bg-accent/90 text-white shadow-glow transition group-hover:scale-110">
                            <PlayIcon className="h-7 w-7" />
                          </span>
                        </span>
                      </button>
                    )}
                  </div>
                  {videos.length > 1 && (
                    <div className="mt-3 flex gap-2 overflow-x-auto pb-2">
                      {videos.map((v) => (
                        <button
                          key={v}
                          onClick={() => setPlayId(v)}
                          className={`relative aspect-video w-28 shrink-0 overflow-hidden border transition ${
                            playId === v ? 'border-accent' : 'border-line hover:border-accent/50'
                          }`}
                        >
                          {/* eslint-disable-next-line @next/next/no-img-element */}
                          <img
                            src={`https://img.youtube.com/vi/${v}/mqdefault.jpg`}
                            alt=""
                            loading="lazy"
                            className="h-full w-full object-cover"
                          />
                        </button>
                      ))}
                    </div>
                  )}
                </div>
              </Section>
            ) : (
              <p className="text-sm text-muted mt-8">No hay vídeos disponibles para este juego.</p>
            )}
          </div>
        )}

        {/* SANTO GRIAL */}
        {activeTab === 'grial' && (
          <div className="space-y-10">
            {/* Time to beat */}
            {ttb && (formatHours(ttb.hastily) || formatHours(ttb.normally) || formatHours(ttb.completely)) && (
              <Section title="Duración (HowLongToBeat)">
                <div className="grid max-w-2xl grid-cols-3 gap-3">
                  <Metric label="Historia" value={formatHours(ttb.hastily) ?? '—'} />
                  <Metric label="Normal" value={formatHours(ttb.normally) ?? '—'} />
                  <Metric label="Al 100%" value={formatHours(ttb.completely) ?? '—'} />
                </div>
              </Section>
            )}

            {/* Websites */}
            <Section title="Enlaces Externos Útiles">
              {details?.websites && details.websites.length > 0 ? (
                <div className="grid grid-cols-1 sm:grid-cols-2 md:grid-cols-3 gap-3">
                  {details.websites.map((w) => {
                    const info = mapWebsiteCategory(w.category);
                    return (
                      <a
                        key={w.url}
                        href={w.url}
                        target="_blank"
                        rel="noreferrer"
                        className="flex items-center gap-3 border border-line bg-elevated px-4 py-3 text-sm text-ink transition hover:border-accent/50 hover:bg-elevated/80"
                      >
                        <div className="text-accent">{info.icon}</div>
                        <div className="min-w-0 flex-1 truncate font-medium">{info.name}</div>
                      </a>
                    );
                  })}
                </div>
              ) : (
                <p className="text-sm text-muted">No se encontraron enlaces en IGDB para este juego.</p>
              )}
            </Section>

            {/* Similar games */}
            {similar.length > 0 && (
              <Section title="Juegos recomendados (Similares)">
                <div className="flex gap-4 overflow-x-auto pb-2">
                  {similar.map((s) => (
                    <div key={s.name} className="w-[120px] shrink-0" title={s.name}>
                      <div className="aspect-[2/3] overflow-hidden border border-line bg-elevated">
                        {/* eslint-disable-next-line @next/next/no-img-element */}
                        <img
                          src={s.cover_url as string}
                          alt={s.name}
                          loading="lazy"
                          className="h-full w-full object-cover"
                        />
                      </div>
                      <p className="mt-1.5 line-clamp-2 text-xs text-muted">{s.name}</p>
                    </div>
                  ))}
                </div>
              </Section>
            )}
          </div>
        )}

        {/* LOCAL */}
        {activeTab === 'local' && (
          <div className="space-y-10">
            <Section title="Información en Disco">
              <dl className="max-w-3xl divide-y divide-line text-sm border border-line p-4 bg-surface/30">
                <Row label="Directorio de Instalación" value={game.install_dir ?? 'Desconocido'} />
                {game.executable && <Row label="Ejecutable" value={game.executable} />}
                <Row
                  label="Tamaño en disco"
                  value={size === undefined ? 'Calculando…' : size === null ? '—' : formatSize(size)}
                />
                <Row label="Plataforma de origen" value={SOURCE_META[game.source]?.label ?? game.source} />
              </dl>
              {game.install_dir && (
                <button
                  onClick={() => openPath(game.install_dir as string)}
                  className="mt-4 flex items-center gap-2 border border-line px-4 py-2 text-sm text-ink transition hover:border-accent/50 hover:bg-elevated"
                >
                  <FolderIcon className="h-4 w-4 text-accent" />
                  Abrir carpeta contenedora
                </button>
              )}
            </Section>
          </div>
        )}
          </>
        )}
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
      <dt className="w-48 shrink-0 text-muted">{label}</dt>
      <dd className="min-w-0 flex-1 break-all text-ink/90">{value}</dd>
    </div>
  );
}

function mapWebsiteCategory(category: number): { name: string; icon: React.ReactNode } {
  const map: Record<number, string> = {
    1: 'Sitio Oficial',
    2: 'Wikia / Fandom',
    3: 'Wikipedia',
    4: 'Facebook',
    5: 'Twitter',
    6: 'Twitch',
    8: 'Instagram',
    9: 'YouTube',
    10: 'iPhone',
    11: 'iPad',
    12: 'Android',
    13: 'Steam',
    14: 'Reddit',
    15: 'Itch.io',
    16: 'Epic Games',
    17: 'GOG',
    18: 'Discord',
  };
  // Fallback a un icono genérico de "Link" si no tenemos un SVG específico de esa red.
  return {
    name: map[category] || 'Sitio Web',
    icon: <TagIcon className="h-4 w-4" />, // Reutilizamos TagIcon u otro como fallback para webs.
  };
}
