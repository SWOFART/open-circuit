import { describe, expect, it } from "vitest";
import schematicFixture from "../../tests/golden/stage-1-divider.schematic.json";
import type { Schematic } from "../generated/Schematic";
import { symbols } from "../symbols";
import {
  bindCanvasInput,
  isTextEntryTarget,
  type CanvasInputSurface,
} from "./canvasInput";
import { CanvasInteraction, type InteractionIntent } from "./interaction";
import { buildScene } from "./scene";
import { toGrid, toScreen } from "./view";

const R1 = "00000000-0000-4000-8000-000000000002";
// The canvas sits 30 px from the left and 50 px from the top of the window.
const RECT = { left: 30, top: 50 };

class FakeCanvas extends EventTarget implements CanvasInputSurface {
  readonly style = { cursor: "" };
  readonly captured = new Set<number>();

  getBoundingClientRect() {
    return RECT;
  }

  setPointerCapture(pointerId: number): void {
    this.captured.add(pointerId);
  }

  releasePointerCapture(pointerId: number): void {
    this.captured.delete(pointerId);
  }

  hasPointerCapture(pointerId: number): boolean {
    return this.captured.has(pointerId);
  }
}

function domEvent(
  type: string,
  properties: Record<string, unknown>,
  target?: object,
): Event {
  const event = new Event(type, { cancelable: true });
  Object.assign(event, properties);
  if (target) {
    // A window listener sees the focused element as the target.
    Object.defineProperty(event, "target", { value: target });
  }
  return event;
}

function pointer(
  type: string,
  clientX: number,
  clientY: number,
  button = 0,
  pointerId = 1,
): Event {
  return domEvent(type, { clientX, clientY, button, pointerId });
}

function setup() {
  const canvas = new FakeCanvas();
  const keyboard = new EventTarget();
  const interaction = new CanvasInteraction();
  interaction.setViewport({ width: 800, height: 600 });
  interaction.setScene(buildScene(schematicFixture as Schematic, symbols));
  const binding = bindCanvasInput(canvas, interaction, keyboard);

  // Client coordinates of a grid point under the current view.
  const client = (grid: { x: number; y: number }) => {
    const screen = toScreen(grid, interaction.view);
    return { x: screen.x + RECT.left, y: screen.y + RECT.top };
  };

  return { canvas, keyboard, interaction, binding, client };
}

describe("canvas pointer input", () => {
  it("converts client coordinates to canvas pixels before hit testing", () => {
    const { canvas, interaction, client } = setup();
    const onR1 = client({ x: 4, y: 0 });

    canvas.dispatchEvent(pointer("pointerdown", onR1.x, onR1.y));

    expect(interaction.selection).toEqual({ kind: "component", id: R1 });
  });

  it("pans with a captured middle-button drag", () => {
    const { canvas, interaction } = setup();
    const start = interaction.view;
    const down = pointer("pointerdown", 200, 200, 1, 3);

    canvas.dispatchEvent(down);
    expect(down.defaultPrevented).toBe(true);
    expect(canvas.captured.has(3)).toBe(true);
    expect(canvas.style.cursor).toBe("grabbing");

    canvas.dispatchEvent(pointer("pointermove", 260, 170, 1, 3));
    canvas.dispatchEvent(pointer("pointerup", 260, 170, 1, 3));

    expect(interaction.view.offsetX).toBe(start.offsetX + 60);
    expect(interaction.view.offsetY).toBe(start.offsetY - 30);
    expect(canvas.captured.has(3)).toBe(false);
    expect(canvas.style.cursor).toBe("");
  });

  it("still pans when the pointer cannot be captured", () => {
    const { canvas, interaction } = setup();
    canvas.setPointerCapture = () => {
      throw new Error("No active pointer with the given id is found.");
    };
    const start = interaction.view;

    canvas.dispatchEvent(pointer("pointerdown", 200, 200, 1, 3));
    canvas.dispatchEvent(pointer("pointermove", 210, 200, 1, 3));

    expect(canvas.style.cursor).toBe("grabbing");
    expect(interaction.view.offsetX).toBe(start.offsetX + 10);
  });

  it("blocks middle-button autoscroll on the canvas", () => {
    const { canvas } = setup();
    const middle = domEvent("mousedown", { button: 1 });
    const left = domEvent("mousedown", { button: 0 });

    canvas.dispatchEvent(middle);
    canvas.dispatchEvent(left);

    expect(middle.defaultPrevented).toBe(true);
    expect(left.defaultPrevented).toBe(false);
  });

  it("ends a pan when pointer capture is lost", () => {
    const { canvas, interaction } = setup();
    canvas.dispatchEvent(pointer("pointerdown", 200, 200, 1, 4));

    canvas.dispatchEvent(pointer("lostpointercapture", 200, 200, 1, 4));

    expect(interaction.isPanning).toBe(false);
    expect(canvas.style.cursor).toBe("");
  });

  it("forgets the pointer when it leaves the canvas", () => {
    const { canvas, interaction } = setup();
    canvas.dispatchEvent(pointer("pointermove", 300, 300));
    expect(interaction.pointerGridPoint).not.toBeNull();

    canvas.dispatchEvent(domEvent("pointerleave", {}));

    expect(interaction.pointerGridPoint).toBeNull();
  });
});

