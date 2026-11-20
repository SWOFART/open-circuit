import { describe, expect, it } from "vitest";
import type { ComponentSpec } from "../generated/ComponentSpec";
import type { Schematic } from "../generated/Schematic";
import { symbols } from "../symbols";
import { bindCanvasInput, type CanvasInputSurface } from "./canvasInput";
import { analyzeGeometry } from "./connectivity";
import {
  CanvasInteraction,
  DOUBLE_PRESS_MS,
  DOUBLE_PRESS_PX,
  type CanvasPointer,
  type EditorTool,
  type InteractionIntent,
} from "./interaction";
import { operatingTargets } from "./operate";
import { buildScene } from "./scene";
import { toScreen } from "./view";
import { emptyDocumentation } from "./documentation";

const RESISTOR = "00000000-0000-4000-8000-0000000000a1";
const SWITCH = "00000000-0000-4000-8000-0000000000a2";
const BUTTON = "00000000-0000-4000-8000-0000000000a3";
const RAIL = "00000000-0000-4000-8000-0000000000a4";
const OTHER = "00000000-0000-4000-8000-0000000000a5";

function part(
  typeId: ComponentSpec["typeId"],
  id: string,
  x: number,
  parameters: Record<string, number>,
): ComponentSpec {
  return {
    typeId,
    id,
    position: { x, y: 0 },
    rotationDeg: 0,
    parameters,
  } as unknown as ComponentSpec;
}

const schematic: Schematic = {
  schemaVersion: 3,
  components: [
    part("resistor", RESISTOR, 0, { resistanceOhms: 1000 }),
    part("switch_spst", SWITCH, 10, { closedResistanceOhms: 0.01 }),
    part("push_button_no", BUTTON, 20, { closedResistanceOhms: 0.01 }),
    part("resistor", OTHER, 30, { resistanceOhms: 1000 }),
  ],
  wires: [],
  junctions: [],
  referenceMarkers: [],
  rails: [
    { id: RAIL, name: "L1", start: { x: 40, y: -5 }, end: { x: 40, y: 5 } },
  ],
  documentation: emptyDocumentation(),
};

function setUp(operable = false) {
  const intents: InteractionIntent[] = [];
  const interaction = new CanvasInteraction(
    () => {},
    (intent) => intents.push(intent),
  );
  interaction.setViewport({ width: 1600, height: 600 });
  interaction.setScene(
    buildScene(schematic, symbols),
    analyzeGeometry(schematic, symbols),
  );
  interaction.setOperable(operable ? operatingTargets(schematic, symbols) : []);

  const at = (x: number, y = 0) => toScreen({ x, y }, interaction.view);
  const press = (
    point: { x: number; y: number },
    time: number,
    extra: Partial<CanvasPointer> = {},
  ) =>
    interaction.pointerDown({
      ...point,
      button: 0,
      pointerId: 1,
      time,
      ...extra,
    });
  const release = (point: { x: number; y: number }, time: number) =>
    interaction.pointerUp({ ...point, button: 0, pointerId: 1, time });
  const opened = () => intents.filter((intent) => intent.kind === "openEditor");

  return { interaction, intents, at, press, release, opened };
}

