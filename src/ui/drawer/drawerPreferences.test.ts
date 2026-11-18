import { describe, expect, it } from "vitest";
import {
  DEFAULT_DRAWER_PREFERENCES,
  DRAWER_DEFAULT_WIDTH,
  DRAWER_MAX_WIDTH,
  DRAWER_MIN_WIDTH,
  DRAWER_STORAGE_KEY,
  MAX_FAVORITES,
  MAX_RECENT,
  clampDrawerWidth,
  loadDrawerPreferences,
  recordRecent,
  saveDrawerPreferences,
  toggleFavorite,
  type PreferenceStorage,
} from "./drawerPreferences";

const known = new Set(["resistor", "fuse", "diode", "capacitor"]);

function memoryStorage(initial?: string) {
  const store = {
    value: initial ?? null,
    getItem: (): string | null => store.value,
    setItem: (_key: string, value: string): void => {
      store.value = value;
    },
  };
  return store;
}

describe("drawer preferences storage", () => {
  it("round-trips the preferences", () => {
    const storage = memoryStorage();
    const preferences = {
      favorites: ["fuse", "resistor"],
      recent: ["diode"],
      docked: true,
      width: 400,
    } as const;

    saveDrawerPreferences(storage, preferences);

    expect(loadDrawerPreferences(storage, known)).toEqual(preferences);
  });

  it("starts from the defaults when nothing is stored or storage is missing", () => {
    expect(loadDrawerPreferences(memoryStorage(), known)).toEqual(
      DEFAULT_DRAWER_PREFERENCES,
    );
    expect(loadDrawerPreferences(null, known)).toEqual(
      DEFAULT_DRAWER_PREFERENCES,
    );
  });

  it.each(["not json", "[]", "7", "null", '"text"'])(
    "uses the defaults for the unreadable stored value %s",
    (raw) => {
      expect(loadDrawerPreferences(memoryStorage(raw), known)).toEqual(
        DEFAULT_DRAWER_PREFERENCES,
      );
    },
  );

  it("drops unknown types, duplicates and non-strings, and keeps the rest", () => {
    const raw = JSON.stringify({
      favorites: ["fuse", "ghost", 5, "fuse", "diode"],
      recent: "not a list",
      docked: "yes",
      width: "wide",
    });

    expect(loadDrawerPreferences(memoryStorage(raw), known)).toEqual({
      favorites: ["fuse", "diode"],
      recent: [],
      docked: false,
      width: DRAWER_DEFAULT_WIDTH,
    });
  });

  it("limits the stored lists and the width", () => {
    const many = Array.from({ length: 50 }, (_, index) => `t${index}`);
    const raw = JSON.stringify({ favorites: many, recent: many, width: 9999 });
    const loaded = loadDrawerPreferences(memoryStorage(raw), new Set(many));

    expect(loaded.favorites).toHaveLength(MAX_FAVORITES);
    expect(loaded.recent).toHaveLength(MAX_RECENT);
    expect(loaded.width).toBe(DRAWER_MAX_WIDTH);
  });

  it("never throws when the storage throws", () => {
    const broken: PreferenceStorage = {
      getItem: () => {
        throw new Error("blocked");
      },
      setItem: () => {
        throw new Error("full");
      },
    };

    expect(loadDrawerPreferences(broken, known)).toEqual(
      DEFAULT_DRAWER_PREFERENCES,
    );
    expect(() =>
      saveDrawerPreferences(broken, DEFAULT_DRAWER_PREFERENCES),
    ).not.toThrow();
  });

  it("stores one key holding only the drawer fields", () => {
    const storage = memoryStorage();
    saveDrawerPreferences(storage, DEFAULT_DRAWER_PREFERENCES);

    expect(DRAWER_STORAGE_KEY).toContain("partsDrawer");
    expect(Object.keys(JSON.parse(storage.value ?? "{}")).sort()).toEqual([
      "docked",
      "favorites",
      "recent",
      "width",
    ]);
  });
});

describe("favorites and recent parts", () => {
  it("adds favorites in order and removes them again", () => {
    let preferences = toggleFavorite(DEFAULT_DRAWER_PREFERENCES, "fuse");
    preferences = toggleFavorite(preferences, "resistor");
    expect(preferences.favorites).toEqual(["fuse", "resistor"]);

    preferences = toggleFavorite(preferences, "fuse");
    expect(preferences.favorites).toEqual(["resistor"]);
  });

  it("holds at most 32 favorites", () => {
    const full = {
      ...DEFAULT_DRAWER_PREFERENCES,
      favorites: Array.from({ length: MAX_FAVORITES }, () => "fuse" as const),
    };

    expect(toggleFavorite(full, "resistor")).toBe(full);
  });

  it("keeps the last 8 distinct chosen parts, newest first", () => {
    let preferences = DEFAULT_DRAWER_PREFERENCES;
    for (const id of ["fuse", "diode", "fuse", "resistor"] as const) {
      preferences = recordRecent(preferences, id);
    }
    expect(preferences.recent).toEqual(["resistor", "fuse", "diode"]);

    for (let index = 0; index < 12; index += 1) {
      preferences = recordRecent(preferences, `t${index}` as "fuse");
    }
    expect(preferences.recent).toHaveLength(MAX_RECENT);
    expect(preferences.recent[0]).toBe("t11");
  });

  it("clamps the width to 240 to 480 pixels", () => {
    expect(clampDrawerWidth(10)).toBe(DRAWER_MIN_WIDTH);
    expect(clampDrawerWidth(1000)).toBe(DRAWER_MAX_WIDTH);
    expect(clampDrawerWidth(333.4)).toBe(333);
    expect(clampDrawerWidth(Number.NaN)).toBe(DRAWER_DEFAULT_WIDTH);
  });
});
