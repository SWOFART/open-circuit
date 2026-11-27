import { readFileSync } from "node:fs";
import { join } from "node:path";
import { describe, expect, it } from "vitest";
import { message } from "../ui/messages";
import {
  detectPlatform,
  matchShortcut,
  matchesShortcut,
  SHORTCUT_GROUPS,
  SHORTCUTS,
  shortcutKeys,
  shortcutOf,
  type KeyChord,
  type ShortcutId,
} from "./shortcuts";

// The keyboard map of docs/contracts/stage-16-editor-improvements.md section 10: one table that
// the key handlers read and the shortcut window is generated from.

const root = process.cwd();
const read = (path: string) => readFileSync(join(root, path), "utf8");

/** The files whose key handling is an application binding, and so must read the table. */
const HANDLERS = [
  "src/editor/canvasInput.ts",
  "src/ui/projectShortcuts.ts",
  "src/editor/search.ts",
  "src/ui/App.tsx",
  "src/ui/drawer/usePartsDrawer.ts",
  "src/ui/ShortcutWindow.tsx",
];

function signature(chord: KeyChord): string {
  return [
    chord.letter ?? chord.key ?? chord.code,
    chord.primary === true ? "primary" : "plain",
  ].join("|");
}

/** Two chords share keys when they name the same key with overlapping modifiers. */
function overlap(a: KeyChord, b: KeyChord): boolean {
  return (
    signature(a) === signature(b) &&
    (a.shift === undefined || b.shift === undefined || a.shift === b.shift)
  );
}

describe("the table", () => {
  it("has a unique identifier, a description and at least one key for every entry", () => {
    const ids = SHORTCUTS.map(({ id }) => id);

    expect(new Set(ids).size).toBe(ids.length);
    for (const shortcut of SHORTCUTS) {
      expect(shortcut.chords.length, shortcut.id).toBeGreaterThan(0);
      // A message key that has a text, not one that falls back to itself.
      expect(message(shortcut.description), shortcut.id).not.toBe(
        shortcut.description,
      );
      expect(message(shortcut.description).length, shortcut.id).toBeGreaterThan(
        3,
      );
      expect(SHORTCUT_GROUPS, shortcut.id).toContain(shortcut.group);
    }
  });

  it("gives no two entries of one scope the same keys", () => {
    const clashes: string[] = [];
    SHORTCUTS.forEach((first, index) => {
      SHORTCUTS.slice(index + 1)
        .filter((second) => second.scope === first.scope)
        .forEach((second) => {
          for (const a of first.chords) {
            for (const b of second.chords) {
              if (overlap(a, b)) clashes.push(`${first.id} and ${second.id}`);
            }
          }
        });
    });

    expect(clashes).toEqual([]);
  });

  it("resolves every entry through its own keys, to itself", () => {
    // A chord of one entry never triggers another of the same scope.
    for (const shortcut of SHORTCUTS) {
      for (const chord of shortcut.chords) {
        const event = {
          ...(chord.letter
            ? { code: `Key${chord.letter.toUpperCase()}`, key: chord.letter }
            : {}),
          ...(chord.key ? { key: chord.key } : {}),
          ...(chord.code ? { code: chord.code } : {}),
          ctrlKey: chord.primary === true,
          shiftKey: chord.shift === true,
        };
        expect(
          matchShortcut(event, shortcut.scope),
          `${shortcut.id} ${signature(chord)}`,
        ).toBe(shortcut.id);
      }
    }
  });
});