describe("the double press", () => {
  it("opens the editor of a component on the second press, once", () => {
    const { at, press, release, opened } = setUp();
    const point = at(0);

    press(point, 1000);
    release(point, 1050);
    expect(opened()).toEqual([]);

    press(point, 1200);

    expect(opened()).toEqual([
      { kind: "openEditor", target: { kind: "component", id: RESISTOR } },
    ]);
    // A third quick press starts over; it does not open another editor.
    release(point, 1230);
    press(point, 1300);
    expect(opened()).toHaveLength(1);
  });

  it("does not open it when the second press is too late or too far", () => {
    const { at, press, release, opened } = setUp();
    const point = at(0);

    press(point, 1000);
    release(point, 1020);
    press(point, 1000 + DOUBLE_PRESS_MS + 1);
    release(point, 1500);
    expect(opened()).toEqual([]);

    press({ x: point.x, y: point.y }, 3000);
    release(point, 3010);
    press({ x: point.x + DOUBLE_PRESS_PX + 1, y: point.y }, 3100);
    expect(opened()).toEqual([]);
  });

  it("opens it at the very limits", () => {
    const { at, press, release, opened } = setUp();
    const point = at(0);

    press(point, 1000);
    release(point, 1010);
    press({ x: point.x + DOUBLE_PRESS_PX, y: point.y }, 1000 + DOUBLE_PRESS_MS);

    expect(opened()).toHaveLength(1);
  });

  it("needs both presses on the same component", () => {
    const { at, press, release, opened } = setUp();

    press(at(0), 1000);
    release(at(0), 1010);
    press(at(30), 1100);

    expect(opened()).toEqual([]);
  });

  it("is not a double press when the first press dragged the part", () => {
    const { interaction, at, press, release, opened } = setUp();
    const start = at(0);

    press(start, 1000);
    interaction.pointerMove({
      ...at(2),
      button: 0,
      pointerId: 1,
      time: 1010,
    });
    release(at(2), 1020);
    press(at(2), 1100);

    expect(opened()).toEqual([]);
  });

  it("never opens without event times", () => {
    const { interaction, at, opened } = setUp();
    const point = at(0);

    interaction.pointerDown({ ...point, button: 0, pointerId: 1 });
    interaction.pointerUp({ ...point, button: 0, pointerId: 1 });
    interaction.pointerDown({ ...point, button: 0, pointerId: 1 });

    expect(opened()).toEqual([]);
  });

  it("needs the same pointer for both presses", () => {
    const { at, press, release, opened } = setUp();
    const point = at(0);

    press(point, 1000);
    release(point, 1010);
    press(point, 1100, { pointerId: 2 });

    expect(opened()).toEqual([]);
  });

  it("does nothing new on empty canvas", () => {
    const { at, press, release, opened } = setUp();
    const empty = at(15, 8);

    press(empty, 1000);
    release(empty, 1010);
    press(empty, 1100);

    expect(opened()).toEqual([]);
  });

  it("selects a rail and asks for its properties", () => {
    const { interaction, at, press, release, opened } = setUp();
    const onRail = at(40, 2);

    press(onRail, 1000);
    release(onRail, 1010);
    press(onRail, 1100);

    expect(interaction.selection).toEqual({ kind: "rail", id: RAIL });
    expect(opened()).toEqual([
      { kind: "openEditor", target: { kind: "rail", id: RAIL } },
    ]);
  });
});

describe("the double press with another tool", () => {
  it.each([
    [{ kind: "wire" }],
    [{ kind: "junction" }],
    [{ kind: "reference" }],
    [{ kind: "rail" }],
    [{ kind: "place", typeId: "resistor", rotationDeg: 0 }],
  ] as readonly (readonly [EditorTool])[])(
    "behaves as two presses and opens nothing with the %o tool",
    (tool) => {
      const { interaction, at, press, release, opened } = setUp();
      const point = at(0);

      interaction.setTool(tool);
      press(point, 1000);
      release(point, 1010);
      press(point, 1100);

      expect(opened()).toEqual([]);
    },
  );

  it("places two parts with the Place tool", () => {
    const { interaction, intents, at, press, release } = setUp();
    const point = at(15, 8);

    interaction.setTool({ kind: "place", typeId: "resistor", rotationDeg: 0 });
    press(point, 1000);
    release(point, 1010);
    press(point, 1100);

    expect(intents.filter((intent) => intent.kind === "place")).toHaveLength(2);
  });

  it("forgets a press made before a tool change", () => {
    const { interaction, at, press, release, opened } = setUp();
    const point = at(0);

    press(point, 1000);
    release(point, 1010);
    interaction.setTool({ kind: "wire" });
    interaction.setTool({ kind: "select" });
    press(point, 1100);

    expect(opened()).toEqual([]);
  });
});

