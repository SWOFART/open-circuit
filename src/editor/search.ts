import type { ComponentTypeId } from "../generated/ComponentTypeId";
import type { Schematic } from "../generated/Schematic";
import type { StableId } from "../generated/StableId";
import { matchShortcut, type ShortcutEvent } from "./shortcuts";

// Canvas search (docs/contracts/stage-16-editor-improvements.md section 7). It reads the document
// and the labels and never changes either.

export type SearchKind = "component" | "rail" | "note";

export interface SearchResult {
  readonly kind: SearchKind;
  readonly id: StableId;
  /** A part's display name or designator, a rail's name, or the start of a note's text. */
  readonly name: string;
  /** The catalog name of a part's type; absent for a rail or a note. */
  readonly typeName?: string;
}

/** A result list never grows past this, so a one-letter query stays a short list. */
export const MAX_SEARCH_RESULTS = 100;
const NOTE_NAME_CHARS = 40;

function normalize(text: string): string {
  return text.trim().toLocaleLowerCase();
}

/** The first line of a note, shortened, as the name it is listed under. */
function noteName(text: string): string {
  const line = (text.split("\n")[0] ?? "").trim();
  return line.length > NOTE_NAME_CHARS
    ? `${line.slice(0, NOTE_NAME_CHARS - 1)}…`
    : line;
}

/**
 * Finds objects whose text contains `query`, case-insensitively: a part's display name and
 * designator and the catalog name of its type, a rail's name, and a note's text. An empty query
 * finds nothing. Parts come first, then rails, then notes, each in document order.
 */
export function searchDocument(
  schematic: Schematic,
  labels: ReadonlyMap<StableId, string>,
  typeName: (typeId: ComponentTypeId) => string,
  query: string,
): SearchResult[] {
  const needle = normalize(query);
  if (needle === "") {
    return [];
  }

  const designators = new Map(
    schematic.documentation.designators.map((entry) => [
      entry.componentId,
      entry.designator,
    ]),
  );
  const results: SearchResult[] = [];

  for (const component of schematic.components) {
    const designator = designators.get(component.id);
    const name = labels.get(component.id) ?? designator ?? component.typeId;
    const type = typeName(component.typeId);
    if (
      [name, designator ?? "", type].some((text) =>
        normalize(text).includes(needle),
      )
    ) {
      results.push({
        kind: "component",
        id: component.id,
        name: designator ?? name,
        typeName: type,
      });
    }
  }
  for (const rail of schematic.rails) {
    if (normalize(rail.name).includes(needle)) {
      results.push({ kind: "rail", id: rail.id, name: rail.name });
    }
  }
  for (const note of schematic.documentation.notes) {
    if (normalize(note.text).includes(needle)) {
      results.push({ kind: "note", id: note.id, name: noteName(note.text) });
    }
  }

  return results.slice(0, MAX_SEARCH_RESULTS);
}

/** What a key does in the search field. */
export type SearchKeyAction =
  | { readonly action: "move"; readonly current: number }
  | { readonly action: "choose" }
  | { readonly action: "close" };

/**
 * The effect of a key press (looked up in the table of `shortcuts.ts`) with `count` results and `current` the highlighted one: Down and Up move
 * through the results and wrap round, Enter chooses (when there is a result), Escape closes.
 * Null for any other key, or for a key with nothing to act on.
 */
export function searchKeyAction(
  event: ShortcutEvent,
  current: number,
  count: number,
): SearchKeyAction | null {
  switch (matchShortcut(event, "search")) {
    case "search.close":
      return { action: "close" };
    case "search.next":
      return count > 0
        ? { action: "move", current: (current + 1) % count }
        : null;
    case "search.previous":
      return count > 0
        ? { action: "move", current: (current - 1 + count) % count }
        : null;
    case "search.choose":
      return count > 0 ? { action: "choose" } : null;
    default:
      return null;
  }
}
