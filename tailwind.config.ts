import type { Config } from 'tailwindcss';

const config: Config = {
  content: ['./src/**/*.{ts,tsx}'],
  theme: {
    extend: {
      colors: {
        void: '#0A0C10',
        surface: '#12151C',
        elevated: '#1A1F29',
        line: '#232A36',
        ink: '#E6E9EF',
        muted: '#8A93A3',
        accent: {
          DEFAULT: '#FF6A3D',
          soft: '#FF8A63',
        },
      },
      fontFamily: {
        display: ['var(--font-display)', 'system-ui', 'sans-serif'],
        sans: ['var(--font-body)', 'system-ui', 'sans-serif'],
      },
      boxShadow: {
        card: '0 10px 30px -12px rgba(0,0,0,0.7)',
        glow: '0 0 0 1px rgba(255,106,61,0.35), 0 14px 40px -12px rgba(255,106,61,0.35)',
      },
      borderRadius: {
        xl2: '14px',
      },
    },
  },
  plugins: [],
};

export default config;
