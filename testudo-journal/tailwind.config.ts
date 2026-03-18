import type { Config } from 'tailwindcss'

export default {
  content: [
    './index.html',
    './src/**/*.{js,ts,jsx,tsx}',
  ],
  theme: {
    extend: {
      colors: {
        'main-bg': '#050505',
        'container-bg': '#0A0A0A',
        'container-bg-hover': '#111111',
        'panel-bg': '#0A0A0A',
        'elevated': '#111111',
        'container-border': '#3F3F46',
        'border-active': '#FFFFFF',
        'signal-green': '#00FF41',
        'signal-red': '#FF003C',
        'text-primary': '#FFFFFF',
        'text-secondary': '#888888',
        'text-tertiary': '#555555',
      },
      fontFamily: {
        display: ['Space Grotesk', 'system-ui', 'sans-serif'],
        mono: ['Space Mono', 'JetBrains Mono', 'monospace'],
      },
    },
  },
  plugins: [],
} satisfies Config
