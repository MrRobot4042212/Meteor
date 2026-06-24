'use client';

import { useState, useEffect } from 'react';
import { getAutostart, setAutostart, getAppSettings, setAppSettings } from '@/lib/tauri';

const SLIDES = [
  {
    title: 'Bienvenido a Meteor',
    description: 'Tu nueva central unificada de juegos. Hemos analizado tu PC y organizado tus títulos de Steam, Epic, GOG, y aplicaciones de Windows en un solo lugar.',
  },
  {
    title: 'Búsqueda Global Ultrarrápida',
    description: 'Presiona la combinación Ctrl+Shift+Espacio desde cualquier parte (incluso si Meteor está minimizado) para buscar e iniciar tus juegos al instante mediante el Spotlight.',
  },
  {
    title: 'Personalización Total',
    description: 'Haz clic derecho en cualquier juego para ver opciones ocultas. Puedes cambiar carátulas fácilmente arrastrando una imagen desde tu equipo sobre la ventana del juego.',
  },
  {
    title: 'Comportamiento en Segundo Plano',
    description: 'Meteor funciona mejor si se queda en la bandeja del sistema, registrando tus horas de juego y sincronizando con Discord Rich Presence.',
  }
];

export function Onboarding({
  onComplete,
}: {
  onComplete: () => void;
}) {
  const [step, setStep] = useState(0);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  
  const [auto, setAuto] = useState<boolean>(true);
  const [tray, setTray] = useState<boolean>(true);

  useEffect(() => {
    Promise.all([getAutostart(), getAppSettings()])
      .then(([autostartRes, settingsRes]) => {
        setAuto(autostartRes);
        setTray(settingsRes.minimize_to_tray);
      })
      .catch((e) => console.error(e));
  }, []);

  async function handleNext() {
    if (step < SLIDES.length - 1) {
      setStep(step + 1);
    } else {
      await finish();
    }
  }

  async function finish() {
    setBusy(true);
    setError(null);
    try {
      await setAutostart(auto);
      const current = await getAppSettings();
      await setAppSettings({
        ...current,
        setup_completed: true,
        minimize_to_tray: tray,
      });
      onComplete();
    } catch (e) {
      setError(String(e));
      setBusy(false);
    }
  }

  const isLast = step === SLIDES.length - 1;

  return (
    <div className="fixed inset-0 z-[100] flex flex-col bg-void text-ink items-center justify-center p-6">
      <div className="pointer-events-none absolute left-1/2 top-1/2 -translate-x-1/2 -translate-y-1/2 w-[800px] h-[800px] bg-accent/5 blur-[120px] rounded-full" />
      
      <div className="relative w-full max-w-lg">
        {/* Header Icon */}
        <div className="mx-auto mb-6 flex h-20 w-20 items-center justify-center rounded-2xl bg-gradient-to-br from-accent to-accent-soft shadow-[0_0_40px_rgba(223,79,79,0.3)] transition-all duration-500">
          <svg xmlns="http://www.w3.org/2000/svg" width="40" height="40" viewBox="0 0 256 251">
            <path fill="#fff" d="M.439.438L219.3 232.266s7.457 5.259 13.158-.877c5.702-6.135 1.316-12.27 1.316-12.27zM69.738 22.35l166.668 179.677s7.456 5.26 13.158-.876c5.702-6.135 1.316-12.27 1.316-12.27zM21.053 69.242L187.72 248.919s7.456 5.259 13.158-.877c5.702-6.135 1.316-12.27 1.316-12.27zM128.32 41.194l116.442 125.53s5.21 3.674 9.193-.612c3.983-4.287.919-8.573.919-8.573zm-91.228 82.389l116.441 125.53s5.21 3.674 9.193-.613c3.983-4.286.919-8.572.919-8.572zM188.16 68.365l52.775 57.067s2.577 1.722 4.547-.287s.455-4.017.455-4.017zM66.229 181.43l52.775 57.067s2.577 1.722 4.547-.286s.455-4.017.455-4.017z" />
          </svg>
        </div>

        {/* Carousel Content */}
        <div className="text-center mb-10 h-32">
          <h1 className="font-display text-4xl font-bold tracking-tight text-white mb-3">
            {SLIDES[step].title}
          </h1>
          <p className="text-muted text-base leading-relaxed max-w-md mx-auto">
            {SLIDES[step].description}
          </p>
        </div>

        {/* Preferences (only shown on the last slide) */}
        <div className={`space-y-4 mb-10 transition-opacity duration-300 ${isLast ? 'opacity-100' : 'opacity-0 pointer-events-none absolute w-full'}`}>
          <label className="flex items-center justify-between cursor-pointer rounded-xl border border-line bg-surface p-5 transition hover:border-accent/40">
            <div>
              <p className="text-sm font-semibold text-ink mb-1">Iniciar con Windows</p>
              <p className="text-xs text-muted pr-8">
                Arranca automáticamente en segundo plano para registrar el tiempo de juego.
              </p>
            </div>
            <div
              role="switch"
              aria-checked={auto}
              className={`relative h-6 w-11 shrink-0 border transition-colors ${
                auto ? 'border-accent bg-accent' : 'border-line bg-elevated'
              }`}
            >
              <span
                className={`absolute top-1/2 h-4 w-4 -translate-y-1/2 transition-all ${
                  auto ? 'left-6 bg-white' : 'left-1 bg-muted'
                }`}
              />
            </div>
            <input 
              type="checkbox" 
              className="hidden" 
              checked={auto} 
              onChange={(e) => setAuto(e.target.checked)} 
            />
          </label>

          <label className="flex items-center justify-between cursor-pointer rounded-xl border border-line bg-surface p-5 transition hover:border-accent/40">
            <div>
              <p className="text-sm font-semibold text-ink mb-1">Minimizar a la bandeja al cerrar</p>
              <p className="text-xs text-muted pr-8">
                Al cerrar la ventana con la "X", Meteor seguirá funcionando en el área de notificaciones.
              </p>
            </div>
            <div
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
            </div>
            <input 
              type="checkbox" 
              className="hidden" 
              checked={tray} 
              onChange={(e) => setTray(e.target.checked)} 
            />
          </label>

          <div className="rounded-xl border border-line bg-surface p-5">
            <p className="text-sm font-semibold text-ink mb-1">Escaneo de tu biblioteca</p>
            <p className="text-xs text-muted">
              Al pulsar «Escanear», Meteor analizará tu equipo en busca de juegos y
              aplicaciones y descargará sus carátulas. Puede tardar unos segundos.
            </p>
          </div>
        </div>

        {/* Steps dots */}
        <div className={`flex justify-center gap-2 mb-8 ${isLast ? 'mt-0' : 'mt-[168px]'}`}>
          {SLIDES.map((_, i) => (
            <button
              key={i}
              onClick={() => setStep(i)}
              className={`h-2 rounded-full transition-all duration-300 ${i === step ? 'w-8 bg-accent' : 'w-2 bg-line hover:bg-muted'}`}
            />
          ))}
        </div>

        {error && <p className="mb-4 text-center text-sm text-accent">{error}</p>}

        {/* Action Button */}
        <button
          onClick={handleNext}
          disabled={busy}
          className="w-full rounded-xl bg-accent py-4 text-base font-semibold text-white transition hover:bg-accent-soft shadow-[0_0_20px_rgba(223,79,79,0.2)] disabled:opacity-50"
        >
          {busy ? 'Preparando…' : isLast ? 'Escanear mi biblioteca' : 'Siguiente'}
        </button>
      </div>
    </div>
  );
}
