/** Spanish labels for IGDB genre and game-mode names (a finite set). Unknown
 *  values fall back to the original so nothing ever shows blank. */

const GENRES: Record<string, string> = {
  'Point-and-click': 'Point-and-click',
  Fighting: 'Lucha',
  Shooter: 'Disparos',
  Music: 'Música',
  Platform: 'Plataformas',
  Puzzle: 'Puzles',
  Racing: 'Carreras',
  'Real Time Strategy (RTS)': 'Estrategia en tiempo real',
  'Role-playing (RPG)': 'Rol (RPG)',
  Simulator: 'Simulación',
  Sport: 'Deportes',
  Strategy: 'Estrategia',
  'Turn-based strategy (TBS)': 'Estrategia por turnos',
  Tactical: 'Táctico',
  "Hack and slash/Beat 'em up": "Hack and slash / Beat 'em up",
  'Quiz/Trivia': 'Preguntas y respuestas',
  Pinball: 'Pinball',
  Adventure: 'Aventura',
  Indie: 'Indie',
  Arcade: 'Arcade',
  'Visual Novel': 'Novela visual',
  'Card & Board Game': 'Cartas y mesa',
  MOBA: 'MOBA',
};

const MODES: Record<string, string> = {
  'Single player': 'Un jugador',
  Multiplayer: 'Multijugador',
  'Co-operative': 'Cooperativo',
  'Split screen': 'Pantalla dividida',
  'Massively Multiplayer Online (MMO)': 'MMO',
  'Battle Royale': 'Battle Royale',
};

export const translateGenre = (g: string): string => GENRES[g] ?? g;
export const translateMode = (m: string): string => MODES[m] ?? m;
