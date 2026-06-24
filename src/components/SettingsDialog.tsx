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
  systemInfo,
  isElevated,
  restartAsAdmin,
} from '@/lib/tauri';
import type { OverlaySettings, OverlayPosition, SystemInfo } from '@/lib/types';
import { CloseIcon, InfoIcon, GearIcon, FireIcon } from './icons';

/** MB → human GB/MB string. */
function fmtMem(mb: number): string {
  if (mb >= 1024) return `${(mb / 1024).toFixed(mb >= 10240 ? 0 : 1)} GB`;
  return `${mb} MB`;
}

const OVERLAY_POSITIONS: { value: OverlayPosition; label: string }[] = [
  { value: 'top-left', label: 'Sup. izq.' },
  { value: 'top-right', label: 'Sup. der.' },
  { value: 'bottom-left', label: 'Inf. izq.' },
  { value: 'bottom-right', label: 'Inf. der.' },
];

// Metric toggles shown in the overlay settings. FPS/frametime depend on the
// PresentMon integration (later phase); flagged so expectations are clear.
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

type Tab = 'system' | 'app' | 'metrics';

const TABS: { id: Tab; label: string; icon: typeof InfoIcon }[] = [
  { id: 'metrics', label: 'Métricas', icon: FireIcon },
  { id: 'system', label: 'Información del sistema', icon: InfoIcon },
  { id: 'app', label: 'Preferencias de aplicación', icon: GearIcon },
];

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
  const [overlay, setOverlay] = useState<OverlaySettings | null>(null);
  const [sys, setSys] = useState<SystemInfo | null>(null);
  const [elevated, setElevated] = useState<boolean | null>(null);
  const [tab, setTab] = useState<Tab>('system');
  // Drives the slide-in: false on first paint, flipped true after mount.
  const [shown, setShown] = useState(false);

  useEffect(() => {
    setShown(true);
    systemInfo()
      .then(setSys)
      .catch(() => setSys(null));
    isElevated()
      .then(setElevated)
      .catch(() => setElevated(null));
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
      .then((s) => {
        setTray(s.minimize_to_tray);
        setOverlay(s.overlay);
      })
      .catch(() => setTray(null));
  }, []);

  // Esc closes the drawer.
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === 'Escape') onClose();
    };
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  }, [onClose]);

  // Patch the overlay config: optimistic local update, then persist (merging onto
  // the freshest settings so we don't clobber other fields like minimize_to_tray).
  async function updateOverlay(patch: Partial<OverlaySettings>) {
    setOverlay((prev) => (prev ? { ...prev, ...patch } : prev));
    try {
      const current = await getAppSettings();
      await setAppSettings({ ...current, overlay: { ...current.overlay, ...patch } });
    } catch (e) {
      setError(String(e));
    }
  }

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

  async function restartAdmin() {
    setError(null);
    try {
      await restartAsAdmin(); // app relaunches elevated and this instance exits
    } catch (e) {
      setError(String(e));
    }
  }

  return (
    <div
      className="fixed inset-0 z-50 bg-void/70 backdrop-blur-sm"
      onClick={onClose}
    >
      {/* Drawer: slides in from the left, spans 80% of the viewport width. */}
      <div
        onClick={(e) => e.stopPropagation()}
        className={`absolute inset-y-0 left-0 flex h-full w-[80vw] max-w-[1280px] border-r border-line bg-surface shadow-card transition-transform duration-300 ease-out ${
          shown ? 'translate-x-0' : '-translate-x-full'
        }`}
      >
        {/* Vertical tab rail. */}
        <nav className="flex w-64 shrink-0 flex-col border-r border-line bg-sidebar">
          <div className="flex items-center gap-2 px-5 py-5">
            <h2 className="font-display text-lg font-semibold text-ink">Ajustes</h2>
          </div>
          <div className="flex flex-1 flex-col gap-0.5 px-3">
            {TABS.map((t) => {
              const active = tab === t.id;
              const Icon = t.icon;
              return (
                <button
                  key={t.id}
                  onClick={() => setTab(t.id)}
                  className={`flex items-center gap-3 border-l-2 px-3 py-2.5 text-left text-sm transition ${
                    active
                      ? 'border-accent bg-elevated font-medium text-ink'
                      : 'border-transparent text-muted hover:bg-elevated/50 hover:text-ink'
                  }`}
                >
                  <Icon className="h-4 w-4 shrink-0" />
                  <span>{t.label}</span>
                </button>
              );
            })}
          </div>
        </nav>

        {/* Close button, floating top-right of the drawer. */}
        <button
          onClick={onClose}
          className="absolute right-4 top-4 z-10 grid h-9 w-9 place-items-center text-muted transition hover:bg-elevated hover:text-ink"
          aria-label="Cerrar"
        >
          <CloseIcon className="h-5 w-5" />
        </button>

        {/* Content for the active tab. */}
        <section className="flex-1 overflow-y-auto px-8 py-7">
          <div className="mx-auto max-w-3xl">
            {tab === 'system' && <SystemTab sys={sys} />}
            {tab === 'app' && (
              <AppTab
                busy={busy}
                hidden={hidden}
                autostart={autostart}
                tray={tray}
                discordId={discordId}
                discordSaved={discordSaved}
                setDiscordId={setDiscordId}
                saveDiscord={saveDiscord}
                clear={clear}
                restore={restore}
                toggleAutostart={toggleAutostart}
                toggleTray={toggleTray}
              />
            )}
            {tab === 'metrics' && (
              <MetricsTab
                sys={sys}
                overlay={overlay}
                updateOverlay={updateOverlay}
                elevated={elevated}
                restartAdmin={restartAdmin}
              />
            )}

            {error && <p className="mt-6 text-sm text-accent">{error}</p>}
          </div>
        </section>
      </div>
    </div>
  );
}

