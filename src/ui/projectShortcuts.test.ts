import { describe, expect, it } from "vitest";
import { projectShortcutFromEvent } from "./projectShortcuts";

function keyEvent(
  key: string,
  options: Partial<KeyboardEvent> = {},
  target: EventTarget | null = null,
): KeyboardEvent {
  return {
    key,
    ctrlKey: false,
    metaKey: false,
    shiftKey: false,
    altKey: false,
    target,
    ...options,
  } as KeyboardEvent;
}

describe("project shortcuts", () => {
  it("maps Ctrl and Command file shortcuts", () => {
    expect(
      projectShortcutFromEvent(keyEvent("n", { ctrlKey: true }), false),
    ).toBe("new");
    expect(
      projectShortcutFromEvent(keyEvent("o", { metaKey: true }), true),
    ).toBe("open");
    expect(
      projectShortcutFromEvent(keyEvent("s", { ctrlKey: true }), false),
    ).toBe("save");
    expect(
      projectShortcutFromEvent(
        keyEvent("s", { metaKey: true, shiftKey: true }),
        true,
      ),
    ).toBe("saveAs");
  });

  it("leaves text entry and unrelated modifiers alone", () => {
    const input = { closest: () => ({}) } as unknown as EventTarget;

    expect(
      projectShortcutFromEvent(keyEvent("s", { ctrlKey: true }, input), false),
    ).toBeNull();
    expect(
      projectShortcutFromEvent(
        keyEvent("s", { ctrlKey: true, altKey: true }),
        false,
      ),
    ).toBeNull();
    expect(
      projectShortcutFromEvent(keyEvent("o", { ctrlKey: true }), true),
    ).toBeNull();
  });
});
