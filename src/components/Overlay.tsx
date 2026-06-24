'use client';

import { useEffect, useState } from 'react';
import { listen } from '@tauri-apps/api/event';
import { getAppSettings } from '@/lib/tauri';
import type { MetricsSample, OverlaySettings } from '@/lib/types';

/** Corner placement → fixed-position classes. */
const CORNER: Record<string, string> = {
  'top-left': 'top-3 left-3',
  'top-right': 'top-3 right-3',
  'bottom-left': 'bottom-3 left-3',
  'bottom-right': 'bottom-3 right-3',
};

/** One metric line in the HUD. */
function Row({ label, value, accent }: { label: string; value: string; accent?: string }) {
  return (
    <div className="flex items-baseline justify-between gap-4 leading-tight">
      <span className="text-[10px] uppercase tracking-wider text-white/55">{label}</span>
      <span className={`text-sm font-semibold tabular-nums ${accent ?? 'text-white'}`}>
        {value}
      </span>
    </div>
  );
}

/** Color a temperature by how hot it is. */
function tempColor(c: number): string {
  if (c >= 85) return 'text-red-400';
  if (c >= 75) return 'text-amber-400';
  return 'text-emerald-400';
}

/**
 * The in-game metrics HUD. Rendered in the dedicated transparent, click-through
 * `overlay` window (see `page.tsx` window-label branch). Listens for telemetry
 * samples from the Rust sampler and paints a compact panel in the chosen corner.
 */
export function Overlay() {
  const [m, setM] = useState<MetricsSample | null>(null);
  const [cfg, setCfg] = useState<OverlaySettings | null>(null);

  useEffect(() => {
    // The overlay window must be see-through; force the document transparent
    // (globals.css paints an opaque background on the main window).
    document.documentElement.style.background = 'transparent';
    document.body.style.background = 'transparent';

    const loadCfg = () =>
      getAppSettings()
        .then((s) => setCfg(s.overlay))
        .catch(() => {});
    loadCfg();

    const unSample = listen<MetricsSample>('metrics-sample', (e) => setM(e.payload));
    const unCfg = listen('overlay-config', loadCfg);
    return () => {
      unSample.then((f) => f());
      unCfg.then((f) => f());
    };
  }, []);

  if (!m || !cfg) return null;

  const gb = (mb: number) => (mb / 1024).toFixed(1);

  const rows: { label: string; value: string; accent?: string }[] = [];
  if (cfg.show_fps && m.fps != null) {
    rows.push({ label: 'FPS', value: m.fps.toFixed(0), accent: 'text-accent' });
  }
  if (cfg.show_frametime && m.frametime_ms != null) {
    rows.push({ label: 'Frame', value: `${m.frametime_ms.toFixed(1)} ms` });
  }
  if (cfg.show_gpu && m.gpu_usage != null) {
    rows.push({ label: 'GPU', value: `${m.gpu_usage}%`, accent: 'text-info' });
  }
  if (cfg.show_gpu_temp && m.gpu_temp_c != null) {
    rows.push({ label: 'GPU °C', value: `${m.gpu_temp_c}°`, accent: tempColor(m.gpu_temp_c) });
  }
  if (cfg.show_vram && m.vram_used_mb != null && m.vram_total_mb != null) {
    rows.push({ label: 'VRAM', value: `${gb(m.vram_used_mb)}/${gb(m.vram_total_mb)} GB` });
  }
  if (cfg.show_cpu) {
    rows.push({ label: 'CPU', value: `${m.cpu_usage.toFixed(0)}%`, accent: 'text-info' });
  }
  if (cfg.show_cpu_temp && m.cpu_temp_c != null) {
    rows.push({ label: 'CPU °C', value: `${m.cpu_temp_c}°`, accent: tempColor(m.cpu_temp_c) });
  }
  if (cfg.show_ram) {
    rows.push({ label: 'RAM', value: `${gb(m.ram_used_mb)}/${gb(m.ram_total_mb)} GB` });
  }

  if (rows.length === 0) return null;

  return (
    <div
      className={`pointer-events-none fixed ${CORNER[cfg.position] ?? CORNER['top-left']} select-none`}
    >
      <div className="min-w-[148px] border border-white/10 bg-black/55 px-3 py-2 font-mono shadow-lg backdrop-blur-sm">
        {m.game && (
          <div className="mb-1.5 max-w-[180px] truncate border-b border-white/10 pb-1 text-[10px] font-semibold uppercase tracking-wider text-accent">
            {m.game}
          </div>
        )}
        <div className="space-y-0.5">
          {rows.map((r) => (
            <Row key={r.label} label={r.label} value={r.value} accent={r.accent} />
          ))}
        </div>
      </div>
    </div>
  );
}
