/**
 * Turn a stored shortcut string (Tauri global-shortcut form, e.g.
 * "Control+Shift+KeyO", "F9", "Control+Shift+Space") into human-readable key
 * tokens for display: ["Ctrl", "Shift", "O"] / ["F9"] / ["Ctrl", "Shift",
 * "Espacio"]. Used wherever we show a keybinding so it always reflects the
 * user's custom binding (or our default), never a hardcoded label.
 */
export function formatShortcut(shortcut: string | undefined | null): string[] {
  if (!shortcut) return [];
  return shortcut.split('+').map((part) => {
    if (part === 'CommandOrControl' || part === 'Control' || part === 'CmdOrCtrl') return 'Ctrl';
    if (part === 'Space') return 'Espacio';
    if (part === 'Meta' || part === 'Super') return 'Win';
    // Tauri encodes letter/digit keys as KeyO / Digit5; show the bare character.
    if (part.startsWith('Key') && part.length === 4) return part.slice(3);
    if (part.startsWith('Digit') && part.length === 6) return part.slice(5);
    return part;
  });
}