describe("canvas wheel input", () => {
  it("zooms around the pointer and keeps the page from scrolling", () => {
    const { canvas, interaction } = setup();
    const canvasPoint = { x: 410, y: 260 };
    const anchor = toGrid(canvasPoint, interaction.view);
    const startScale = interaction.view.scale;
    const wheel = domEvent("wheel", {
      clientX: canvasPoint.x + RECT.left,
      clientY: canvasPoint.y + RECT.top,
      deltaY: -120,
      deltaMode: 0,
    });

    canvas.dispatchEvent(wheel);

    expect(wheel.defaultPrevented).toBe(true);
    expect(interaction.view.scale).toBeGreaterThan(startScale);
    const after = toGrid(canvasPoint, interaction.view);
    expect(after.x).toBeCloseTo(anchor.x, 9);
    expect(after.y).toBeCloseTo(anchor.y, 9);
  });
});

describe("Space pan modifier", () => {
  it("turns a left-button drag into a pan while Space is held", () => {
    const { canvas, keyboard, interaction, client } = setup();
    const onR1 = client({ x: 4, y: 0 });
    const start = interaction.view;
    canvas.dispatchEvent(pointer("pointermove", onR1.x, onR1.y));

    const keydown = domEvent("keydown", { code: "Space" });
    keyboard.dispatchEvent(keydown);
    expect(keydown.defaultPrevented).toBe(true);
    expect(canvas.style.cursor).toBe("grab");

    canvas.dispatchEvent(pointer("pointerdown", onR1.x, onR1.y));
    canvas.dispatchEvent(pointer("pointermove", onR1.x + 25, onR1.y + 5));
    canvas.dispatchEvent(pointer("pointerup", onR1.x + 25, onR1.y + 5));
    keyboard.dispatchEvent(domEvent("keyup", { code: "Space" }));

    expect(interaction.selection).toBeNull();
    expect(interaction.view.offsetX).toBe(start.offsetX + 25);
    expect(interaction.view.offsetY).toBe(start.offsetY + 5);
    expect(interaction.isPanModifierHeld).toBe(false);
    expect(canvas.style.cursor).toBe("");
  });

  it("leaves Space alone while typing in a text field", () => {
    const { canvas, keyboard, interaction } = setup();
    canvas.dispatchEvent(pointer("pointermove", 300, 300));
    const input = { tagName: "INPUT", type: "text", isContentEditable: false };
    const keydown = domEvent("keydown", { code: "Space" }, input);

    keyboard.dispatchEvent(keydown);

    expect(keydown.defaultPrevented).toBe(false);
    expect(interaction.isPanModifierHeld).toBe(false);
  });

  it("does not swallow Space when the pointer is outside the canvas", () => {
    const { keyboard, interaction } = setup();
    const keydown = domEvent("keydown", { code: "Space" });

    keyboard.dispatchEvent(keydown);

    expect(keydown.defaultPrevented).toBe(false);
    expect(interaction.isPanModifierHeld).toBe(true);
  });

  it("releases the modifier when the window loses focus", () => {
    const { canvas, keyboard, interaction } = setup();
    keyboard.dispatchEvent(domEvent("keydown", { code: "Space" }));

    keyboard.dispatchEvent(domEvent("blur", {}));

    expect(interaction.isPanModifierHeld).toBe(false);
    expect(canvas.style.cursor).toBe("");
  });

  it("ignores other keys", () => {
    const { keyboard, interaction } = setup();

    keyboard.dispatchEvent(domEvent("keydown", { code: "KeyA" }));

    expect(interaction.isPanModifierHeld).toBe(false);
  });
});

