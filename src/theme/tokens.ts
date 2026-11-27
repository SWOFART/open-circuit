/**
 * The colors of the interface and of the canvas, as one set of named tokens per theme
 * (docs/contracts/stage-16-editor-improvements.md section 8). The interface reads them as CSS
 * custom properties (`--surface-page` and so on, set on the root element); the canvas renderer is
 * handed the same values. No component hard-codes a color. The drawing exports and printing keep
 * their own fixed light colors and do not use these.
 *
 * Every value is `#rrggbb`, except the two that carry an alpha (`shadow`, `scrim`).
 */

export type ThemeName = "light" | "dark";

/** The token names; adding one here makes both palettes below require it. */
export const TOKEN_NAMES = [
  // Surfaces: what a piece of the interface is drawn on.
  "surface-page",
  "surface-panel",
  "surface-raised",
  "surface-inset",
  "surface-hover",
  "surface-selected",
  "surface-faint",
  "surface-track",
  "surface-control",
  "surface-muted",
  "surface-accent",
  "surface-warning",
  "surface-danger",
  "surface-danger-faint",
  "surface-success",
  // Text.
  "text",
  "text-soft",
  "text-muted",
  "text-accent",
  "text-on-accent",
  "text-warning",
  "text-warning-soft",
  "text-warning-strong",
  "text-danger",
  "text-danger-strong",
  "text-success",
  // Lines: borders, outlines and strokes. `line` and `line-faint` are decoration; the others
  // outline a control, a state or a focused element.
  "line",
  "line-faint",
  "line-control",
  "line-dashed",
  "line-neutral",
  "line-focus",
  "line-accent",
  "line-warning",
  "line-warning-strong",
  "line-danger",
  "line-success",
  // Shadow and the scrim behind a dialog.
  "shadow",
  "scrim",
  // The canvas.
  "canvas-background",
  "canvas-grid",
  "canvas-wire",
  "canvas-rail",
  "canvas-symbol",
  "canvas-reference",
  "canvas-junction",
  "canvas-label",
  "canvas-selected",
  "canvas-selected-label",
  "canvas-detached",
  "canvas-cursor",
  "canvas-preview",
  "canvas-probe-first",
  "canvas-probe-second",
  // The traces of the oscilloscope, in the order a selection takes them.
  "trace-1",
  "trace-2",
  "trace-3",
  "trace-4",
  "trace-5",
  "trace-6",
  "trace-7",
  "trace-8",
] as const;

export type TokenName = (typeof TOKEN_NAMES)[number];
export type ThemeTokens = Readonly<Record<TokenName, string>>;

const DARK: ThemeTokens = {
  "surface-page": "#0b1120",
  "surface-panel": "#111827",
  "surface-raised": "#1f2937",
  "surface-inset": "#0f172a",
  "surface-hover": "#1e293b",
  "surface-selected": "#1e3a5f",
  "surface-faint": "#1f2a3d",
  "surface-track": "#273449",
  "surface-control": "#334155",
  "surface-muted": "#9ca3af",
  "surface-accent": "#93c5fd",
  "surface-warning": "#fbbf24",
  "surface-danger": "#7f1d1d",
  "surface-danger-faint": "#451a1a",
  "surface-success": "#14532d",

  text: "#e5e7eb",
  "text-soft": "#cbd5e1",
  "text-muted": "#9ca3af",
  "text-accent": "#bfdbfe",
  "text-on-accent": "#0b1120",
  "text-warning": "#fbbf24",
  "text-warning-soft": "#fde68a",
  "text-warning-strong": "#fcd34d",
  "text-danger": "#fca5a5",
  "text-danger-strong": "#fecaca",
  "text-success": "#bbf7d0",

  line: "#273449",
  "line-faint": "#1f2a3d",
  "line-control": "#6b7a90",
  "line-dashed": "#6b7a90",
  "line-neutral": "#64748b",
  "line-focus": "#60a5fa",
  "line-accent": "#93c5fd",
  "line-warning": "#fbbf24",
  "line-warning-strong": "#f59e0b",
  "line-danger": "#f87171",
  "line-success": "#4ade80",

  shadow: "rgb(0 0 0 / 45%)",
  scrim: "rgb(3 7 18 / 72%)",

  "canvas-background": "#111827",
  "canvas-grid": "#263244",
  "canvas-wire": "#60a5fa",
  "canvas-rail": "#34d399",
  "canvas-symbol": "#e5e7eb",
  "canvas-reference": "#94a3b8",
  "canvas-junction": "#60a5fa",
  "canvas-label": "#cbd5e1",
  "canvas-selected": "#fbbf24",
  "canvas-selected-label": "#fbbf24",
  "canvas-detached": "#f87171",
  "canvas-cursor": "#94a3b8",
  "canvas-preview": "#93c5fd",
  "canvas-probe-first": "#fb7185",
  "canvas-probe-second": "#e2e8f0",

  "trace-1": "#60a5fa",
  "trace-2": "#f59e0b",
  "trace-3": "#34d399",
  "trace-4": "#f472b6",
  "trace-5": "#a78bfa",
  "trace-6": "#f87171",
  "trace-7": "#22d3ee",
  "trace-8": "#a3e635",
};

