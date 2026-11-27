// The keyboard map (docs/contracts/stage-16-editor-improvements.md section 10): one table of every
// binding. The key handlers look a binding up here and none matches a key on its own, and the
// shortcut window is generated from this same table, so a binding that works is listed and a
// listed binding works. This module is pure: no DOM, no React.

export type Platform = "mac" | "other";

/** Where a binding applies. */
export type ShortcutScope =
  /** Anywhere in the application (a key a text field does not use is left to it, see `inText`). */
  | "anywhere"
  /** The canvas and its selection. */
  | "canvas"
  /** The search field. */
  | "search"
  /** A dialog. */
  | "dialog";

export type ShortcutGroup =
  "File" | "Edit" | "Transform" | "Find" | "Parts" | "Wiring" | "View" | "Help";

/** The groups in the order the window lists them. */
export const SHORTCUT_GROUPS: readonly ShortcutGroup[] = [
  "File",
  "Edit",
  "Transform",
  "Find",
  "Parts",
  "Wiring",
  "View",
  "Help",
];

/** One key and the modifiers it needs. */
export interface KeyChord {
  /** A letter, matched by the physical key (so it works on other layouts). */
  readonly letter?: string;
  /** A named key (`Escape`, `ArrowUp`, `?`), matched as the key it produces. */
  readonly key?: string;
  /** A physical key code (`Space`, `ShiftLeft`), for a key that is held rather than pressed. */
  readonly code?: string;
  /** Ctrl (Windows, Linux) or Cmd (macOS) is held. Absent or false: it is not. */
  readonly primary?: boolean;
  /** Shift is held; false: it is not; absent: either. */
  readonly shift?: boolean;
  /** How the key reads in the window; defaults to the letter or the key. */
  readonly label?: string;
  /**
   * The key is also taken while a text field has the focus: it is one the field does not use
   * (`Escape` closes a dialog, `F1` opens help). Every other key is left to the field.
   */
  readonly inText?: boolean;
}

export type ShortcutId =
  | "file.new"
  | "file.open"
  | "file.save"
  | "file.saveAs"
  | "edit.undo"
  | "edit.redo"
  | "edit.cut"
  | "edit.copy"
  | "edit.paste"
  | "edit.duplicate"
  | "edit.selectAll"
  | "edit.delete"
  | "edit.escape"
  | "edit.enter"
  | "edit.move"
  | "edit.moveTen"
  | "transform.rotate"
  | "transform.half"
  | "transform.mirror"
  | "find.open"
  | "search.next"
  | "search.previous"
  | "search.choose"
  | "search.close"
  | "parts.drawer"
  | "wiring.vertical"
  | "view.pan"
  | "help.shortcuts"
  | "dialog.close"
  | "dialog.focus";

export interface Shortcut {
  readonly id: ShortcutId;
  readonly group: ShortcutGroup;
  /** The message key of the description (`src/ui/messages.ts`). */
  readonly description: string;
  readonly scope: ShortcutScope;
  readonly chords: readonly KeyChord[];
  /**
   * What the window shows instead of the generated key names, for a binding of several keys that
   * reads better as one phrase (the arrow keys). `{primary}` stands for Ctrl or Cmd.
   */
  readonly shown?: readonly string[];
}

const ARROWS: readonly KeyChord[] = [
  { key: "ArrowLeft", primary: false },
  { key: "ArrowRight", primary: false },
  { key: "ArrowUp", primary: false },
  { key: "ArrowDown", primary: false },
];

function withShift(chords: readonly KeyChord[], shift: boolean): KeyChord[] {
  return chords.map((chord) => ({ ...chord, shift }));
}

