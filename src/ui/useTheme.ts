import { useCallback, useEffect, useState } from "react";
import { applyTheme } from "../theme/apply";
import {
  browserThemeStorage,
  loadThemeMode,
  resolveTheme,
  saveThemeMode,
  systemPrefersDark,
  type ThemeMode,
} from "../theme/preference";
import type { ThemeName } from "../theme/tokens";

export interface ThemeController {
  /** The choice: System, Light or Dark. */
  readonly mode: ThemeMode;
  /** The theme in force. */
  readonly theme: ThemeName;
  readonly setMode: (mode: ThemeMode) => void;
}

/**
 * The theme: the stored choice, followed live when it is System (the operating system changing
 * its appearance changes the theme at once), applied to the page whenever it changes.
 */
export function useTheme(): ThemeController {
  const [mode, setModeState] = useState<ThemeMode>(() =>
    loadThemeMode(browserThemeStorage()),
  );
  const [dark, setDark] = useState(systemPrefersDark);
  const theme = resolveTheme(mode, dark);

  useEffect(() => {
    if (typeof window === "undefined" || !window.matchMedia) {
      return;
    }

    const query = window.matchMedia("(prefers-color-scheme: dark)");
    const onChange = (event: MediaQueryListEvent): void =>
      setDark(event.matches);
    setDark(query.matches);
    query.addEventListener("change", onChange);
    return () => query.removeEventListener("change", onChange);
  }, []);

  useEffect(() => {
    applyTheme(document.documentElement, theme);
  }, [theme]);

  const setMode = useCallback((next: ThemeMode) => {
    setModeState(next);
    saveThemeMode(browserThemeStorage(), next);
  }, []);

  return { mode, theme, setMode };
}
