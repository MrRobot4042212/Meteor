export type GameSource = 'steam' | 'manual';

export interface Game {
  id: string;
  name: string;
  source: GameSource;
  app_id?: number | null;
  executable?: string | null;
  install_dir?: string | null;
  cover_url?: string | null;
}
