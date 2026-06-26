'use client';

import { useEffect, useState } from 'react';
import { I18nextProvider } from 'react-i18next';
import { listen } from '@tauri-apps/api/event';
import { getAppSettings } from '@/lib/tauri';
import i18n, { resolveLanguage } from './config';

/**
 * Applies the saved UI language to i18next and keeps it in sync. Re-applies on
 * the `settings-updated` event so changing the language in Ajustes (or from
 * another window) updates the whole app live. Renders nothing until the first
 * language is resolved, to avoid a flash of the default English.
 */
export function I18nProvider({ children }: { children: React.ReactNode }) {
  const [ready, setReady] = useState(false);

  useEffect(() => {
    let unlisten: (() => void) | undefined;

    const apply = () =>
      getAppSettings()
        .then((s) => i18n.changeLanguage(resolveLanguage(s.language)))
        .catch(() => i18n.changeLanguage(resolveLanguage('system')))
        .finally(() => setReady(true));

    apply();
    listen('settings-updated', apply).then((f) => {
      unlisten = f;
    });

    return () => unlisten?.();
  }, []);

  if (!ready) return null;
  return <I18nextProvider i18n={i18n}>{children}</I18nextProvider>;
}