describe("the map of the contract", () => {
  const keys = (id: ShortcutId, platform: "mac" | "other" = "other") =>
    shortcutKeys(shortcutOf(id), platform);

  it("keeps the keys that existed, named for the platform", () => {
    expect(keys("file.new")).toEqual(["Ctrl+N"]);
    expect(keys("file.open")).toEqual(["Ctrl+O"]);
    expect(keys("file.save")).toEqual(["Ctrl+S"]);
    expect(keys("file.saveAs")).toEqual(["Ctrl+Shift+S"]);
    expect(keys("edit.undo")).toEqual(["Ctrl+Z"]);
    expect(keys("edit.redo")).toEqual(["Ctrl+Shift+Z"]);
    expect(keys("edit.selectAll")).toEqual(["Ctrl+A"]);
    expect(keys("edit.delete")).toEqual(["Delete", "Backspace"]);
    expect(keys("edit.escape")).toEqual(["Esc"]);
    expect(keys("edit.enter")).toEqual(["Enter"]);
    expect(keys("parts.drawer")).toEqual(["/"]);
    expect(keys("file.save", "mac")).toEqual(["Cmd+S"]);
    expect(keys("file.saveAs", "mac")).toEqual(["Cmd+Shift+S"]);
  });

  it("has the clipboard, transform, find and help keys of stage 16", () => {
    expect(keys("edit.cut")).toEqual(["Ctrl+X"]);
    expect(keys("edit.copy")).toEqual(["Ctrl+C"]);
    expect(keys("edit.paste")).toEqual(["Ctrl+V"]);
    expect(keys("edit.duplicate")).toEqual(["Ctrl+D"]);
    expect(keys("transform.rotate")).toEqual(["R"]);
    expect(keys("transform.half")).toEqual(["Shift+R"]);
    expect(keys("transform.mirror")).toEqual(["M"]);
    expect(keys("find.open", "mac")).toEqual(["Cmd+F"]);
    expect(keys("search.next")).toEqual(["Down"]);
    expect(keys("search.previous")).toEqual(["Up"]);
    expect(keys("help.shortcuts")).toEqual(["F1", "?"]);
    expect(keys("edit.move")).toEqual(["Arrow keys"]);
    expect(keys("edit.moveTen")).toEqual(["Shift+Arrow keys"]);
    expect(keys("wiring.vertical")).toEqual(["Shift (while routing)"]);
  });
});

describe("matching a key press", () => {
  it("matches a letter by the physical key, so it works on another layout", () => {
    // A Cyrillic layout: the key labelled S produces "ы", the code is still KeyS.
    expect(
      matchShortcut({ code: "KeyS", key: "ы", ctrlKey: true }, "anywhere"),
    ).toBe("file.save");
    // An event with no code falls back to the letter.
    expect(matchShortcut({ key: "s", ctrlKey: true }, "anywhere")).toBe(
      "file.save",
    );
  });

  it("tells Shift apart where the table does", () => {
    expect(matchShortcut({ code: "KeyS", ctrlKey: true }, "anywhere")).toBe(
      "file.save",
    );
    expect(
      matchShortcut(
        { code: "KeyS", ctrlKey: true, shiftKey: true },
        "anywhere",
      ),
    ).toBe("file.saveAs");
    expect(matchShortcut({ code: "KeyR" }, "canvas")).toBe("transform.rotate");
    expect(matchShortcut({ code: "KeyR", shiftKey: true }, "canvas")).toBe(
      "transform.half",
    );
    // Escape, Enter and Delete are the same with or without Shift.
    expect(matchShortcut({ key: "Escape", shiftKey: true }, "canvas")).toBe(
      "edit.escape",
    );
  });

  it("takes Ctrl or Cmd as the table says, and either when no platform is named", () => {
    const ctrl = { code: "KeyS", ctrlKey: true };
    const meta = { code: "KeyS", metaKey: true };

    expect(matchShortcut(ctrl, "anywhere", "other")).toBe("file.save");
    expect(matchShortcut(meta, "anywhere", "other")).toBeNull();
    expect(matchShortcut(meta, "anywhere", "mac")).toBe("file.save");
    expect(matchShortcut(ctrl, "anywhere", "mac")).toBeNull();
    expect(matchShortcut(ctrl, "anywhere")).toBe("file.save");
    expect(matchShortcut(meta, "anywhere")).toBe("file.save");
  });

  it("never takes a key with Alt held, or a plain key with Ctrl held", () => {
    expect(
      matchShortcut({ code: "KeyS", ctrlKey: true, altKey: true }, "anywhere"),
    ).toBeNull();
    expect(matchShortcut({ code: "KeyR", ctrlKey: true }, "canvas")).toBeNull();
    expect(
      matchShortcut({ key: "ArrowLeft", ctrlKey: true }, "canvas"),
    ).toBeNull();
  });

  it("takes the arrow keys by one or by ten with Shift", () => {
    expect(matchShortcut({ key: "ArrowLeft" }, "canvas")).toBe("edit.move");
    expect(matchShortcut({ key: "ArrowLeft", shiftKey: true }, "canvas")).toBe(
      "edit.moveTen",
    );
  });

  it("keeps the scopes apart: Enter and Escape mean one thing on the canvas and another in search", () => {
    expect(matchShortcut({ key: "Enter" }, "canvas")).toBe("edit.enter");
    expect(matchShortcut({ key: "Enter" }, "search")).toBe("search.choose");
    expect(matchShortcut({ key: "Escape" }, "search")).toBe("search.close");
    expect(matchShortcut({ key: "Escape" }, "dialog")).toBe("dialog.close");
  });

  it("matches a held key by its code", () => {
    expect(matchesShortcut({ code: "Space" }, "view.pan")).toBe(true);
    expect(matchesShortcut({ code: "ShiftLeft" }, "wiring.vertical")).toBe(
      true,
    );
    expect(matchesShortcut({ code: "ShiftRight" }, "wiring.vertical")).toBe(
      true,
    );
    expect(matchesShortcut({ code: "KeyA" }, "view.pan")).toBe(false);
  });
});

