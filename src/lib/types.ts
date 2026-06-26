export type GameSource =
  | 'steam'
  | 'epic'
  | 'gog'
  | 'ea'
  | 'ubisoft'
  | 'xbox'
  | 'battlenet'
  | 'riot'
  | 'rockstar'
  | 'amazon'
  | 'battlestate'
  | 'windows'
  | 'app'
  | 'manual';

/** A user-created category with an optional bundled-icon key. */
export interface Category {
  name: string;
  icon?: string | null;
}

/** A related game suggestion. */
interface SimilarGame {
  name: string;
  cover_url?: string | null;
}

/** Time-to-beat in seconds (IGDB aggregated). */
interface TimeToBeat {
  hastily?: number | null;
  normally?: number | null;
  completely?: number | null;
}

/** Rich IGDB metadata for the detail page. */
export interface GameDetails {
  summary?: string | null;
  rating?: number | null;
  rating_count?: number | null;
  release_year?: number | null;
  genres: string[];
  modes: string[];
  developer?: string | null;
  publisher?: string | null;
  screenshots: string[];
  themes?: string[];
  perspectives?: string[];
  franchise?: string | null;
  artworks?: string[];
  /** YouTube video ids for trailers. */
  videos?: string[];
  similar?: SimilarGame[];
  time_to_beat?: TimeToBeat | null;
  websites?: { category: number; url: string }[];
}

/** One finished play session (unix timestamps, seconds). */
export interface Session {
  start: number;
  end: number;
}

/** Accumulated play stats for a game. */
export interface PlayStat {
  seconds: number;
  last_played?: number | null;
  history: Session[];
}

export interface Game {
  id: string;
  name: string;
  source: GameSource;
  app_id?: number | null;
  executable?: string | null;
  install_dir?: string | null;
  cover_url?: string | null;
  launch_uri?: string | null;
  /** User overlay: marked as favorite. */
  favorite?: boolean;
  /** User overlay: manual categories assigned to this entry. */
  categories?: string[];
  /** Client-side only: extracted exe icon path (apps without cover/brand logo).
   *  Not part of the backend model; filled in lazily by `useLibrary`. */
  icon?: string;
}

export type OverlayPosition = 'top-left' | 'top-right' | 'bottom-left' | 'bottom-right';

/** In-game metrics overlay configuration (part of AppSettings). */
export interface OverlaySettings {
  enabled: boolean;
  position: OverlayPosition;
  interval_ms: number;
  show_fps: boolean;
  show_frametime: boolean;
  show_gpu: boolean;
  show_gpu_temp: boolean;
  show_vram: boolean;
  show_cpu: boolean;
  show_cpu_temp: boolean;
  show_ram: boolean;
  /** Which GPU to sample: "auto" | "nvml:<i>" | "adlx:<i>". */
  gpu: string;
  /** CSS hex color for metric labels ("FPS", "GPU"…). */
  label_color: string;
  /** CSS hex color for metric values (numbers). */
  value_color: string;
  /** CSS hex color for accented values (FPS, GPU%, CPU%) and the game title. */
  accent_color: string;
  /** HUD background opacity 0–100. */
  bg_opacity: number;
  /** Font size key: "xs" | "sm" | "base". */
  font_size: string;
}

export interface ShortcutsSettings {
  spotlight: string;
  overlay_toggle: string;
  overlay_settings: string;
}

export interface AppSettings {
  setup_completed: boolean;
  minimize_to_tray: boolean;
  overlay: OverlaySettings;
  shortcuts: ShortcutsSettings;
  /** UI language: "system" (follow OS, fallback English), "es" or "en". */
  language: string;
}

/** A GPU as reported by `system_info`. `key` is set only for metric-capable GPUs. */
export interface GpuInfo {
  name: string;
  vendor: string;
  vram_mb: number;
  kind: string;
  key: string;
}

export interface DiskInfo {
  name: string;
  fs: string;
  total_mb: number;
  available_mb: number;
}

export interface DisplayInfo {
  name: string;
  width: number;
  height: number;
  refresh_hz: number;
  primary: boolean;
}

/** Hardware/system info for the "Mi equipo" panel. */
export interface SystemInfo {
  cpu: string;
  cpu_cores: number;
  cpu_threads: number;
  ram_total_mb: number;
  os: string;
  motherboard: string | null;
  gpus: GpuInfo[];
  disks: DiskInfo[];
  displays: DisplayInfo[];
}

/** One telemetry sample pushed to the overlay window via the `metrics-sample` event. */
export interface MetricsSample {
  game?: string | null;
  cpu_usage: number;
  ram_used_mb: number;
  ram_total_mb: number;
  gpu_usage?: number | null;
  gpu_temp_c?: number | null;
  vram_used_mb?: number | null;
  vram_total_mb?: number | null;
  gpu_clock_mhz?: number | null;
  gpu_power_w?: number | null;
  /** CPU temperature from the LibreHardwareMonitor sidecar (admin + driver). */
  cpu_temp_c?: number | null;
  /** FPS / frametime arrive from the PresentMon integration (later phase). */
  fps?: number | null;
  frametime_ms?: number | null;
}

