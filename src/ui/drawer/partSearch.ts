import type { CategoryId } from "../../generated/CategoryId";
import type { ComponentCatalogEntry } from "../../generated/ComponentCatalogEntry";
import { message } from "../messages";

/** The drawer's search, as docs/contracts/stage-11-drawer-and-editing.md section 3.3 defines it. */

/** NFKC, lower case, no combining marks, and `ё` read as `е`. */
export function normalizeSearchText(text: string): string {
  return text
    .normalize("NFKD")
    .replace(/\p{M}/gu, "")
    .normalize("NFKC")
    .toLowerCase()
    .replace(/ё/g, "е");
}

export function searchTokens(query: string): string[] {
  return normalizeSearchText(query)
    .split(/\s+/u)
    .filter((token) => token.length > 0);
}

interface Searchable {
  readonly name: string;
  readonly nameWords: readonly string[];
  readonly keywords: string;
  readonly keywordWords: readonly string[];
  readonly typeId: string;
  readonly category: string;
}

function words(text: string): string[] {
  return text.split(/[^\p{L}\p{N}]+/u).filter((word) => word.length > 0);
}

function searchable(entry: ComponentCatalogEntry): Searchable {
  const name = normalizeSearchText(message(entry.displayNameKey));
  const keywords = normalizeSearchText(
    entry.searchKeywordKeys.map((key) => message(key)).join(" "),
  );

  return {
    name,
    nameWords: words(name),
    keywords,
    keywordWords: words(keywords),
    typeId: normalizeSearchText(entry.typeId),
    category: normalizeSearchText(message(categoryKey(entry.category))),
  };
}

export function categoryKey(category: CategoryId): string {
  return `category.${category}`;
}

/** 1 is the best rank; null when the token matches nothing. */
function tokenRank(
  token: string,
  query: string,
  item: Searchable,
): number | null {
  if (item.name === query) return 1;
  if (item.name.startsWith(query)) return 2;
  if (item.nameWords.some((word) => word.startsWith(token))) return 3;
  if (item.keywordWords.some((word) => word.startsWith(token))) return 4;
  if (
    item.name.includes(token) ||
    item.keywords.includes(token) ||
    item.typeId.includes(token) ||
    item.category.includes(token)
  ) {
    return 5;
  }
  return null;
}

/**
 * The entries that match `query`, best first. Every token must match somewhere; the rank of an
 * entry is the best rank of any token, and ties keep the catalog order. An empty query matches
 * every entry in the catalog order.
 */
export function searchParts(
  entries: readonly ComponentCatalogEntry[],
  query: string,
): ComponentCatalogEntry[] {
  const tokens = searchTokens(query);

  if (tokens.length === 0) {
    return [...entries];
  }

  const whole = normalizeSearchText(query).trim().replace(/\s+/gu, " ");
  const ranked: {
    entry: ComponentCatalogEntry;
    rank: number;
    index: number;
  }[] = [];

  entries.forEach((entry, index) => {
    const item = searchable(entry);
    let best = Number.POSITIVE_INFINITY;

    for (const token of tokens) {
      const rank = tokenRank(token, whole, item);
      if (rank === null) {
        return;
      }
      best = Math.min(best, rank);
    }

    ranked.push({ entry, rank: best, index });
  });

  return ranked
    .sort((a, b) => a.rank - b.rank || a.index - b.index)
    .map((item) => item.entry);
}