describe("a text field", () => {
  it("leaves every binding to the field except F1 and a dialog's Escape", () => {
    expect(matchShortcut({ key: "F1" }, "anywhere", "other", true)).toBe(
      "help.shortcuts",
    );
    expect(matchShortcut({ key: "Escape" }, "dialog", "other", true)).toBe(
      "dialog.close",
    );

    for (const event of [
      { code: "KeyS", key: "s", ctrlKey: true },
      { code: "KeyF", key: "f", ctrlKey: true },
      { key: "/" },
      { key: "?" },
    ]) {
      expect(matchShortcut(event, "anywhere", "other", true)).toBeNull();
    }
    // Outside a text field the question mark opens the window.
    expect(matchShortcut({ key: "?", shiftKey: true }, "anywhere")).toBe(
      "help.shortcuts",
    );
  });
});

describe("the platform", () => {
  it("is a Mac for the platforms Apple reports, and anything else is not", () => {
    expect(detectPlatform("MacIntel")).toBe("mac");
    expect(detectPlatform("iPhone")).toBe("mac");
    expect(detectPlatform("Win32")).toBe("other");
    expect(detectPlatform("Linux x86_64")).toBe("other");
    expect(detectPlatform(undefined)).toBe("other");
  });
});

describe("the handlers read the table", () => {
  const idPattern =
    /"((?:file|edit|transform|find|search|parts|wiring|view|help|dialog)\.[A-Za-z]+)"/g;
  const known = new Set<string>(SHORTCUTS.map(({ id }) => id));

  it("names only bindings that are in the table, and every binding is named by a handler", () => {
    const named = new Set<string>();
    for (const file of HANDLERS) {
      for (const match of read(file).matchAll(idPattern)) {
        named.add(match[1]!);
        expect(known.has(match[1]!), `${file}: ${match[1]}`).toBe(true);
      }
    }

    // A binding in the table that no handler uses would be listed and not work.
    expect([...known].filter((id) => !named.has(id))).toEqual([]);
  });

  it("matches no application key on its own", () => {
    const raw =
      /\.(?:code|key)\s*[!=]==|case\s+"(?:Escape|Enter|Delete|Backspace|Space|ArrowUp|ArrowDown)"|ARROW_DELTAS\[event\.code\]/;
    const offenders = HANDLERS.filter((file) => raw.test(read(file)));

    expect(offenders).toEqual([]);
  });
});