/* ------------------------------------------------------------------ Tabs --- */

function MetricsTab({
  sys,
  overlay,
  updateOverlay,
  elevated,
  restartAdmin,
}: {
  sys: SystemInfo | null;
  overlay: OverlaySettings | null;
  updateOverlay: (patch: Partial<OverlaySettings>) => void;
  elevated: boolean | null;
  restartAdmin: () => void;
}) {
  // Admin is only actually needed for CPU temp (and NVIDIA FPS via PresentMon).
  const needsAdmin = !!overlay?.show_cpu_temp;
  return (
    <div>
      <TabHeader
        title="Métricas"
        desc="Overlay de rendimiento sobre el juego y la GPU que mide."
      />
      {!overlay ? (
        <p className="text-sm text-muted">Cargando…</p>
      ) : (
        <div className="space-y-6">
          {elevated === false && (
            <div className={`border p-4 ${needsAdmin ? 'border-accent/50 bg-accent/5' : 'border-line bg-elevated/30'}`}>
              <p className="mb-1 text-sm font-medium text-ink">Permisos de administrador</p>
              <p className="mb-3 text-xs leading-relaxed text-muted">
                La <strong>temperatura de CPU</strong> y el <strong>FPS en tarjetas NVIDIA</strong>{' '}
                necesitan que Meteor se ejecute como administrador (cargan un driver/sesión del
                sistema). El resto de métricas (GPU, FPS en AMD, CPU/RAM) funcionan sin admin.
              </p>
              <Button onClick={restartAdmin} className="w-auto px-4">
                Reiniciar como administrador
              </Button>
            </div>
          )}
          {elevated === true && (
            <p className="text-xs text-info">✓ Ejecutándose como administrador.</p>
          )}
          <Card
            title="Overlay de métricas (in-game)"
            control={
              <Toggle
                on={overlay.enabled}
                onClick={() => updateOverlay({ enabled: !overlay.enabled })}
              />
            }
          >
            <p className="text-xs leading-relaxed text-muted">
              Muestra FPS, GPU/CPU, temperaturas y memoria sobre el juego mientras juegas.
              Aparece solo con un juego en marcha. Actívalo/desactívalo en cualquier
              momento con <kbd className="bg-elevated px-1">Ctrl+Shift+O</kbd>.
            </p>
          </Card>

          {overlay.enabled && (
            <>
              <Card title="Posición">
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
              </Card>

              {sys && sys.gpus.some((g) => g.key) && (
                <Card title="Tarjeta gráfica para las métricas">
                  <select
                    value={overlay.gpu}
                    onChange={(e) => updateOverlay({ gpu: e.target.value })}
                    className="w-full border border-line bg-elevated px-3 py-2 text-sm text-ink outline-none focus:border-accent"
                  >
                    <option value="auto">Automática (recomendada)</option>
                    {sys.gpus
                      .filter((g) => g.key)
                      .map((g) => (
                        <option key={g.key} value={g.key}>
                          {g.name}
                          {g.kind ? ` · ${g.kind}` : ''}
                        </option>
                      ))}
                  </select>
                </Card>
              )}

              <Card title="Métricas a mostrar">
                <div className="grid grid-cols-2 gap-1.5 sm:grid-cols-3">
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
                        <span>
                          {m.label}
                          {m.note && <span className="ml-1 text-[10px] text-muted">· {m.note}</span>}
                        </span>
                        <span
                          className={`h-2.5 w-2.5 border ${
                            on ? 'border-accent bg-accent' : 'border-muted'
                          }`}
                        />
                      </button>
                    );
                  })}
                </div>
              </Card>
            </>
          )}
        </div>
      )}
    </div>
  );
}

