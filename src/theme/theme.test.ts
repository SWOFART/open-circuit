import { readdirSync, readFileSync, statSync } from "node:fs";
import { join, relative, sep } from "node:path";
import { describe, expect, it } from "vitest";
import { applyTheme, type ThemeRoot } from "./apply";
import {
  DEFAULT_THEME_MODE,
  loadThemeMode,
  resolveTheme,
  saveThemeMode,
  THEME_STORAGE_KEY,
  type ThemeStorage,
} from "./preference";
import { cssVariable, THEMES, TOKEN_NAMES } from "./tokens";

// The theme preference and how a theme is put on the page (docs/contracts/stage-16-editor-
// improvements.md section 8).

function memoryStorage(initial?: string): ThemeStorage & {
  readonly items: Map<string, string>;
} {
  const items = new Map<string, string>();
  if (initial !== undefined) items.set(THEME_STORAGE_KEY, initial);
  return {
    items,
    getItem: (key) => items.get(key) ?? null,
    setItem: (key, value) => void items.set(key, value),
  };
}

describe("the stored preference", () => {
  it("is System when nothing is stored, and when there is no storage", () => {
    expect(loadThemeMode(memoryStorage())).toBe("system");
    expect(loadThemeMode(null)).toBe(DEFAULT_THEME_MODE);
    expect(DEFAULT_THEME_MODE).toBe("system");
  });

  it.each(["light", "dark", "system"] as const)("reads %s back", (mode) => {
    expect(loadThemeMode(memoryStorage(mode))).toBe(mode);
  });

  it.each(["", "Dark", "blue", "{}", "null", "1"])(
    "falls back to System for the unknown value %j",
    (value) => {
      expect(loadThemeMode(memoryStorage(value))).toBe("system");
    },
  );

  it("falls back to System when the storage cannot be read", () => {
    const broken: ThemeStorage = {
      getItem: () => {
        throw new Error("blocked");
      },
      setItem: () => {},
    };

    expect(loadThemeMode(broken)).toBe("system");
  });

  it("is saved under one key, as the plain name of the mode", () => {
    const storage = memoryStorage();

    saveThemeMode(storage, "dark");

    expect([...storage.items]).toEqual([["open-circuit.theme", "dark"]]);
  });

  it("lives for the session when the storage refuses to be written", () => {
    const broken: ThemeStorage = {
      getItem: () => null,
      setItem: () => {
        throw new Error("full");
      },
    };

    expect(() => saveThemeMode(broken, "light")).not.toThrow();
    expect(() => saveThemeMode(null, "light")).not.toThrow();
  });
});

describe("the theme in force", () => {
  it("follows the system for System, live, and is the choice otherwise", () => {
    expect(resolveTheme("system", true)).toBe("dark");
    expect(resolveTheme("system", false)).toBe("light");
    expect(resolveTheme("light", true)).toBe("light");
    expect(resolveTheme("dark", false)).toBe("dark");
  });
});

describe("putting a theme on the page", () => {
  function fakeRoot() {
    const properties = new Map<string, string>();
    const root: ThemeRoot & { colorSchemeSet?: string } = {
      style: {
        setProperty: (name, value) => void properties.set(name, value),
      },
      dataset: {},
    };
    return { root, properties };
  }

  it.each(["light", "dark"] as const)(
    "sets every token of the %s theme as a custom property, and names it",
    (theme) => {
      const { root, properties } = fakeRoot();

      applyTheme(root, theme);

      for (const name of TOKEN_NAMES) {
        expect(properties.get(cssVariable(name)), name).toBe(
          THEMES[theme][name],
        );
      }
      expect(properties.size).toBe(TOKEN_NAMES.length);
      expect(root.dataset.theme).toBe(theme);
      expect(root.style.colorScheme).toBe(theme);
    },
  );

  it("changes only how things look: switching back and forth leaves the same properties", () => {
    const { root, properties } = fakeRoot();

    applyTheme(root, "dark");
    const dark = new Map(properties);
    applyTheme(root, "light");
    applyTheme(root, "dark");

    expect(new Map(properties)).toEqual(dark);
  });
});

describe("no component hard-codes a color", () => {
  const source = join(process.cwd(), "src");
  // The tokens themselves, and the drawing exports and their tests, which keep fixed light
  // colors in both themes (stage 12).
  const allowed = new Set([
    join("src", "theme", "tokens.ts"),
    join("src", "editor", "drawingExport.ts"),
  ]);

  function files(directory: string): string[] {
    return readdirSync(directory).flatMap((entry) => {
      const path = join(directory, entry);
      if (entry === "generated") return [];
      if (statSync(path).isDirectory()) return files(path);
      return /\.(ts|tsx|css)$/.test(entry) && !/\.test\./.test(entry)
        ? [path]
        : [];
    });
  }

  it("keeps hex and rgb colors in the tokens and the fixed export colors only", () => {
    const offenders = files(source)
      .filter((path) => !allowed.has(relative(process.cwd(), path)))
      .filter((path) => {
        const text = readFileSync(path, "utf8");
        return (
          /#[0-9a-fA-F]{6}\b|#[0-9a-fA-F]{3}\b(?![0-9a-fA-F])/.test(text) ||
          /0x[0-9a-fA-F]{6}\b/.test(text) ||
          /\brgba?\(/.test(text)
        );
      })
      .map((path) => relative(process.cwd(), path).split(sep).join("/"));

    expect(offenders).toEqual([]);
  });

  it("uses only custom properties that a theme defines", () => {
    const css = readFileSync(join(source, "styles.css"), "utf8");
    const used = new Set(
      [...css.matchAll(/var\(--([a-z0-9-]+)/g)].map((match) => match[1]!),
    );
    const defined = new Set<string>(TOKEN_NAMES);

    // A custom property with a fallback (`var(--space-3, 12px)`) may be undefined.
    const undefinedUses = [...used].filter(
      (name) => !defined.has(name) && !new RegExp(`var\\(--${name},`).test(css),
    );
    expect(undefinedUses).toEqual([]);
  });
});
