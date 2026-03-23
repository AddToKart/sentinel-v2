import type { Config } from 'tailwindcss'

export default {
  content: [
    './src/renderer/index.html',
    './src/renderer/src/**/*.{ts,tsx}',
  ],
  theme: {
    extend: {
      colors: {
        sentinel: {
          ink: '#071017',
          mist: '#8fa5b8',
          accent: '#46d6b6',
          glow: '#a7ffef',
          ice: '#6da9ff',
        },
      },
      boxShadow: {
        terminal: '0 24px 80px rgba(0, 0, 0, 0.45)',
      },
      keyframes: {
        'sentinel-fade-in': {
          '0%': { opacity: '0' },
          '100%': { opacity: '1' },
        },
        'sentinel-slide-in-left': {
          '0%': { opacity: '0', transform: 'translateX(-0.5rem)' },
          '100%': { opacity: '1', transform: 'translateX(0)' },
        },
        'sentinel-slide-in-bottom': {
          '0%': { opacity: '0', transform: 'translateY(0.5rem)' },
          '100%': { opacity: '1', transform: 'translateY(0)' },
        },
      },
      animation: {
        'sentinel-fade-in': 'sentinel-fade-in 220ms ease-out forwards',
        'sentinel-slide-in-left': 'sentinel-slide-in-left 260ms ease-out forwards',
        'sentinel-slide-in-bottom': 'sentinel-slide-in-bottom 260ms ease-out forwards',
      },
    },
  },
  plugins: [],
} satisfies Config
