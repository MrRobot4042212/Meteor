'use client';

import { useEffect, useState } from 'react';
import { listen } from '@tauri-apps/api/event';
import { getAppSettings } from '@/lib/tauri';

/** A small key/badge chip. */
function Kbd({ children }: { children: React.ReactNode }) {
  return (
    <kbd className="border border-line bg-elevated px-1.5 py-0.5 font-mono text-[10px] leading-none text-ink">
      {children}
    </kbd>
  );
}

function Shortcut({ keys, label }: { keys: React.ReactNode[]; label: string }) {
  return (
    <span className="flex shrink-0 items-center gap-1.5 border border-line bg-elevated px-1.5 py-0.5 font-mono text-[10px] leading-none text-ink">
      <span className="flex items-center gap-1">
        {keys.map((k, i) => (
          <Kbd key={i}>{k}</Kbd>
        ))}
      </span>
      <span className="text-muted">{label}</span>
    </span>
  );
}

/** Footer toolbar listing the app's keyboard/interaction shortcuts. */
export function Footer() {
  const [spotlightShortcut, setSpotlightShortcut] = useState<string[]>(['Ctrl', 'Shift', 'Espacio']);

  useEffect(() => {
    const fetchSettings = async () => {
      try {
        const settings = await getAppSettings();
        if (settings.shortcuts?.spotlight) {
          const parts = settings.shortcuts.spotlight.split('+').map(p => {
            if (p === 'CommandOrControl') return 'Ctrl';
            if (p === 'Space') return 'Espacio';
            return p;
          });
          setSpotlightShortcut(parts);
        }
      } catch {
        // ignore
      }
    };

    fetchSettings();
    const un = listen('settings-updated', fetchSettings);
    return () => {
      un.then(f => f());
    };
  }, []);

  return (
    <footer className="flex shrink-0 items-center gap-5 overflow-x-auto border-t border-line bg-sidebar px-4 py-2 text-[11px] text-muted">
      <Shortcut keys={spotlightShortcut} label="Spotlight" />
      <Shortcut keys={['Clic der.']} label="Acciones" />
      <Shortcut keys={['Ctrl', 'clic']} label="Seleccionar varios" />
      <Shortcut keys={['Arrastrar']} label="Categorizar / reordenar" />
      <Shortcut keys={['↑', '↓', '↵']} label="Navegar el buscador" />
      <Shortcut keys={['Esc']} label="Cerrar / volver" />
    </footer>
  );
}