describe("unbinding", () => {
  it("removes every listener", () => {
    const { canvas, keyboard, interaction, binding } = setup();
    const start = interaction.view;
    binding.dispose();

    canvas.dispatchEvent(
      domEvent("wheel", { clientX: 100, clientY: 100, deltaY: -100 }),
    );
    canvas.dispatchEvent(pointer("pointerdown", 200, 200, 1));
    keyboard.dispatchEvent(domEvent("keydown", { code: "Space" }));

    expect(interaction.view).toEqual(start);
    expect(interaction.isPanning).toBe(false);
    expect(interaction.isPanModifierHeld).toBe(false);
  });
});

describe("text-entry detection", () => {
  it.each([
    [{ tagName: "INPUT", type: "text" }, true],
    [{ tagName: "INPUT", type: "number" }, true],
    [{ tagName: "TEXTAREA" }, true],
    [{ tagName: "SELECT" }, true],
    [{ tagName: "DIV", isContentEditable: true }, true],
    [{ tagName: "INPUT", type: "checkbox" }, false],
    [{ tagName: "BUTTON" }, false],
    [{ tagName: "CANVAS" }, false],
  ])("classifies %o", (target, expected) => {
    expect(isTextEntryTarget(target as unknown as EventTarget)).toBe(expected);
  });

  it("treats a non-element target as not typing", () => {
    expect(isTextEntryTarget(null)).toBe(false);
    expect(isTextEntryTarget(new EventTarget())).toBe(false);
  });
});

