import {
  browserThemeStorage,
  loadThemeMode,
  resolveTheme,
  systemPrefersDark,
} from "./preference";
import { cssVariable, THEMES, TOKEN_NAMES, type ThemeName } from "./tokens";

/** The parts of the root element the theme sets; the document element satisfies it. */
export interface ThemeRoot {
  readonly style: {
    setProperty(name: string, value: string): void;
    colorScheme?: string;
  };
  readonly dataset: Record<string, string | undefined>;
}

/**
 * Sets every token of `theme` as a CSS custom property on `root`, and marks the root with the
 * theme's name and color scheme (so the controls the browser draws follow it). Applying a theme
 * only changes how things look: it touches no document and restarts nothing.
 */
export function applyTheme(root: ThemeRoot, theme: ThemeName): void {
  const tokens = THEMES[theme];
  for (const name of TOKEN_NAMES) {
    root.style.setProperty(cssVariable(name), tokens[name]);
  }
  root.style.colorScheme = theme;
  root.dataset.theme = theme;
}

/**
 * Applies the stored (or system) theme to the page before the interface is drawn, so it never
 * appears in the other theme first. Returns the theme applied.
 */
export function applyInitialTheme(): ThemeName {
  const theme = resolveTheme(
    loadThemeMode(browserThemeStorage()),
    systemPrefersDark(),
  );
  if (typeof document !== "undefined") {
    applyTheme(document.documentElement, theme);
  }
  return theme;
}
