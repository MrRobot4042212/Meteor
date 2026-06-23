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

export interface AppSettings {
  setup_completed: boolean;
  minimize_to_tray: boolean;
}

