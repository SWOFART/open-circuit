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
  if (event.altKey || isEditableTarget(event.target)) return null;
  if (isMacPlatform ? !event.metaKey : !event.ctrlKey) return null;

  const key = event.key.toLowerCase();
  if (key === "n" && !event.shiftKey) return "new";
  if (key === "o" && !event.shiftKey) return "open";
  if (key === "s") return event.shiftKey ? "saveAs" : "save";
  return null;
}