function SystemTab({ sys }: { sys: SystemInfo | null }) {
  return (
    <div>
      <TabHeader
        title="Información del sistema"
        desc="Resumen del hardware detectado en este equipo."
      />
      {!sys ? (
        <p className="text-sm text-muted">Cargando…</p>
      ) : (
        <div className="space-y-6">
          <Card title="Resumen">
            <dl className="space-y-2 text-sm">
              <InfoRow
                label="Procesador"
                value={`${sys.cpu} · ${sys.cpu_cores} núcleos / ${sys.cpu_threads} hilos`}
              />
              <InfoRow label="Memoria RAM" value={fmtMem(sys.ram_total_mb)} />
              <InfoRow label="Sistema" value={sys.os} />
              {sys.motherboard && <InfoRow label="Placa base" value={sys.motherboard} />}
            </dl>
          </Card>

          {sys.gpus.length > 0 && (
            <Card title="Tarjetas gráficas">
              <div className="space-y-1.5">
                {sys.gpus.map((g, i) => (
                  <ListItem
                    key={g.key || `gpu-${i}`}
                    left={
                      <>
                        {g.name}
                        {g.kind && <Tag>{g.kind}</Tag>}
                        {!g.key && <Tag>sin métricas</Tag>}
                      </>
                    }
                    right={fmtMem(g.vram_mb)}
                  />
                ))}
              </div>
            </Card>
          )}

          {sys.displays.length > 0 && (
            <Card title="Pantallas">
              <div className="space-y-1.5">
                {sys.displays.map((d, i) => (
                  <ListItem
                    key={`disp-${i}`}
                    left={
                      <>
                        {d.name}
                        {d.primary && <Tag accent>principal</Tag>}
                      </>
                    }
                    right={`${d.width}×${d.height} @ ${d.refresh_hz} Hz`}
                  />
                ))}
              </div>
            </Card>
          )}

          {sys.disks.length > 0 && (
            <Card title="Almacenamiento">
              <div className="space-y-1.5">
                {sys.disks.map((d, i) => (
                  <ListItem
                    key={`disk-${i}`}
                    left={
                      <>
                        {d.name || 'Disco'}
                        {d.fs && <Tag>{d.fs}</Tag>}
                      </>
                    }
                    right={`${fmtMem(d.total_mb - d.available_mb)} / ${fmtMem(d.total_mb)}`}
                  />
                ))}
              </div>
            </Card>
          )}
        </div>
      )}
    </div>
  );
}

