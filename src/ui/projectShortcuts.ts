import { matchShortcut } from "../editor/shortcuts";

export type ProjectShortcut = "new" | "open" | "save" | "saveAs";

function isEditableTarget(target: EventTarget | null): boolean {
  if (
    !target ||
    typeof target !== "object" ||
    !("closest" in target) ||
    typeof target.closest !== "function"
  ) {
    return false;
  }

  return Boolean(
    target.closest(
      'input, textarea, select, [contenteditable="true"], [role="textbox"]',
    ),
  );
}

/** File shortcuts never take over text-entry fields or unrelated modifiers. */
export function projectShortcutFromEvent(
  event: Pick<
    KeyboardEvent,
    "key" | "ctrlKey" | "metaKey" | "shiftKey" | "altKey" | "target"
  >,
  isMacPlatform: boolean,
): ProjectShortcut | null {
  if (isEditableTarget(event.target)) return null;

  // The keys are the table's (src/editor/shortcuts.ts); only this machine's own Ctrl or Cmd counts.
  switch (matchShortcut(event, "anywhere", isMacPlatform ? "mac" : "other")) {
    case "file.new":
      return "new";
    case "file.open":
      return "open";
    case "file.save":
      return "save";
    case "file.saveAs":
      return "saveAs";
    default:
      return null;
  }
}
