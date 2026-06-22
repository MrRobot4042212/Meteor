import type { SVGProps } from 'react';

/** Bundled icon set users can pick from when creating a custom category. Each is
 *  a monochrome stroke icon (currentColor), matching the app's icon style. The
 *  chosen key is persisted per category and resolved here for rendering. */
type P = SVGProps<SVGSVGElement>;
const base = {
  fill: 'none',
  stroke: 'currentColor',
  strokeWidth: 1.8,
  strokeLinecap: 'round' as const,
  strokeLinejoin: 'round' as const,
  viewBox: '0 0 24 24',
};

const make = (d: string) => (p: P) => (
  <svg {...base} {...p}>
    <path d={d} />
  </svg>
);

export const CATEGORY_ICONS: Record<string, (p: P) => React.ReactElement> = {
  star: make('M12 3.5l2.6 5.27 5.82.85-4.21 4.1.99 5.79L12 16.77l-5.2 2.74.99-5.79-4.21-4.1 5.82-.85z'),
  heart: make('M12 20S3 14.5 3 8.5A4 4 0 0112 6a4 4 0 019 2.5C21 14.5 12 20 12 20z'),
  flag: make('M5 21V4m0 0h11l-2 4 2 4H5'),
  bolt: make('M13 2 4 14h6l-1 8 9-12h-6z'),
  fire: make('M12 3c1 3-2 4-2 7a2 2 0 004 0c0-1 0-2-.5-3 2 1 3.5 3 3.5 6a5 5 0 11-10 0c0-4 3-5 5-10z'),
  diamond: make('M6 3h12l3 6-9 12L3 9z'),
  trophy: make('M8 4h8v4a4 4 0 01-8 0zM8 6H5a3 3 0 003 3M16 6h3a3 3 0 01-3 3M10 14h4M9 20h6M12 14v4'),
  crown: make('M4 18h16M4 18l-1-9 5 4 4-7 4 7 5-4-1 9'),
  rocket: make('M5 15c-1 2-1 4-1 4s2 0 4-1m-3-3a8 8 0 0111-11 8 8 0 01-11 11zm6-6a1.5 1.5 0 100-3 1.5 1.5 0 000 3z'),
  ghost: make('M5 21v-9a7 7 0 0114 0v9l-2.5-2-2.5 2-2-2-2 2L7 19zM9.5 11h.01M14.5 11h.01'),
  gamepad: make('M7 8h10a4 4 0 014 4l-1 4a3 3 0 01-5 1l-1-1H10l-1 1a3 3 0 01-5-1l-1-4a4 4 0 014-4zM7 12h4M9 10v4M16 11h.01M18 13h.01'),
  sword: make('M14.5 3H21v6.5l-9 9-3-3zM5 16l3 3M3 21l3-2'),
  sparkles: make('M12 4l1.5 4.5L18 10l-4.5 1.5L12 16l-1.5-4.5L6 10l4.5-1.5zM18.5 16l.6 1.8 1.9.6-1.9.6-.6 1.9-.6-1.9-1.9-.6 1.9-.6z'),
  bookmark: make('M6 3h12v18l-6-4-6 4z'),
  clock: make('M12 7v5l3 2M21 12a9 9 0 11-18 0 9 9 0 0118 0z'),
  controller: make('M3 9h18v9a1 1 0 01-1 1H4a1 1 0 01-1-1zM3 9l2-4h14l2 4M8 14h.01M16 14h.01'),
};

/** Stable ordering for the picker grid. */
export const CATEGORY_ICON_KEYS = Object.keys(CATEGORY_ICONS);