describe("the double press on an operating area", () => {
  const leverOf = (kind: "switch" | "pushButton") => {
    const target = operatingTargets(schematic, symbols).find(
      (candidate) => candidate.kind === kind,
    );
    return target!;
  };

  it("toggles a switch once and does not open the editor", () => {
    const { interaction, intents, press, release, opened } = setUp(true);
    const lever = toScreen(leverOf("switch").center, interaction.view);

    press(lever, 1000);
    release(lever, 1010);
    press(lever, 1100);
    release(lever, 1110);

    expect(intents.filter((intent) => intent.kind === "operate")).toEqual([
      { kind: "operate", componentId: SWITCH, action: "toggle" },
    ]);
    expect(opened()).toEqual([]);
  });

  it("presses a push button once and releases it by the first release", () => {
    const { interaction, intents, press, release, opened } = setUp(true);
    const actuator = toScreen(leverOf("pushButton").center, interaction.view);

    press(actuator, 1000);
    release(actuator, 1010);
    press(actuator, 1100);
    release(actuator, 1110);

    expect(
      intents
        .filter((intent) => intent.kind === "operate")
        .map((intent) => intent.action),
    ).toEqual(["press", "release"]);
    expect(opened()).toEqual([]);
  });

  it("opens the editor from the rest of the symbol", () => {
    const { interaction, press, release, opened } = setUp(true);
    const body = toScreen({ x: 10.8, y: 0 }, interaction.view);

    press(body, 1000);
    release(body, 1010);
    press(body, 1100);

    expect(opened()).toEqual([
      { kind: "openEditor", target: { kind: "component", id: SWITCH } },
    ]);
  });

  it("opens the editor on a double press when no run is on screen", () => {
    const { interaction, press, release, opened } = setUp(false);
    const point = toScreen(leverOf("switch").center, interaction.view);

    press(point, 1000);
    release(point, 1010);
    press(point, 1100);

    expect(opened()).toEqual([
      { kind: "openEditor", target: { kind: "component", id: SWITCH } },
    ]);
  });
});

describe("Enter on the canvas", () => {
  class Surface extends EventTarget implements CanvasInputSurface {
    readonly style = { cursor: "" };
    focused = 0;
    getBoundingClientRect() {
      return { left: 0, top: 0 };
    }
    setPointerCapture() {}
    releasePointerCapture() {}
    hasPointerCapture() {
      return false;
    }
    focus() {
      this.focused += 1;
    }
  }

  function enter(target: object) {
    const event = new Event("keydown", { cancelable: true });
    Object.assign(event, { key: "Enter", code: "Enter" });
    Object.defineProperty(event, "target", { value: target });
    return event;
  }

  function bound() {
    const surface = new Surface();
    const keyboard = new EventTarget();
    const intents: InteractionIntent[] = [];
    const interaction = new CanvasInteraction(
      () => {},
      (intent) => intents.push(intent),
    );
    interaction.setViewport({ width: 1600, height: 600 });
    interaction.setScene(
      buildScene(schematic, symbols),
      analyzeGeometry(schematic, symbols),
    );
    bindCanvasInput(surface, interaction, keyboard);
    return { surface, keyboard, interaction, intents };
  }

  it("opens the editor of the selected component when the canvas is the target", () => {
    const { surface, keyboard, interaction, intents } = bound();
    interaction.select({ kind: "component", id: RESISTOR });

    const event = enter(surface);
    keyboard.dispatchEvent(event);

    expect(intents).toEqual([
      { kind: "openEditor", target: { kind: "component", id: RESISTOR } },
    ]);
    expect(event.defaultPrevented).toBe(true);
  });

  it("leaves Enter alone for any other target, such as a focused button", () => {
    const { keyboard, interaction, intents } = bound();
    interaction.select({ kind: "component", id: RESISTOR });

    const event = enter({ tagName: "BUTTON", type: "button" });
    keyboard.dispatchEvent(event);

    expect(intents).toEqual([]);
    expect(event.defaultPrevented).toBe(false);
  });

  it("does nothing when a wire, a rail or nothing is selected", () => {
    const { surface, keyboard, interaction, intents } = bound();

    keyboard.dispatchEvent(enter(surface));
    interaction.select({ kind: "rail", id: RAIL });
    keyboard.dispatchEvent(enter(surface));

    expect(intents).toEqual([]);
  });

  it("finishes a route first when one is in progress", () => {
    const { surface, keyboard, interaction, intents } = bound();
    interaction.select({ kind: "component", id: RESISTOR });
    interaction.setTool({ kind: "wire" });
    interaction.pointerDown({ x: 100, y: 100, button: 0, pointerId: 1 });

    keyboard.dispatchEvent(enter(surface));

    expect(intents.some((intent) => intent.kind === "openEditor")).toBe(false);
  });

  it("gives the canvas the focus on a press, for the keyboard that follows", () => {
    const { surface } = bound();
    const down = new Event("pointerdown", { cancelable: true });
    Object.assign(down, { clientX: 5, clientY: 5, button: 0, pointerId: 1 });

    surface.dispatchEvent(down);

    expect(surface.focused).toBe(1);
  });
});