const LIGHT: ThemeTokens = {
  "surface-page": "#f1f5f9",
  "surface-panel": "#ffffff",
  "surface-raised": "#e2e8f0",
  "surface-inset": "#f8fafc",
  "surface-hover": "#e2e8f0",
  "surface-selected": "#dbeafe",
  "surface-faint": "#e5e7eb",
  "surface-track": "#cbd5e1",
  "surface-control": "#cbd5e1",
  "surface-muted": "#4b5563",
  "surface-accent": "#1d4ed8",
  "surface-warning": "#b45309",
  "surface-danger": "#fecaca",
  "surface-danger-faint": "#fee2e2",
  "surface-success": "#bbf7d0",

  text: "#111827",
  "text-soft": "#334155",
  "text-muted": "#4b5563",
  "text-accent": "#1d4ed8",
  "text-on-accent": "#ffffff",
  "text-warning": "#92400e",
  "text-warning-soft": "#78350f",
  "text-warning-strong": "#92400e",
  "text-danger": "#b91c1c",
  "text-danger-strong": "#7f1d1d",
  "text-success": "#14532d",

  line: "#cbd5e1",
  "line-faint": "#e2e8f0",
  "line-control": "#64748b",
  "line-dashed": "#64748b",
  "line-neutral": "#64748b",
  "line-focus": "#1d4ed8",
  "line-accent": "#1d4ed8",
  "line-warning": "#b45309",
  "line-warning-strong": "#b45309",
  "line-danger": "#b91c1c",
  "line-success": "#15803d",

  shadow: "rgb(15 23 42 / 22%)",
  scrim: "rgb(15 23 42 / 55%)",

  "canvas-background": "#ffffff",
  "canvas-grid": "#cbd5e1",
  "canvas-wire": "#1d4ed8",
  "canvas-rail": "#047857",
  "canvas-symbol": "#111827",
  "canvas-reference": "#475569",
  "canvas-junction": "#1d4ed8",
  "canvas-label": "#334155",
  "canvas-selected": "#b45309",
  "canvas-selected-label": "#92400e",
  "canvas-detached": "#b91c1c",
  "canvas-cursor": "#475569",
  "canvas-preview": "#2563eb",
  "canvas-probe-first": "#be123c",
  "canvas-probe-second": "#475569",

  "trace-1": "#1d4ed8",
  "trace-2": "#b45309",
  "trace-3": "#047857",
  "trace-4": "#be185d",
  "trace-5": "#6d28d9",
  "trace-6": "#b91c1c",
  "trace-7": "#0e7490",
  "trace-8": "#4d7c0f",
};

export const THEMES: Readonly<Record<ThemeName, ThemeTokens>> = {
  light: LIGHT,
  dark: DARK,
};

/** The CSS custom property that carries a token: `--surface-page`. */
export function cssVariable(name: TokenName): string {
  return `--${name}`;
}

/** A `#rrggbb` token as the number the canvas renderer takes. */
export function colorNumber(token: string): number {
  if (!/^#[0-9a-f]{6}$/i.test(token)) {
    throw new Error(`${token} is not a #rrggbb color.`);
  }
  return Number.parseInt(token.slice(1), 16);
}

/** The color of a trace as a `var()` reference, so SVG and CSS follow the theme. */
export function traceVariable(index: number): string {
  const traces = TOKEN_NAMES.filter((name) => name.startsWith("trace-"));
  return `var(${cssVariable(traces[index % traces.length]!)})`;
}
