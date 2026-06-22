'use client';

import { useEffect, useState } from 'react';
import {
  clearCoverCache,
  hiddenCount,
  restoreHidden,
  getDiscordClientId,
  setDiscordClientId,
} from '@/lib/tauri';
import { CloseIcon } from './icons';

export function SettingsDialog({
  onClose,
  onChanged,
}: {
  onClose: () => void;
  /** Called after a change (cache wiped / hidden restored) so the library can refresh. */
  onChanged: () => void;
}) {
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [hidden, setHidden] = useState<number | null>(null);
  const [discordId, setDiscordId] = useState('');
  const [discordSaved, setDiscordSaved] = useState(false);

  useEffect(() => {
    hiddenCount()
      .then(setHidden)
      .catch(() => setHidden(null));
    getDiscordClientId()
      .then(setDiscordId)
      .catch(() => {});
  }, []);

  async function saveDiscord() {
    setBusy(true);
    setError(null);
    try {
      await setDiscordClientId(discordId.trim());
      setDiscordSaved(true);
      window.setTimeout(() => setDiscordSaved(false), 2000);
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  }

  async function clear() {
    setBusy(true);
    setError(null);
    try {
      await clearCoverCache();
      onChanged();
      onClose();
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  }

  async function restore() {
    setBusy(true);
    setError(null);
    try {
      await restoreHidden();
      onChanged();
      onClose();
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  }

  return (
    <div
      className="fixed inset-0 z-50 grid place-items-center bg-void/70 p-4 backdrop-blur-sm"
      onClick={onClose}
    >
      <div
        className="w-full max-w-md rounded-xl2 border border-line bg-surface p-6 shadow-card"
        onClick={(e) => e.stopPropagation()}
      >
        <div className="mb-5 flex items-center justify-between">
          <h2 className="font-display text-lg font-semibold text-ink">Ajustes</h2>
          <button
            onClick={onClose}
            className="grid h-8 w-8 place-items-center rounded-lg text-muted hover:bg-elevated hover:text-ink"
          >
            <CloseIcon className="h-5 w-5" />
          </button>
        </div>

        <p className="mb-2 text-xs font-medium text-ink">Carátulas</p>
        <p className="mb-3 text-xs leading-relaxed text-muted">
          Las carátulas se obtienen automáticamente y se guardan en disco para que
          carguen al instante. Si alguna se ve mal o quieres volver a buscarlas
          todas, vacía la caché y se descargarán de nuevo.
        </p>
        <button
          onClick={clear}
          disabled={busy}
          className="mb-5 w-full rounded-lg border border-line bg-elevated px-4 py-2.5 text-sm font-medium text-ink transition hover:border-accent/40 disabled:opacity-50"
        >
          {busy ? 'Trabajando…' : 'Vaciar caché de carátulas'}
        </button>

        <div className="border-t border-line pt-5">
          <p className="mb-2 text-xs font-medium text-ink">Juegos ocultos</p>
          <p className="mb-3 text-xs leading-relaxed text-muted">
            {hidden && hidden > 0
              ? `Tienes ${hidden} ${hidden === 1 ? 'juego oculto' : 'juegos ocultos'}. Restáuralos si ocultaste algo por error.`
              : 'No has ocultado nada. Usa el icono del ojo en una card para ocultar lo que no sea un juego.'}
          </p>
          <button
            onClick={restore}
            disabled={busy || !hidden}
            className="w-full rounded-lg border border-line bg-elevated px-4 py-2.5 text-sm font-medium text-ink transition hover:border-accent/40 disabled:opacity-50"
          >
            Restaurar ocultos
          </button>
        </div>

        <div className="mt-5 border-t border-line pt-5">
          <p className="mb-2 text-xs font-medium text-ink">Discord Rich Presence</p>
          <p className="mb-3 text-xs leading-relaxed text-muted">
            Muestra a qué juegas en tu estado de Discord. Ya viene activado para todos;
            este campo es <span className="text-ink">opcional</span>: pon tu propio{' '}
            <span className="text-ink">Application ID</span> de
            discord.com/developers/applications solo si quieres usar tu app en vez de la de
            Meteor. Vacío = se usa la de Meteor.
          </p>
          <div className="flex gap-2">
            <input
              value={discordId}
              onChange={(e) => setDiscordId(e.target.value)}
              placeholder="Application ID (solo números)"
              inputMode="numeric"
              className="w-full rounded-lg border border-line bg-elevated px-3 py-2.5 text-sm text-ink outline-none placeholder:text-muted/60 focus:border-accent/60"
            />
            <button
              onClick={saveDiscord}
              disabled={busy}
              className="shrink-0 rounded-lg bg-accent px-4 py-2.5 text-sm font-semibold text-white transition hover:bg-accent-soft disabled:opacity-50"
            >
              {discordSaved ? 'Guardado ✓' : 'Guardar'}
            </button>
          </div>
        </div>

        {error && <p className="mt-4 text-sm text-accent">{error}</p>}

        <div className="mt-5 flex justify-end">
          <button
            onClick={onClose}
            className="rounded-lg px-4 py-2 text-sm text-muted hover:text-ink"
          >
            Cerrar
          </button>
        </div>
      </div>
    </div>
  );
}
