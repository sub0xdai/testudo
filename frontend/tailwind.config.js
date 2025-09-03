/** @type {import('tailwindcss').Config} */
module.exports = {
  darkMode: ["class"],
  content: [
    "./src/**/*.{rs,html}",
    "./index.html"
  ],
  theme: {
    container: {
      center: true,
      padding: "2rem",
      screens: {
        "2xl": "1400px",
      },
    },
    extend: {
      colors: {
        // Trading Terminal Monochromatic Color System
        'terminal-bg': 'var(--black-000)',
        'surface': {
          '00': 'var(--surface-00)',
          '01': 'var(--surface-01)',
          '02': 'var(--surface-02)',
          '03': 'var(--surface-03)',
          '04': 'var(--surface-04)',
          '05': 'var(--surface-05)',
        },
        'text': {
          'primary': 'var(--text-primary)',
          'secondary': 'var(--text-secondary)',
          'muted': 'var(--text-muted)',
          'label': 'var(--text-label)',
        },
        'roman': {
          'gold': 'var(--roman-gold)',
          'crimson': 'var(--roman-crimson)',
          'purple': 'var(--roman-purple)',
        },
        'neon': {
          'profit': 'var(--neon-profit)',
          'loss': 'var(--neon-loss)',
          'alert': 'var(--neon-alert)',
        },
        'accent': {
          'profit': 'var(--accent-profit)',
          'loss': 'var(--accent-loss)',
          'active': 'var(--accent-active)',
          'warning': 'var(--accent-warning)',
        },
        // Gray scale system
        'gray': {
          600: 'var(--gray-600)',
          700: 'var(--gray-700)',
          800: 'var(--gray-800)',
          900: 'var(--gray-900)',
          950: 'var(--gray-950)',
          980: 'var(--gray-980)',
          990: 'var(--gray-990)',
        },
        // Black to white spectrum
        'black': {
          '000': 'var(--black-000)',
          '100': 'var(--black-100)',
          '200': 'var(--black-200)',
          '300': 'var(--black-300)',
          '400': 'var(--black-400)',
          '500': 'var(--black-500)',
        },
        'white': {
          '000': 'var(--white-000)',
          '100': 'var(--white-100)',
        },
      },
      fontFamily: {
        'primary': 'var(--font-primary)',
        'mono': 'var(--font-mono)',
        'display': 'var(--font-display)',
      },
      spacing: {
        '1': 'var(--space-1)',
        '2': 'var(--space-2)',
        '3': 'var(--space-3)',
        '4': 'var(--space-4)',
        '6': 'var(--space-6)',
        '8': 'var(--space-8)',
        '12': 'var(--space-12)',
        '16': 'var(--space-16)',
        'header': 'var(--header-height)',
        'order-panel': 'var(--order-panel-width)',
        'status-panel': 'var(--status-panel-height)',
      },
      borderRadius: {
        lg: "var(--radius)",
        md: "calc(var(--radius) - 2px)",
        sm: "calc(var(--radius) - 4px)",
      },
      keyframes: {
        "accordion-down": {
          from: { height: "0" },
          to: { height: "var(--radix-accordion-content-height)" },
        },
        "accordion-up": {
          from: { height: "var(--radix-accordion-content-height)" },
          to: { height: "0" },
        },
      },
      animation: {
        "accordion-down": "accordion-down 0.2s ease-out",
        "accordion-up": "accordion-up 0.2s ease-out",
      },
    },
  },
  plugins: [],
}
