/** @type {import('tailwindcss').Config} */
module.exports = {
  content: [
    "./src/**/*.{rs,html}",
    "./index.html"
  ],
  theme: {
    extend: {
      fontFamily: {
        'display': ['Cinzel', 'serif'],
        'sans': ['Inter', 'sans-serif'],
      },
      colors: {
        'roman-gold': '#FFD700',
        'roman-crimson': '#8B0000',
        'roman-purple': '#4B0082',
        'terminal': {
          'bg': 'var(--black-000)',
          'panel': 'var(--black-100)',
          'card': 'var(--black-200)',
          'border': 'var(--gray-600)',
        },
        'text': {
          'primary': 'var(--text-primary)',
          'secondary': 'var(--text-secondary)',
          'muted': 'var(--text-muted)',
        }
      }
    },
  },
  plugins: [],
}