/** @anchor infra:tailwind-preset
 * @tags infra
 */

/** Shared Testudo design tokens — imported by testudo-web and testudo-journal */
export default {
  theme: {
    extend: {
      colors: {
        'main-bg': 'rgb(var(--bg-core) / <alpha-value>)',
        'container-bg': 'rgb(var(--bg-panel) / <alpha-value>)',
        'container-bg-hover': 'rgb(var(--bg-elevated) / <alpha-value>)',
        'elevated': 'rgb(var(--bg-elevated) / <alpha-value>)',
        'container-border': 'rgb(var(--border) / <alpha-value>)',
        'border-active': 'rgb(var(--border-active-accent) / <alpha-value>)',
        'accent-steel': 'rgb(var(--accent-steel) / <alpha-value>)',
        'accent-steel-hover': 'rgb(var(--accent-steel-hover) / <alpha-value>)',
        'accent-primary': 'rgb(var(--accent-primary) / <alpha-value>)',
        'signal-green': 'rgb(var(--signal-green) / <alpha-value>)',
        'signal-red': 'rgb(var(--signal-red) / <alpha-value>)',
        'signal-amber': 'rgb(var(--signal-amber) / <alpha-value>)',
        'text-primary': 'rgb(var(--text-primary) / <alpha-value>)',
        'text-secondary': 'rgb(var(--text-secondary) / <alpha-value>)',
        'text-tertiary': 'rgb(var(--text-tertiary) / <alpha-value>)',
      },
      fontFamily: {
        display: ['Space Grotesk', 'system-ui', 'sans-serif'],
        mono: ['Space Mono', 'JetBrains Mono', 'monospace'],
      },
      letterSpacing: {
        section: '0.2em',
      },
    },
  },
}