describe("editing shortcuts", () => {
  function withIntents() {
    const canvas = new FakeCanvas();
    const keyboard = new EventTarget();
    const intents: InteractionIntent[] = [];
    const interaction = new CanvasInteraction(undefined, (intent) =>
      intents.push(intent),
    );
    interaction.setViewport({ width: 800, height: 600 });
    interaction.setScene(buildScene(schematicFixture as Schematic, symbols));
    bindCanvasInput(canvas, interaction, keyboard);
    return { canvas, keyboard, interaction, intents };
  }

  function key(
    code: string,
    keyName: string,
    modifiers: Partial<
      Record<"ctrlKey" | "metaKey" | "shiftKey" | "altKey", boolean>
    > = {},
    target?: object,
  ): Event {
    return domEvent(
      "keydown",
      {
        code,
        key: keyName,
        ctrlKey: false,
        metaKey: false,
        shiftKey: false,
        altKey: false,
        ...modifiers,
      },
      target,
    );
  }

  it.each([
    ["Ctrl+Z", key("KeyZ", "z", { ctrlKey: true }), { kind: "undo" }],
    ["Command+Z", key("KeyZ", "z", { metaKey: true }), { kind: "undo" }],
    [
      "Ctrl+Shift+Z",
      key("KeyZ", "Z", { ctrlKey: true, shiftKey: true }),
      { kind: "redo" },
    ],
    [
      "Command+Shift+Z",
      key("KeyZ", "Z", { metaKey: true, shiftKey: true }),
      { kind: "redo" },
    ],
    // A Russian layout reports a Cyrillic key for the same physical key.
    [
      "Ctrl+Z on a Cyrillic layout",
      key("KeyZ", "я", { ctrlKey: true }),
      { kind: "undo" },
    ],
  ])("maps %s", (_, event, intent) => {
    const { keyboard, intents } = withIntents();

    keyboard.dispatchEvent(event);

    expect(intents).toEqual([intent]);
    expect(event.defaultPrevented).toBe(true);
  });

  it.each([
    ["Delete", key("Delete", "Delete")],
    ["Backspace", key("Backspace", "Backspace")],
  ])("deletes the selected component with %s", (_, event) => {
    const { keyboard, interaction, intents } = withIntents();
    interaction.select({ kind: "component", id: R1 });

    keyboard.dispatchEvent(event);

    expect(intents).toEqual([
      { kind: "delete", target: { kind: "component", id: R1 } },
    ]);
    expect(event.defaultPrevented).toBe(true);
  });

  it("rotates the selected component with R", () => {
    const { keyboard, interaction, intents } = withIntents();
    interaction.select({ kind: "component", id: R1 });

    keyboard.dispatchEvent(key("KeyR", "r"));

    expect(intents).toEqual([{ kind: "rotate", componentId: R1 }]);
  });

  it("rotates once while R is held down", () => {
    const { keyboard, interaction, intents } = withIntents();
    interaction.select({ kind: "component", id: R1 });
    const held = Object.assign(key("KeyR", "r"), { repeat: true });

    keyboard.dispatchEvent(key("KeyR", "r"));
    keyboard.dispatchEvent(held);

    expect(intents).toEqual([{ kind: "rotate", componentId: R1 }]);
    // The repeat is still consumed, so it does not type or scroll anything.
    expect(held.defaultPrevented).toBe(true);
  });

  it("returns a placement tool to Select mode with Escape", () => {
    const { canvas, keyboard, interaction } = withIntents();
    interaction.setTool({ kind: "place", typeId: "resistor", rotationDeg: 0 });

    keyboard.dispatchEvent(key("Escape", "Escape"));

    expect(interaction.tool).toEqual({ kind: "select" });
    expect(canvas.style.cursor).toBe("");
  });

  it("never acts while a text field has focus", () => {
    const { keyboard, interaction, intents } = withIntents();
    interaction.select({ kind: "component", id: R1 });
    const input = { tagName: "INPUT", type: "text", isContentEditable: false };
    const events = [
      key("KeyZ", "z", { ctrlKey: true }, input),
      key("Backspace", "Backspace", {}, input),
      key("KeyR", "r", {}, input),
    ];

    for (const event of events) {
      keyboard.dispatchEvent(event);
    }

    expect(intents).toEqual([]);
    expect(events.some((event) => event.defaultPrevented)).toBe(false);
  });

  it("ignores unrelated keys and modified letters", () => {
    const { keyboard, interaction, intents } = withIntents();
    interaction.select({ kind: "component", id: R1 });
    const events = [
      key("KeyA", "a"),
      key("KeyR", "r", { ctrlKey: true }),
      key("KeyZ", "z"),
      key("KeyZ", "z", { ctrlKey: true, altKey: true }),
    ];

    for (const event of events) {
      keyboard.dispatchEvent(event);
    }

    expect(intents).toEqual([]);
    expect(events.some((event) => event.defaultPrevented)).toBe(false);
  });

  it("shows a crosshair while placing and a move cursor while dragging", () => {
    const { canvas, interaction } = withIntents();
    const binding = bindCanvasInput(canvas, interaction, new EventTarget());

    interaction.setTool({ kind: "place", typeId: "resistor", rotationDeg: 0 });
    binding.refreshCursor();
    expect(canvas.style.cursor).toBe("crosshair");

    interaction.setTool({ kind: "select" });
    const start = toScreen({ x: 4, y: 0 }, interaction.view);
    const end = toScreen({ x: 6, y: 0 }, interaction.view);
    canvas.dispatchEvent(
      pointer("pointerdown", start.x + RECT.left, start.y + RECT.top),
    );
    canvas.dispatchEvent(
      pointer("pointermove", end.x + RECT.left, end.y + RECT.top),
    );
    expect(canvas.style.cursor).toBe("move");

    canvas.dispatchEvent(
      pointer("pointerup", end.x + RECT.left, end.y + RECT.top),
    );
    expect(canvas.style.cursor).toBe("");
  });
});