/** Every binding of the application. */
export const SHORTCUTS: readonly Shortcut[] = [
  {
    id: "file.new",
    group: "File",
    description: "shortcut.file.new",
    scope: "anywhere",
    chords: [{ letter: "n", primary: true, shift: false }],
  },
  {
    id: "file.open",
    group: "File",
    description: "shortcut.file.open",
    scope: "anywhere",
    chords: [{ letter: "o", primary: true, shift: false }],
  },
  {
    id: "file.save",
    group: "File",
    description: "shortcut.file.save",
    scope: "anywhere",
    chords: [{ letter: "s", primary: true, shift: false }],
  },
  {
    id: "file.saveAs",
    group: "File",
    description: "shortcut.file.saveAs",
    scope: "anywhere",
    chords: [{ letter: "s", primary: true, shift: true }],
  },
  {
    id: "edit.undo",
    group: "Edit",
    description: "shortcut.edit.undo",
    scope: "canvas",
    chords: [{ letter: "z", primary: true, shift: false }],
  },
  {
    id: "edit.redo",
    group: "Edit",
    description: "shortcut.edit.redo",
    scope: "canvas",
    chords: [{ letter: "z", primary: true, shift: true }],
  },
  {
    id: "edit.cut",
    group: "Edit",
    description: "shortcut.edit.cut",
    scope: "canvas",
    chords: [{ letter: "x", primary: true, shift: false }],
  },
  {
    id: "edit.copy",
    group: "Edit",
    description: "shortcut.edit.copy",
    scope: "canvas",
    chords: [{ letter: "c", primary: true, shift: false }],
  },
  {
    id: "edit.paste",
    group: "Edit",
    description: "shortcut.edit.paste",
    scope: "canvas",
    chords: [{ letter: "v", primary: true, shift: false }],
  },
  {
    id: "edit.duplicate",
    group: "Edit",
    description: "shortcut.edit.duplicate",
    scope: "canvas",
    chords: [{ letter: "d", primary: true, shift: false }],
  },
  {
    id: "edit.selectAll",
    group: "Edit",
    description: "shortcut.edit.selectAll",
    scope: "canvas",
    chords: [{ letter: "a", primary: true, shift: false }],
  },
  {
    id: "edit.delete",
    group: "Edit",
    description: "shortcut.edit.delete",
    scope: "canvas",
    chords: [
      { key: "Delete", primary: false, label: "Delete" },
      { key: "Backspace", primary: false, label: "Backspace" },
    ],
  },
  {
    id: "edit.escape",
    group: "Edit",
    description: "shortcut.edit.escape",
    scope: "canvas",
    chords: [{ key: "Escape", primary: false, label: "Esc" }],
  },
  {
    id: "edit.enter",
    group: "Edit",
    description: "shortcut.edit.enter",
    scope: "canvas",
    chords: [{ key: "Enter", primary: false, label: "Enter" }],
  },
  {
    id: "edit.move",
    group: "Edit",
    description: "shortcut.edit.move",
    scope: "canvas",
    chords: withShift(ARROWS, false),
    shown: ["Arrow keys"],
  },
  {
    id: "edit.moveTen",
    group: "Edit",
    description: "shortcut.edit.moveTen",
    scope: "canvas",
    chords: withShift(ARROWS, true),
    shown: ["Shift+Arrow keys"],
  },
  {
    id: "transform.rotate",
    group: "Transform",
    description: "shortcut.transform.rotate",
    scope: "canvas",
    chords: [{ letter: "r", primary: false, shift: false }],
  },
  {
    id: "transform.half",
    group: "Transform",
    description: "shortcut.transform.half",
    scope: "canvas",
    chords: [{ letter: "r", primary: false, shift: true }],
  },
  {
    id: "transform.mirror",
    group: "Transform",
    description: "shortcut.transform.mirror",
    scope: "canvas",
    chords: [{ letter: "m", primary: false, shift: false }],
  },
  {
    id: "find.open",
    group: "Find",
    description: "shortcut.find.open",
    scope: "anywhere",
    chords: [{ letter: "f", primary: true, shift: false }],
  },
  {
    id: "search.next",
    group: "Find",
    description: "shortcut.search.next",
    scope: "search",
    chords: [{ key: "ArrowDown", label: "Down" }],
  },
  {
    id: "search.previous",
    group: "Find",
    description: "shortcut.search.previous",
    scope: "search",
    chords: [{ key: "ArrowUp", label: "Up" }],
  },
  {
    id: "search.choose",
    group: "Find",
    description: "shortcut.search.choose",
    scope: "search",
    chords: [{ key: "Enter", label: "Enter" }],
  },
  {
    id: "search.close",
    group: "Find",
    description: "shortcut.search.close",
    scope: "search",
    chords: [{ key: "Escape", label: "Esc" }],
  },
  {
    id: "parts.drawer",
    group: "Parts",
    description: "shortcut.parts.drawer",
    scope: "anywhere",
    chords: [{ key: "/", primary: false, label: "/" }],
  },
  {
    id: "wiring.vertical",
    group: "Wiring",
    description: "shortcut.wiring.vertical",
    scope: "canvas",
    chords: [
      { code: "ShiftLeft", label: "Shift" },
      { code: "ShiftRight", label: "Shift" },
    ],
    shown: ["Shift (while routing)"],
  },
  {
    id: "view.pan",
    group: "View",
    description: "shortcut.view.pan",
    scope: "canvas",
    chords: [{ code: "Space", label: "Space" }],
    shown: ["Space (hold) and drag"],
  },
  {
    id: "help.shortcuts",
    group: "Help",
    description: "shortcut.help.shortcuts",
    scope: "anywhere",
    chords: [
      { key: "F1", label: "F1", primary: false, inText: true },
      // Typing a question mark in a field is typing, not a request for help.
      { key: "?", label: "?", primary: false },
    ],
  },
  {
    id: "dialog.close",
    group: "Help",
    description: "shortcut.dialog.close",
    scope: "dialog",
    chords: [{ key: "Escape", primary: false, label: "Esc", inText: true }],
  },
  {
    id: "dialog.focus",
    group: "Help",
    description: "shortcut.dialog.focus",
    scope: "dialog",
    chords: [{ key: "Tab", primary: false, label: "Tab", inText: true }],
    shown: ["Tab", "Shift+Tab"],
  },
];

