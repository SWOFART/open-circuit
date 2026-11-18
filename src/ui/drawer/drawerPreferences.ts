import type { ComponentTypeId } from "../../generated/ComponentTypeId";

/**
 * The drawer's user preferences (docs/contracts/stage-11-drawer-and-editing.md section 3.6): kept
 * in the web view's local storage under one key, never in a project, an assignment or the Engine.
 */
export const DRAWER_STORAGE_KEY = "open-circuit.partsDrawer.v1";

export const MAX_FAVORITES = 32;
export const MAX_RECENT = 8;
export const DRAWER_MIN_WIDTH = 240;
export const DRAWER_MAX_WIDTH = 480;
export const DRAWER_DEFAULT_WIDTH = 300;

export interface DrawerPreferences {
  /** In the order the user added them. */
  readonly favorites: readonly ComponentTypeId[];
  /** Newest first. */
  readonly recent: readonly ComponentTypeId[];
  readonly docked: boolean;
  readonly width: number;
}

export const DEFAULT_DRAWER_PREFERENCES: DrawerPreferences = {
  favorites: [],
  recent: [],
  docked: false,
  width: DRAWER_DEFAULT_WIDTH,
};

/** The storage calls the drawer uses; `Storage` satisfies it. */
export interface PreferenceStorage {
  getItem(key: string): string | null;
  setItem(key: string, value: string): void;
}

export function clampDrawerWidth(width: number): number {
  return Number.isFinite(width)
    ? Math.min(DRAWER_MAX_WIDTH, Math.max(DRAWER_MIN_WIDTH, Math.round(width)))
    : DRAWER_DEFAULT_WIDTH;
}

function typeList(
  value: unknown,
  known: ReadonlySet<string>,
  limit: number,
): ComponentTypeId[] {
  if (!Array.isArray(value)) {
    return [];
  }

  const seen = new Set<string>();
  const kept: ComponentTypeId[] = [];

  for (const item of value) {
    if (typeof item === "string" && known.has(item) && !seen.has(item)) {
      seen.add(item);
      kept.push(item as ComponentTypeId);
    }
  }

  return kept.slice(0, limit);
}

/**
 * Reads the preferences, dropping whatever is unreadable or names a type that `known` does not
 * contain. Anything unusable yields the defaults; reading never throws.
 */
export function loadDrawerPreferences(
  storage: PreferenceStorage | null,
  known: ReadonlySet<string>,
): DrawerPreferences {
  try {
    const raw = storage?.getItem(DRAWER_STORAGE_KEY);
    if (!raw) {
      return DEFAULT_DRAWER_PREFERENCES;
    }

    const value: unknown = JSON.parse(raw);
    if (typeof value !== "object" || value === null || Array.isArray(value)) {
      return DEFAULT_DRAWER_PREFERENCES;
    }

    const record = value as Record<string, unknown>;
    return {
      favorites: typeList(record.favorites, known, MAX_FAVORITES),
      recent: typeList(record.recent, known, MAX_RECENT),
      docked: record.docked === true,
      width:
        typeof record.width === "number"
          ? clampDrawerWidth(record.width)
          : DRAWER_DEFAULT_WIDTH,
    };
  } catch {
    return DEFAULT_DRAWER_PREFERENCES;
  }
}

/**
 * Writes the preferences. Storage that is unavailable or full is not an error: the preferences
 * then live for the session only.
 */
export function saveDrawerPreferences(
  storage: PreferenceStorage | null,
  preferences: DrawerPreferences,
): void {
  try {
    storage?.setItem(DRAWER_STORAGE_KEY, JSON.stringify(preferences));
  } catch {
    // Session-only preferences.
  }
}

/** Adds or removes a favorite; a new one goes last and the list never exceeds the limit. */
export function toggleFavorite(
  preferences: DrawerPreferences,
  typeId: ComponentTypeId,
): DrawerPreferences {
  if (preferences.favorites.includes(typeId)) {
    return {
      ...preferences,
      favorites: preferences.favorites.filter((id) => id !== typeId),
    };
  }

  if (preferences.favorites.length >= MAX_FAVORITES) {
    return preferences;
  }

  return { ...preferences, favorites: [...preferences.favorites, typeId] };
}

/** Puts a chosen part first in the recent list. */
export function recordRecent(
  preferences: DrawerPreferences,
  typeId: ComponentTypeId,
): DrawerPreferences {
  return {
    ...preferences,
    recent: [typeId, ...preferences.recent.filter((id) => id !== typeId)].slice(
      0,
      MAX_RECENT,
    ),
  };
}
