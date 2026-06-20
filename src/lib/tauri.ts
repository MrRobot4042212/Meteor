import { invoke } from '@tauri-apps/api/core';
import type { Game } from './types';

/** Steam games + manually-added apps, sorted by name. */
export const getLibrary = () => invoke<Game[]>('get_library');

/** Launch a Steam game or manual app. */
export const launchGame = (game: Game) => invoke<void>('launch_game', { game });

/** Add a manual app. `coverUrl` maps to the Rust `cover_url` argument. */
export const addManualApp = (name: string, executable: string, coverUrl?: string) =>
  invoke<Game>('add_manual_app', { name, executable, coverUrl: coverUrl ?? null });

/** Remove a manual app by id (Steam entries are ignored server-side). */
export const removeGame = (id: string) => invoke<void>('remove_game', { id });