function AppTab({
  busy,
  hidden,
  autostart,
  tray,
  discordId,
  discordSaved,
  setDiscordId,
  saveDiscord,
  clear,
  restore,
  toggleAutostart,
  toggleTray,
}: {
  busy: boolean;
  hidden: number | null;
  autostart: boolean | null;
  tray: boolean | null;
  discordId: string;
  discordSaved: boolean;
  setDiscordId: (v: string) => void;
  saveDiscord: () => void;
  clear: () => void;
  restore: () => void;
  toggleAutostart: () => void;
  toggleTray: () => void;
}) {
  return (
    <div>
      <TabHeader
        title="Preferencias de aplicación"
        desc="Comportamiento de Meteor, caché e integraciones."
      />
      <div className="space-y-6">
        <Card title="Carátulas">
          <p className="mb-3 text-xs leading-relaxed text-muted">
            Las carátulas se obtienen automáticamente y se guardan en disco para que
            carguen al instante. Si alguna se ve mal o quieres volver a buscarlas todas,
            vacía la caché y se descargarán de nuevo.
          </p>
          <Button onClick={clear} disabled={busy}>
            {busy ? 'Trabajando…' : 'Vaciar caché de carátulas'}
          </Button>
        </Card>

        <Card title="Juegos ocultos">
          <p className="mb-3 text-xs leading-relaxed text-muted">
            {hidden && hidden > 0
              ? `Tienes ${hidden} ${hidden === 1 ? 'juego oculto' : 'juegos ocultos'}. Restáuralos si ocultaste algo por error.`
              : 'No has ocultado nada. Usa el icono del ojo en una card para ocultar lo que no sea un juego.'}
          </p>
          <Button onClick={restore} disabled={busy || !hidden}>
            Restaurar ocultos
          </Button>
        </Card>

        {autostart !== null && (
          <Card
            title="Iniciar con Windows"
            control={<Toggle on={autostart} onClick={toggleAutostart} />}
          >
            <p className="text-xs leading-relaxed text-muted">
              Meteor arranca al iniciar sesión y se queda en la bandeja del sistema. Así
              el tiempo de juego y Discord se registran aunque no abras la ventana. Cerrar
              la ventana minimiza a la bandeja; sal del todo desde su menú.
            </p>
          </Card>
        )}

        {tray !== null && (
          <Card
            title="Minimizar a la bandeja al cerrar"
            control={<Toggle on={tray} onClick={toggleTray} />}
          >
            <p className="text-xs leading-relaxed text-muted">
              Si está activo, al cerrar Meteor la aplicación seguirá en segundo plano para
              registrar tu tiempo de juego.
            </p>
          </Card>
        )}

{/*         <Card title="Discord Rich Presence">
          <p className="mb-3 text-xs leading-relaxed text-muted">
            Opcional: usa tu propio Application ID de Discord. Déjalo vacío para usar el
            de Meteor.
          </p>
          <div className="flex gap-2">
            <input
              value={discordId}
              onChange={(e) => setDiscordId(e.target.value)}
              placeholder="Client ID (opcional)"
              className="flex-1 border border-line bg-elevated px-3 py-2 text-sm text-ink outline-none focus:border-accent"
            />
            <Button onClick={saveDiscord} disabled={busy} className="w-auto px-4">
              {discordSaved ? 'Guardado ✓' : 'Guardar'}
            </Button>
          </div>
        </Card> */}
      </div>
    </div>
  );
}



/* ----------------------------------------------------------- UI primitives --- */

function TabHeader({ title, desc }: { title: string; desc: string }) {
  return (
    <div className="mb-6">
      <h3 className="font-display text-xl font-semibold text-ink">{title}</h3>
      <p className="mt-1 text-sm text-muted">{desc}</p>
    </div>
  );
}

function Card({
  title,
  control,
  children,
}: {
  title: string;
  control?: React.ReactNode;
  children: React.ReactNode;
}) {
  return (
    <div className="border border-line bg-elevated/30 p-4">
      <div className="mb-2 flex items-center justify-between gap-3">
        <p className="text-sm font-medium text-ink">{title}</p>
        {control}
      </div>
      {children}
    </div>
  );
}

function Button({
  children,
  onClick,
  disabled,
  className = '',
}: {
  children: React.ReactNode;
  onClick: () => void;
  disabled?: boolean;
  className?: string;
}) {
  return (
    <button
      onClick={onClick}
      disabled={disabled}
      className={`w-full border border-line bg-elevated px-4 py-2.5 text-sm font-medium text-ink transition hover:border-accent/40 disabled:opacity-50 ${className}`}
    >
      {children}
    </button>
  );
}

function Toggle({ on, onClick }: { on: boolean; onClick: () => void }) {
  return (
    <button
      onClick={onClick}
      role="switch"
      aria-checked={on}
      className={`relative h-6 w-11 shrink-0 border transition-colors ${
        on ? 'border-accent bg-accent' : 'border-line bg-elevated'
      }`}
    >
      <span
        className={`absolute top-1/2 h-4 w-4 -translate-y-1/2 transition-all ${
          on ? 'left-6 bg-white' : 'left-1 bg-muted'
        }`}
      />
    </button>
  );
}

function InfoRow({ label, value }: { label: string; value: string }) {
  return (
    <div className="flex items-baseline justify-between gap-3">
      <dt className="shrink-0 text-muted">{label}</dt>
      <dd className="truncate text-right text-ink">{value}</dd>
    </div>
  );
}

function ListItem({ left, right }: { left: React.ReactNode; right: string }) {
  return (
    <div className="flex items-center justify-between border border-line bg-elevated px-3 py-2 text-sm">
      <span className="flex min-w-0 items-center truncate text-ink">{left}</span>
      <span className="ml-2 shrink-0 tabular-nums text-muted">{right}</span>
    </div>
  );
}

function Tag({ children, accent }: { children: React.ReactNode; accent?: boolean }) {
  return (
    <span className={`ml-1.5 text-[10px] ${accent ? 'text-accent' : 'text-muted'}`}>
      · {children}
    </span>
  );
}
