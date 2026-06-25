'use client';

import { useEffect, useState } from 'react';
import { MeteorIcon } from './icons';

/** A brief intro splash that plays every time the app starts. */
export function IntroSplash({ onFinish }: { onFinish: () => void }) {
  const [exiting, setExiting] = useState(false);

  useEffect(() => {
    // Show the intro for 1.5 seconds, then begin fade out.
    const t1 = window.setTimeout(() => setExiting(true), 1500);
    // After 500ms fade transition, unmount.
    const t2 = window.setTimeout(() => onFinish(), 2000);

    return () => {
      window.clearTimeout(t1);
      window.clearTimeout(t2);
    };
  }, [onFinish]);

  return (
    <div
      className={`fixed inset-0 z-[200] flex flex-col items-center justify-center bg-void text-ink transition-opacity duration-500 ease-in-out no-select ${
        exiting ? 'opacity-0 pointer-events-none' : 'opacity-100'
      }`}
    >
      <div className="flex flex-col items-center animate-fade-in">
        <MeteorIcon className="h-20 w-20 text-accent mb-6 animate-pulse drop-shadow-[0_0_15px_rgba(var(--ring),0.6)]" />
        <h1 className="font-display text-5xl md:text-7xl font-bold tracking-[0.3em] text-ink drop-shadow-[0_0_20px_rgba(var(--ring),0.4)]">
          METEOR
        </h1>
        <p className="mt-4 font-display text-xs uppercase tracking-[0.4em] text-muted">
          Biblioteca unificada
        </p>
      </div>
    </div>
  );
}
