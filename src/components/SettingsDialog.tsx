'use client';

import { useEffect, useState } from 'react';
import {
  clearCoverCache,
  hiddenCount,
  restoreHidden,
  getDiscordClientId,
  setDiscordClientId,
  getAutostart,
  setAutostart,
  getAppSettings,
  setAppSettings,
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
  const [autostart, setAutostartState] = useState<boolean | null>(null);
  const [tray, setTray] = useState<boolean | null>(null);

  useEffect(() => {
    hiddenCount()
      .then(setHidden)
      .catch(() => setHidden(null));
    getDiscordClientId()
      .then(setDiscordId)
      .catch(() => {});
    getAutostart()
      .then(setAutostartState)
      .catch(() => setAutostartState(null));
    getAppSettings()
      .then((s) => setTray(s.minimize_to_tray))
      .catch(() => setTray(null));
  }, []);

  async function toggleAutostart() {
    if (autostart === null) return;
    const next = !autostart;
    setAutostartState(next); // optimistic
    try {
      await setAutostart(next);
    } catch (e) {
      setAutostartState(!next); // revert
      setError(String(e));
    }
  }

  async function toggleTray() {
    if (tray === null) return;
    const next = !tray;
    setTray(next); // optimistic
    try {
      const current = await getAppSettings();
      await setAppSettings({ ...current, minimize_to_tray: next });
    } catch (e) {
      setTray(!next); // revert
      setError(String(e));
    }
  }

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

        {autostart !== null && (
          <div className="mt-5 border-t border-line pt-5">
            <div className="mb-2 flex items-center justify-between">
              <p className="text-xs font-medium text-ink">Iniciar con Windows</p>
              <button
                onClick={toggleAutostart}
                role="switch"
                aria-checked={autostart}
                className={`relative h-6 w-11 shrink-0 border transition-colors ${
                  autostart ? 'border-accent bg-accent' : 'border-line bg-elevated'
                }`}
              >
                <span
                  className={`absolute top-1/2 h-4 w-4 -translate-y-1/2 transition-all ${
                    autostart ? 'left-6 bg-white' : 'left-1 bg-muted'
                  }`}
                />
              </button>
            </div>
            <p className="text-xs leading-relaxed text-muted">
              Meteor arranca al iniciar sesión y se queda en la bandeja del sistema.
              Así el tiempo de juego y Discord se registran aunque no abras la ventana.
              Cerrar la ventana minimiza a la bandeja; sal del todo desde su menú.
            </p>
          </div>
        )}

        {tray !== null && (
          <div className="mt-5 border-t border-line pt-5">
            <div className="mb-2 flex items-center justify-between">
              <p className="text-xs font-medium text-ink">Minimizar a la bandeja al cerrar</p>
              <button
                onClick={toggleTray}
                role="switch"
                aria-checked={tray}
                className={`relative h-6 w-11 shrink-0 border transition-colors ${
                  tray ? 'border-accent bg-accent' : 'border-line bg-elevated'
                }`}
              >
                <span
                  className={`absolute top-1/2 h-4 w-4 -translate-y-1/2 transition-all ${
                    tray ? 'left-6 bg-white' : 'left-1 bg-muted'
                  }`}
                />
              </button>
            </div>
            <p className="text-xs leading-relaxed text-muted">
              Si está activo, al cerrar Meteor la aplicación seguirá en segundo plano para registrar tu tiempo de juego.
            </p>
          </div>
        )}
        {error && <p className="mt-4 text-sm text-accent">{error}</p>}
      </div>
    </div>
  );
}
