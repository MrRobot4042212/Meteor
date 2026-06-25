'use client';

import { useEffect, useState } from 'react';
import { getAppSettings, setAppSettings } from '@/lib/tauri';
import type { OverlaySettings, OverlayPosition } from '@/lib/types';
import { CloseIcon } from './icons';

const OVERLAY_POSITIONS: { value: OverlayPosition; label: string }[] = [
  { value: 'top-left', label: 'Sup. izq.' },
  { value: 'top-right', label: 'Sup. der.' },
  { value: 'bottom-left', label: 'Inf. izq.' },
  { value: 'bottom-right', label: 'Inf. der.' },
];

const OVERLAY_METRICS: { key: keyof OverlaySettings; label: string; note?: string }[] = [
  { key: 'show_fps', label: 'FPS' },
  { key: 'show_frametime', label: 'Frametime' },
  { key: 'show_gpu', label: 'Uso GPU' },
  { key: 'show_gpu_temp', label: 'Temp. GPU' },
  { key: 'show_vram', label: 'VRAM' },
  { key: 'show_cpu', label: 'Uso CPU' },
  { key: 'show_cpu_temp', label: 'Temp. CPU' },
  { key: 'show_ram', label: 'RAM' },
];

export function OverlaySettingsScreen({ onClose }: { onClose: () => void }) {
  const [overlay, setOverlay] = useState<OverlaySettings | null>(null);

  useEffect(() => {
    getAppSettings()
      .then((s) => setOverlay(s.overlay))
      .catch(() => {});
  }, []);

  function updateOverlay(patch: Partial<OverlaySettings>) {
    setOverlay((prev) => {
      if (!prev) return prev;
      const nextOverlay = { ...prev, ...patch };
      getAppSettings()
        .then((current) => setAppSettings({ ...current, overlay: nextOverlay }))
        .catch(console.error);
      return nextOverlay;
    });
  }

  // Handle Esc to close
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === 'Escape') onClose();
    };
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  }, [onClose]);

  if (!overlay) return null;

  return (
    <div className="fixed inset-0 z-[9999] flex items-center justify-center bg-void/60 backdrop-blur-md">
      <div className="relative w-full max-w-[600px] border border-line bg-surface p-7 shadow-card">
        <button
          onClick={onClose}
          className="absolute right-5 top-5 z-10 grid h-8 w-8 place-items-center text-muted transition hover:bg-elevated hover:text-ink"
          aria-label="Cerrar"
        >
          <CloseIcon className="h-5 w-5" />
        </button>

        <h2 className="mb-1 font-display text-xl font-semibold text-ink">Ajustes del Overlay</h2>
        <p className="mb-6 text-sm text-muted">Ajusta las métricas en tiempo real. Presiona <kbd className="bg-elevated px-1 font-mono text-xs">Ctrl+Shift+M</kbd> para volver al juego.</p>

        <div className="space-y-6">
          <div className="rounded border border-line bg-elevated/20 p-4">
            <h3 className="mb-3 text-sm font-medium text-ink">Posición</h3>
            <div className="flex overflow-hidden border border-line">
              {OVERLAY_POSITIONS.map((p) => (
                <button
                  key={p.value}
                  onClick={() => updateOverlay({ position: p.value })}
                  className={`flex-1 px-2 py-2 text-xs transition ${
                    overlay.position === p.value
                      ? 'bg-accent text-white'
                      : 'bg-elevated text-muted hover:text-ink'
                  }`}
                >
                  {p.label}
                </button>
              ))}
            </div>
          </div>

          <div className="rounded border border-line bg-elevated/20 p-4">
            <h3 className="mb-3 text-sm font-medium text-ink">Métricas a mostrar</h3>
            <div className="grid grid-cols-2 gap-1.5 sm:grid-cols-4">
              {OVERLAY_METRICS.map((m) => {
                const on = overlay[m.key] as boolean;
                return (
                  <button
                    key={m.key}
                    onClick={() =>
                      updateOverlay({ [m.key]: !on } as Partial<OverlaySettings>)
                    }
                    className={`flex items-center justify-between border px-2.5 py-2 text-xs transition ${
                      on ? 'border-accent/40 bg-elevated text-ink' : 'border-line text-muted'
                    }`}
                  >
                    <span>{m.label}</span>
                    <span
                      className={`h-2.5 w-2.5 border ${
                        on ? 'border-accent bg-accent' : 'border-muted'
                      }`}
                    />
                  </button>
                );
              })}
            </div>
          </div>

          <div className="grid grid-cols-2 gap-4">
            <div className="rounded border border-line bg-elevated/20 p-4">
              <h3 className="mb-3 text-sm font-medium text-ink">Tamaño de texto</h3>
              <div className="flex overflow-hidden border border-line">
                {[
                  { label: 'Pequeño', value: 'xs' },
                  { label: 'Normal', value: 'sm' },
                  { label: 'Grande', value: 'base' },
                ].map((o) => (
                  <button
                    key={o.value}
                    onClick={() => updateOverlay({ font_size: o.value })}
                    className={`flex-1 px-2 py-2 text-xs transition ${
                      overlay.font_size === o.value
                        ? 'bg-accent font-medium text-white'
                        : 'bg-elevated text-muted hover:text-ink'
                    }`}
                  >
                    {o.label}
                  </button>
                ))}
              </div>
            </div>

            <div className="rounded border border-line bg-elevated/20 p-4">
              <h3 className="mb-3 text-sm font-medium text-ink">Fondo</h3>
              <div className="flex overflow-hidden border border-line">
                {[
                  { label: '50%', value: 50 },
                  { label: '70%', value: 70 },
                  { label: '85%', value: 85 },
                  { label: '95%', value: 95 },
                ].map((o) => (
                  <button
                    key={o.value}
                    onClick={() => updateOverlay({ bg_opacity: o.value as number })}
                    className={`flex-1 px-2 py-2 text-xs transition ${
                      overlay.bg_opacity === o.value
                        ? 'bg-accent font-medium text-white'
                        : 'bg-elevated text-muted hover:text-ink'
                    }`}
                  >
                    {o.label}
                  </button>
                ))}
              </div>
            </div>
          </div>

        </div>
      </div>
    </div>
  );
}
