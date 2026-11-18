import type { CategoryId } from "../../generated/CategoryId";
import type { ComponentCatalog } from "../../generated/ComponentCatalog";
import type { ComponentCatalogEntry } from "../../generated/ComponentCatalogEntry";
import type { ComponentTypeId } from "../../generated/ComponentTypeId";
import type { DrawerPreferences } from "./drawerPreferences";
import { searchParts } from "./partSearch";

export interface DrawerGroup {
  /** A stable key: a category ID, "favorites" or "recent". */
  readonly id: CategoryId | "favorites" | "recent";
  readonly entries: readonly ComponentCatalogEntry[];
}

export interface DrawerView {
  /** Empty search: Favorites, Recent, then the categories. */
  readonly groups: readonly DrawerGroup[];
  /** A non-empty search: the matching parts, best first, without groups. */
  readonly results: readonly ComponentCatalogEntry[] | null;
  /** Every shown part in the order the keyboard walks them. */
  readonly items: readonly ComponentCatalogEntry[];
}

/**
 * What the drawer shows. `placeable` limits the list to parts the editor can draw; `catalog` is
 * already limited by an assignment. Favorites and recent parts that are not in the catalog
 * shown (for example parts an assignment does not allow) are kept in the preferences but not
 * listed.
 */
export function buildDrawerView(
  catalog: ComponentCatalog | null,
  preferences: DrawerPreferences,
  query: string,
  placeable: (typeId: ComponentTypeId) => boolean,
): DrawerView {
  const entries = (catalog?.componentTypes ?? []).filter((entry) =>
    placeable(entry.typeId),
  );

  if (query.trim() !== "") {
    const results = searchParts(entries, query);
    return { groups: [], results, items: results };
  }

  const byType = new Map(entries.map((entry) => [entry.typeId, entry]));
  const listed = (ids: readonly ComponentTypeId[]) =>
    ids.flatMap((id) => {
      const entry = byType.get(id);
      return entry ? [entry] : [];
    });

  const groups: DrawerGroup[] = [];
  const favorites = listed(preferences.favorites);
  const recent = listed(preferences.recent);

  if (favorites.length > 0) {
    groups.push({ id: "favorites", entries: favorites });
  }
  if (recent.length > 0) {
    groups.push({ id: "recent", entries: recent });
  }

  for (const category of catalog?.categories ?? []) {
    const inCategory = entries.filter((entry) => entry.category === category);
    if (inCategory.length > 0) {
      groups.push({ id: category, entries: inCategory });
    }
  }

  return {
    groups,
    results: null,
    items: groups.flatMap((group) => group.entries),
  };
}

export type HighlightKey = "ArrowDown" | "ArrowUp" | "Home" | "End";

/**
 * The highlighted index after a navigation key in the search field: Down and Up move one item
 * and stop at the ends, Home and End go to the first and last. -1 means no highlight.
 */
export function moveHighlight(
  index: number,
  count: number,
  key: HighlightKey,
): number {
  if (count === 0) {
    return -1;
  }

  switch (key) {
    case "ArrowDown":
      return Math.min(count - 1, index + 1);
    case "ArrowUp":
      return index < 0 ? 0 : Math.max(0, index - 1);
    case "Home":
      return 0;
    case "End":
      return count - 1;
  }
}

export type SearchKeyAction =
  | { readonly kind: "none" }
  | { readonly kind: "highlight"; readonly index: number }
  | {
      readonly kind: "choose";
      readonly index: number;
      readonly placeAtCenter: boolean;
    }
  | { readonly kind: "clear" }
  | { readonly kind: "close" };

/**
 * What a key does in the search field (docs/contracts/stage-11-drawer-and-editing.md section 3.4).
 * Enter chooses the highlighted part, or the best match when the user typed a query and has not
 * walked the list; Shift+Enter places it. Escape clears a query, and in an empty search closes
 * the drawer.
 */
export function searchKeyAction(input: {
  readonly key: string;
  readonly shiftKey: boolean;
  readonly highlight: number;
  readonly count: number;
  readonly query: string;
}): SearchKeyAction {
  const { key, shiftKey, highlight, count, query } = input;

  switch (key) {
    case "ArrowDown":
    case "ArrowUp":
    case "Home":
    case "End":
      // Home and End keep their text-editing meaning until the user walks the list.
      if ((key === "Home" || key === "End") && highlight < 0 && query !== "") {
        return { kind: "none" };
      }
      return count === 0
        ? { kind: "none" }
        : { kind: "highlight", index: moveHighlight(highlight, count, key) };
    case "Enter": {
      const index = highlight >= 0 ? highlight : query !== "" ? 0 : -1;
      return index >= 0 && index < count
        ? { kind: "choose", index, placeAtCenter: shiftKey }
        : { kind: "none" };
    }
    case "Escape":
      return query !== "" ? { kind: "clear" } : { kind: "close" };
    default:
      return { kind: "none" };
  }
}
