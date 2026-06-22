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
  | 'windows'
  | 'manual';

/** A user-created category with an optional bundled-icon key. */
export interface Category {
  name: string;
  icon?: string | null;
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
}

/** Accumulated play stats for a game. */
export interface PlayStat {
  seconds: number;
  last_played?: number | null;
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
}
