// Design tokens for code. Every value here is a CSS custom property from tokens.css, so a theme
// changes in one place.
export const tokens = {
  color: {
    primary: "var(--color-primary)",
    onPrimary: "var(--color-on-primary)",
    surface: "var(--color-surface)",
    text: "var(--color-text)",
    muted: "var(--color-muted)",
    border: "var(--color-border)",
    success: "var(--color-success)",
    danger: "var(--color-danger)",
  },
  space: {
    1: "var(--space-1)",
    2: "var(--space-2)",
    4: "var(--space-4)",
  },
  radius: "var(--radius)",
} as const;
