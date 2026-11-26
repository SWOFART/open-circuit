import {
  addedObjects,
  clipboardText,
  copySelection,
  PASTE_OFFSET,
  parseClipboardText,
  type ClipboardSchematic,
} from "../editor/clipboard";
import { commandForIntent, type EditorCommand } from "../editor/commands";
import type { SelectionTarget } from "../editor/hitTest";
import type { ClipboardAction } from "../editor/interaction";
import type { ComponentCatalog } from "../generated/ComponentCatalog";
import type { Schematic } from "../generated/Schematic";
import type { SymbolRegistry } from "../symbols";

/**
 * What copy, cut, paste and duplicate need from the host (stage 16 contract, section 4). The
 * system clipboard and the document are the host's, so each is a function here and the module
 * is tested without a window.
 */
export interface ClipboardHost {
  readonly schematic: () => Schematic;
  readonly registry: SymbolRegistry;
  readonly catalog: () => ComponentCatalog | null;
  readonly readText: () => Promise<string>;
  readonly writeText: (text: string) => Promise<void>;
  /** The reason the open assignment refuses `command`, or null; changes nothing. */
  readonly refusal: (command: EditorCommand) => string | null;
  /** Applies `command` with the assignment's rules; the reason it was refused, or null. */
  readonly apply: (command: EditorCommand) => string | null;
  /** Starts the placement of a paste; false when there is nothing to place. */
  readonly startPaste: (schematic: ClipboardSchematic) => boolean;
  readonly select: (targets: readonly SelectionTarget[]) => void;
}

/**
 * Runs one clipboard action and returns the message to show, or null when it worked. A refused
 * action changes neither the document nor the system clipboard.
 */
export async function runClipboardAction(
  action: ClipboardAction,
  targets: readonly SelectionTarget[],
  host: ClipboardHost,
): Promise<string | null> {
  switch (action) {
    case "copy":
    case "cut": {
      const copy = copySelection(host.schematic(), targets, host.registry);
      if (!copy) {
        return "Nothing in the selection can be copied.";
      }

      // A cut that the assignment refuses must not even touch the clipboard.
      const deletion =
        action === "cut"
          ? commandForIntent(
              { kind: "deleteSelection", targets },
              host.schematic(),
              host.registry,
            )
          : null;
      const refused = deletion ? host.refusal(deletion) : null;
      if (refused) {
        return refused;
      }

      try {
        await host.writeText(clipboardText(copy));
      } catch {
        // Nothing is deleted when the copy did not reach the clipboard.
        return "The clipboard could not be written, so nothing was cut.";
      }
      return deletion ? host.apply(deletion) : null;
    }
    case "paste": {
      const catalog = host.catalog();
      if (!catalog) {
        return "The component catalog has not loaded yet.";
      }

      let text: string;
      try {
        text = await host.readText();
      } catch {
        return "The clipboard could not be read.";
      }

      const parsed = parseClipboardText(text, catalog);
      if (!parsed.ok) {
        return parsed.reason;
      }
      // The assignment's refusal comes now, not after the user has placed the paste.
      const refused = host.refusal({
        kind: "paste",
        schematic: parsed.payload.schematic,
        delta: { x: 0, y: 0 },
      });
      if (refused) {
        return refused;
      }
      return host.startPaste(parsed.payload.schematic)
        ? null
        : "The clipboard selection cannot be placed.";
    }
    case "duplicate": {
      const before = host.schematic();
      const copy = copySelection(before, targets, host.registry);
      if (!copy) {
        return "Nothing in the selection can be duplicated.";
      }

      // A copy and an immediate paste, with no placement and no change of the clipboard.
      const refused = host.apply({
        kind: "paste",
        schematic: copy,
        delta: PASTE_OFFSET,
      });
      if (refused === null) {
        host.select(addedObjects(before, host.schematic()));
      }
      return refused;
    }
  }
}
