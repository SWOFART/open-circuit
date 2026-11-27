import type { HistoryLabel } from "../editor/historyLabel";

// The text of a history entry (docs/contracts/stage-16-editor-improvements.md section 6). A
// label without a known key, or no label at all, reads as the generic "Edit".

const GENERIC = "Edit";

function objects(count: unknown): string {
  return count === 1 ? "1 object" : `${String(count)} objects`;
}

const TEXTS: Readonly<
  Record<string, (values: NonNullable<HistoryLabel["values"]>) => string>
> = {
  "history.place": (values) => `Place ${String(values.name ?? "a part")}`,
  "history.transform": () => "Move or turn a part",
  "history.delete": () => "Delete an object",
  "history.drawWire": () => "Draw a wire",
  "history.editWire": () => "Edit a wire",
  "history.addJunction": () => "Add a junction",
  "history.addMarker": () => "Add a ground marker",
  "history.addNote": () => "Add a note",
  "history.addTerminal": () => "Add a terminal",
  "history.moveObject": () => "Move an object",
  "history.addRail": () => "Add a rail",
  "history.editRail": () => "Edit a rail",
  "history.move": (values) => `Move ${objects(values.count)}`,
  "history.deleteSelection": (values) => `Delete ${objects(values.count)}`,
  "history.paste": (values) => `Paste ${objects(values.count)}`,
  "history.turn": (values) => `Turn ${objects(values.count)}`,
  "history.mirror": (values) => `Mirror ${objects(values.count)}`,
  "history.setParameter": (values) =>
    `Set ${String(values.parameter)} of ${String(values.part)}`,
  "history.clearParameter": (values) =>
    `Clear ${String(values.parameter)} of ${String(values.part)}`,
  "history.setDesignator": (values) => `Name a part ${String(values.part)}`,
};

/** The line the history panel shows for an entry. */
export function historyLabelText(label: HistoryLabel | null): string {
  const text = label ? TEXTS[label.key] : undefined;
  return text && label ? text(label.values ?? {}) : GENERIC;
}
