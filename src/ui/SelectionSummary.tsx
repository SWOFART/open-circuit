import type { SelectionTarget } from "../editor/hitTest";

const NOUNS: Readonly<
  Record<SelectionTarget["kind"], readonly [string, string]>
> = {
  component: ["part", "parts"],
  wire: ["wire", "wires"],
  junction: ["junction", "junctions"],
  referenceMarker: ["ground marker", "ground markers"],
  rail: ["rail", "rails"],
  note: ["note", "notes"],
  terminal: ["terminal", "terminals"],
};

/**
 * What a selection of several objects says about itself: "3 parts, 2 wires selected". Nothing
 * for none or one (the inspector and the device panels show a single object themselves).
 */
export function selectionSummaryText(
  selections: readonly SelectionTarget[],
): string {
  if (selections.length < 2) {
    return "";
  }

  const counts = new Map<SelectionTarget["kind"], number>();
  for (const target of selections) {
    counts.set(target.kind, (counts.get(target.kind) ?? 0) + 1);
  }

  const parts = (Object.keys(NOUNS) as SelectionTarget["kind"][])
    .filter((kind) => counts.has(kind))
    .map((kind) => {
      const count = counts.get(kind)!;
      return `${count} ${NOUNS[kind][count === 1 ? 0 : 1]}`;
    });

  return `${parts.join(", ")} selected`;
}

/**
 * The count of a selection of several, in a polite live region so a screen reader announces
 * it when it changes. The region stays in the page when it is empty.
 */
export function SelectionSummary({
  selections,
}: {
  readonly selections: readonly SelectionTarget[];
}) {
  return (
    <p className="selection-summary" role="status" aria-live="polite">
      {selectionSummaryText(selections)}
    </p>
  );
}
