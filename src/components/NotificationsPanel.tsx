'use client';

import { useState } from 'react';
import { CloseIcon, BookIcon } from './icons';
import { getAppSettings, setAppSettings } from '@/lib/tauri';
interface Tutorial {
  title: string;
  description: string;
}

const TUTORIALS: Tutorial[] = [
  {
    title: '¡Bienvenido a Meteor!',
    description: 'Meteor es una biblioteca unificada de juegos. Hemos analizado tu PC y añadido automáticamente los juegos de tus plataformas (Steam, Epic, GOG, etc.).',
  },
  {
    title: 'Búsqueda Global (Spotlight)',
    description: 'Presiona Ctrl+Shift+Espacio en cualquier momento (incluso con Meteor cerrado) para buscar e iniciar juegos al instante.',
  },
  {
    title: 'Añadir o cambiar carátulas',
    description: 'Si una carátula falta o no te gusta, haz clic derecho sobre el juego -> "Cambiar carátula". También puedes arrastrar una imagen desde tu PC sobre esa ventana.',
  },
  {
    title: 'Añadir aplicaciones manuales',
    description: 'Si tienes un juego o aplicación independiente (ej: emuladores o instaladores standalone), puedes usar el botón "Añadir" en la esquina superior.',
  },
];

export function NotificationsPanel({ onClose }: { onClose: () => void }) {
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  async function resetOnboarding() {
    setBusy(true);
    setError(null);
    try {
      const current = await getAppSettings();
      await setAppSettings({ ...current, setup_completed: false });
      window.location.reload();
    } catch (e) {
      setError(String(e));
      setBusy(false);
    }
  }

  return (
    <>
      <div
        className="fixed inset-0 z-40 bg-void/40 backdrop-blur-sm"
        onClick={onClose}
      />
      <div className="fixed right-0 top-0 bottom-0 z-50 w-80 border-l border-line bg-surface shadow-2xl flex flex-col">
        <div className="flex items-center justify-between border-b border-line px-5 py-4">
          <div className="flex items-center gap-2">
            <BookIcon className="h-5 w-5 text-accent" />
            <h2 className="font-display text-base font-semibold text-ink">Información y Guías</h2>
          </div>
          <button
            onClick={onClose}
            className="grid h-8 w-8 place-items-center rounded-lg text-muted hover:bg-elevated hover:text-ink"
          >
            <CloseIcon className="h-5 w-5" />
          </button>
        </div>

        <div className="flex-1 overflow-y-auto p-5 flex flex-col gap-5">
          {TUTORIALS.map((tut, i) => (
            <div key={i} className="relative rounded-xl border border-line bg-elevated p-4">
              <div className="mb-1.5 flex items-center gap-2">
                <span className="flex h-5 w-5 items-center justify-center rounded-full bg-accent/20 text-[11px] font-bold text-accent">
                  {i + 1}
                </span>
                <h3 className="text-sm font-semibold text-ink">{tut.title}</h3>
              </div>
              <p className="text-xs leading-relaxed text-muted pl-7">
                {tut.description}
              </p>
            </div>
          ))}

          <div className="mt-4 rounded-lg bg-accent/10 p-4 border border-accent/20">
            <p className="text-xs text-accent">
              ¿No encuentras algo? Haz clic derecho sobre cualquier juego o categoría para descubrir acciones ocultas.
            </p>
          </div>

          <div className="mt-4 border-t border-line pt-5">
            <button
              onClick={resetOnboarding}
              disabled={busy}
              className="w-full rounded-lg border border-line bg-elevated px-4 py-2.5 text-sm font-medium text-ink transition hover:border-accent/40 disabled:opacity-50"
            >
              Repetir configuración inicial
            </button>
            {error && <p className="mt-2 text-center text-xs text-accent">{error}</p>}
          </div>
        </div>
      </div>
    </>
  );
}
