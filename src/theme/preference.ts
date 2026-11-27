import type { ThemeName } from "./tokens";

// The theme preference (docs/contracts/stage-16-editor-improvements.md section 8): System (the
// default), Light or Dark, stored in the web view's local storage under one key as the parts
// drawer stores its own. It is never part of a project, an assignment, the Engine or the history.

export type ThemeMode = "system" | "light" | "dark";

export const THEME_MODES: readonly ThemeMode[] = ["system", "light", "dark"];
export const DEFAULT_THEME_MODE: ThemeMode = "system";
export const THEME_STORAGE_KEY = "open-circuit.theme";

/** The storage calls the preference uses; `Storage` satisfies it. */
export interface ThemeStorage {
  getItem(key: string): string | null;
  setItem(key: string, value: string): void;
}

/**
 * The stored choice. A value that cannot be read, is missing or is not one of the three modes
 * is System; storage that is not there leaves the choice to the session.
 */
export function loadThemeMode(storage: ThemeStorage | null): ThemeMode {
  try {
    const stored = storage?.getItem(THEME_STORAGE_KEY);
    return THEME_MODES.find((mode) => mode === stored) ?? DEFAULT_THEME_MODE;
  } catch {
    return DEFAULT_THEME_MODE;
  }
}

/** Remembers the choice; storage that refuses leaves it to the session. */
export function saveThemeMode(
  storage: ThemeStorage | null,
  mode: ThemeMode,
): void {
  try {
    storage?.setItem(THEME_STORAGE_KEY, mode);
  } catch {
    // Session-only preference.
  }
}

/** The theme in force: the choice, or for System what the operating system prefers. */
export function resolveTheme(
  mode: ThemeMode,
  systemPrefersDark: boolean,
): ThemeName {
  if (mode === "system") {
    return systemPrefersDark ? "dark" : "light";
  }
  return mode;
}

/** The browser's storage, or null when it is not there or refuses to be read. */
export function browserThemeStorage(): ThemeStorage | null {
  try {
    return typeof window === "undefined" ? null : window.localStorage;
  } catch {
    return null;
  }
}

/** Whether the operating system prefers a dark appearance; light when it cannot tell. */
export function systemPrefersDark(): boolean {
  try {
    return (
      typeof window !== "undefined" &&
      typeof window.matchMedia === "function" &&
      window.matchMedia("(prefers-color-scheme: dark)").matches
    );
  } catch {
    return false;
  }
}