/** The parts of a keyboard event the table reads. */
export interface ShortcutEvent {
  readonly key?: string;
  readonly code?: string;
  readonly ctrlKey?: boolean;
  readonly metaKey?: boolean;
  readonly shiftKey?: boolean;
  readonly altKey?: boolean;
}

const BY_ID: ReadonlyMap<ShortcutId, Shortcut> = new Map(
  SHORTCUTS.map((shortcut) => [shortcut.id, shortcut]),
);

/** The table entry of a binding. */
export function shortcutOf(id: ShortcutId): Shortcut {
  const shortcut = BY_ID.get(id);
  if (!shortcut) {
    throw new Error(`No shortcut is registered as ${id}.`);
  }
  return shortcut;
}

function chordMatches(
  chord: KeyChord,
  event: ShortcutEvent,
  platform: Platform | undefined,
): boolean {
  // A held key (Space, Shift) is matched by its physical code, with whatever else is held.
  if (chord.code !== undefined) {
    return event.code === chord.code;
  }
  if (event.altKey) {
    return false;
  }

  // Ctrl or Cmd. With a platform, only its own key counts (Cmd on macOS, Ctrl elsewhere);
  // without one, either does.
  const held =
    platform === "mac"
      ? event.metaKey === true
      : platform === "other"
        ? event.ctrlKey === true
        : event.ctrlKey === true || event.metaKey === true;
  if ((chord.primary ?? false) !== held) {
    return false;
  }
  if (chord.shift !== undefined && chord.shift !== (event.shiftKey === true)) {
    return false;
  }

  if (chord.letter !== undefined) {
    // The physical key; an event without a code (some automation) falls back to the letter.
    return event.code !== undefined
      ? event.code === `Key${chord.letter.toUpperCase()}`
      : event.key?.toLowerCase() === chord.letter;
  }
  return chord.key !== undefined && event.key === chord.key;
}

/** Whether `event` is the binding `id`. */
export function matchesShortcut(
  event: ShortcutEvent,
  id: ShortcutId,
  platform?: Platform,
): boolean {
  return shortcutOf(id).chords.some((chord) =>
    chordMatches(chord, event, platform),
  );
}

/**
 * The binding of `scope` that `event` is, or null. With `inText` set (a text field has the
 * focus), only the keys a text field leaves alone are considered (`Escape` for a dialog, `F1` for
 * help).
 */
export function matchShortcut(
  event: ShortcutEvent,
  scope: ShortcutScope,
  platform?: Platform,
  inText = false,
): ShortcutId | null {
  return (
    SHORTCUTS.find(
      (shortcut) =>
        shortcut.scope === scope &&
        shortcut.chords.some(
          (chord) =>
            (!inText || chord.inText === true) &&
            chordMatches(chord, event, platform),
        ),
    )?.id ?? null
  );
}

/** Whether this machine is a Mac, from the platform the browser reports. */
export function detectPlatform(
  reported: string | undefined = typeof navigator === "undefined"
    ? undefined
    : navigator.platform,
): Platform {
  return /Mac|iPhone|iPad/.test(reported ?? "") ? "mac" : "other";
}

function chordText(chord: KeyChord, platform: Platform): string {
  const name = chord.label ?? chord.letter?.toUpperCase() ?? chord.key ?? "";
  return [
    chord.primary === true ? (platform === "mac" ? "Cmd" : "Ctrl") : null,
    chord.shift === true && chord.label !== "?" ? "Shift" : null,
    name,
  ]
    .filter((part) => part !== null)
    .join("+");
}

/** The keys of a binding as the running platform names them, one entry per way to press it. */
export function shortcutKeys(
  shortcut: Shortcut,
  platform: Platform,
): readonly string[] {
  return (
    shortcut.shown?.map((text) =>
      text.replace("{primary}", platform === "mac" ? "Cmd" : "Ctrl"),
    ) ?? shortcut.chords.map((chord) => chordText(chord, platform))
  );
}

/** The bindings of a group, in table order. */
export function shortcutsInGroup(group: ShortcutGroup): readonly Shortcut[] {
  return SHORTCUTS.filter((shortcut) => shortcut.group === group);
}
