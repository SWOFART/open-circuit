import { describe, expect, it } from "vitest";
import type { ComponentSpec } from "../generated/ComponentSpec";
import type { DeviceState } from "../generated/DeviceState";
import type { Schematic } from "../generated/Schematic";
import type { StableId } from "../generated/StableId";
import { symbols } from "../symbols";
import { analyzeGeometry } from "./connectivity";
import {
  CanvasInteraction,
  type InteractionChange,
  type InteractionIntent,
} from "./interaction";
import { operatingTargetAt, operatingTargets } from "./operate";
import { buildScene } from "./scene";
import { toScreen } from "./view";

const SWITCH = "00000000-0000-4000-8000-0000000000e1";
const BUTTON = "00000000-0000-4000-8000-0000000000e2";
const CONTACTOR = "00000000-0000-4000-8000-0000000000e3";
const BUTTON_NC = "00000000-0000-4000-8000-0000000000e4";

function component(
  typeId: ComponentSpec["typeId"],
  id: string,
  x: number,
  rotationDeg: 0 | 90 | 180 | 270 = 0,
): ComponentSpec {
  const parameters: Record<string, number> =
    typeId === "contactor"
      ? {
          coilResistanceOhms: 100,
          coilInductanceHenries: 0.01,
          pickupCurrentAmps: 0.04,
          dropoutCurrentAmps: 0.02,
          closedContactResistanceOhms: 0.1,
        }
      : { closedResistanceOhms: 0.01 };

  return {
    typeId,
    id,
    position: { x, y: 0 },
    rotationDeg,
    parameters,
  } as unknown as ComponentSpec;
}

const schematic: Schematic = {
  schemaVersion: 1,
  components: [
    component("switch_spst", SWITCH, 0),
    component("push_button_no", BUTTON, 10),
    component("contactor", CONTACTOR, 20),
    component("push_button_nc", BUTTON_NC, 40),
  ],
  wires: [],
  junctions: [],
  referenceMarkers: [],
};

function setUp(operable = true) {
  const intents: InteractionIntent[] = [];
  const changes: InteractionChange[] = [];
  const interaction = new CanvasInteraction(
    (change) => changes.push(change),
    (intent) => intents.push(intent),
  );
  interaction.setViewport({ width: 1200, height: 600 });
  const scene = buildScene(schematic, symbols);
  interaction.setScene(scene, analyzeGeometry(schematic, symbols));
  // A view the tests know: fitting the circuit would move the origin.
  interaction.setOperable(operable ? operatingTargets(schematic, symbols) : []);

  return { interaction, intents, changes, scene };
}

function operations(intents: readonly InteractionIntent[]) {
  return intents
    .filter((intent) => intent.kind === "operate")
    .map((intent) => [intent.componentId, intent.action]);
}

describe("the operating areas", () => {
  it("exist for switches and push buttons and not for the contactor", () => {
    const targets = operatingTargets(schematic, symbols);

    expect(targets.map((target) => [target.componentId, target.kind])).toEqual([
      [SWITCH, "switch"],
      [BUTTON, "pushButton"],
      [BUTTON_NC, "pushButton"],
    ]);
  });

  it("follow the part when it is moved and rotated", () => {
    const rotated: Schematic = {
      ...schematic,
      components: [component("switch_spst", SWITCH, 5, 90)],
    };
    const [target] = operatingTargets(rotated, symbols);

    // The lever area sits above the switch (local y = -0.4); a quarter turn puts it at
    // the right of the origin.
    expect(target?.center.x).toBeCloseTo(5.4);
    expect(target?.center.y).toBeCloseTo(0);
    expect(
      operatingTargetAt(target ? [target] : [], { x: 5.4, y: 0 }, 0),
    ).not.toBeNull();
    expect(
      operatingTargetAt(target ? [target] : [], { x: 5, y: 1.2 }, 0),
    ).toBeNull();
  });
});

describe("drawing the states the Engine reported", () => {
  const polylinesOf = (
    states: Map<StableId, DeviceState> | undefined,
    owner: string,
  ) =>
    buildScene(schematic, symbols, { states })
      .shapes.filter(
        (shape) => shape.kind === "polyline" && shape.owner.id === owner,
      )
      .map((shape) => JSON.stringify(shape));

  it("draws every part at rest when there is no state", () => {
    const rest = buildScene(schematic, symbols);
    const empty = buildScene(schematic, symbols, { states: new Map() });

    expect(empty.shapes).toEqual(rest.shapes);
  });

  it("draws the switch lever closed or open as the Engine said", () => {
    const closed = new Map<StableId, DeviceState>([
      [
        SWITCH,
        {
          kind: "switch",
          componentId: SWITCH,
          position: "closed",
          conducting: true,
        },
      ],
    ]);
    const open = new Map<StableId, DeviceState>([
      [
        SWITCH,
        {
          kind: "switch",
          componentId: SWITCH,
          position: "open",
          conducting: false,
        },
      ],
    ]);

    expect(polylinesOf(closed, SWITCH)).not.toEqual(polylinesOf(open, SWITCH));
    expect(polylinesOf(open, SWITCH)).toEqual(polylinesOf(undefined, SWITCH));
  });

  it("draws a push button from its conduction and pressed position", () => {
    const state = (
      position: "released" | "pressed",
      conducting: boolean,
    ): Map<StableId, DeviceState> =>
      new Map([
        [
          BUTTON_NC,
          { kind: "pushButton", componentId: BUTTON_NC, position, conducting },
        ],
      ]);

    const rest = polylinesOf(undefined, BUTTON_NC);
    expect(polylinesOf(state("released", true), BUTTON_NC)).toEqual(rest);
    expect(polylinesOf(state("pressed", false), BUTTON_NC)).not.toEqual(rest);
  });

  it("draws a picked contactor with its normally open contact closed", () => {
    const picked = new Map<StableId, DeviceState>([
      [
        CONTACTOR,
        { kind: "contactor", componentId: CONTACTOR, state: "picked" },
      ],
    ]);

    expect(polylinesOf(picked, CONTACTOR)).not.toEqual(
      polylinesOf(undefined, CONTACTOR),
    );
    expect(
      polylinesOf(
        new Map([
          [
            CONTACTOR,
            {
              kind: "contactor",
              componentId: CONTACTOR,
              state: "released",
            } as DeviceState,
          ],
        ]),
        CONTACTOR,
      ),
    ).toEqual(polylinesOf(undefined, CONTACTOR));
  });
});

describe("operating a device on the canvas", () => {
  it("toggles a switch on a click of its lever and selects it", () => {
    const { interaction, intents } = setUp();
    interaction.setViewport({ width: 1200, height: 600 });
    const view = interaction.view;
    const lever = operatingTargets(schematic, symbols)[0]!;
    const screen = toScreen(lever.center, view);

    const capture = interaction.pointerDown({
      x: screen.x,
      y: screen.y,
      button: 0,
      pointerId: 1,
    });
    interaction.pointerUp({
      x: screen.x,
      y: screen.y,
      button: 0,
      pointerId: 1,
    });

    expect(capture).toBe(false);
    expect(operations(intents)).toEqual([[SWITCH, "toggle"]]);
    expect(interaction.selection).toEqual({ kind: "component", id: SWITCH });
    expect(interaction.isDragging).toBe(false);
  });

  it("presses a push button on pointer down and releases it on pointer up", () => {
    const { interaction, intents } = setUp();
    const actuator = operatingTargets(schematic, symbols)[1]!;
    const screen = toScreen(actuator.center, interaction.view);
    const at = { x: screen.x, y: screen.y, button: 0, pointerId: 7 };

    expect(interaction.pointerDown(at)).toBe(true);
    expect(operations(intents)).toEqual([[BUTTON, "press"]]);

    interaction.pointerUp(at);
    expect(operations(intents)).toEqual([
      [BUTTON, "press"],
      [BUTTON, "release"],
    ]);
  });

  it.each([
    [
      "the pointer leaves the actuator",
      (interaction: CanvasInteraction) =>
        interaction.pointerMove({
          x: 5,
          y: 5,
          button: 0,
          pointerId: 7,
        }),
    ],
    [
      "the pointer is canceled",
      (interaction: CanvasInteraction) => interaction.pointerCancel(7),
    ],
    [
      "the window loses focus",
      (interaction: CanvasInteraction) => interaction.releaseOperated(),
    ],
    [
      "Escape is pressed",
      (interaction: CanvasInteraction) => interaction.escape(),
    ],
    [
      "the pointer leaves the canvas",
      (interaction: CanvasInteraction) => interaction.pointerLeave(),
    ],
  ])("releases a held push button when %s", (_name, end) => {
    const { interaction, intents } = setUp();
    const actuator = operatingTargets(schematic, symbols)[1]!;
    const screen = toScreen(actuator.center, interaction.view);

    interaction.pointerDown({
      x: screen.x,
      y: screen.y,
      button: 0,
      pointerId: 7,
    });
    end(interaction);
    // A second end of the same press releases nothing more.
    interaction.pointerUp({
      x: screen.x,
      y: screen.y,
      button: 0,
      pointerId: 7,
    });
    interaction.releaseOperated();

    expect(operations(intents)).toEqual([
      [BUTTON, "press"],
      [BUTTON, "release"],
    ]);
  });

  it("releases a held button when the run goes away", () => {
    const { interaction, intents } = setUp();
    const actuator = operatingTargets(schematic, symbols)[1]!;
    const screen = toScreen(actuator.center, interaction.view);

    interaction.pointerDown({
      x: screen.x,
      y: screen.y,
      button: 0,
      pointerId: 7,
    });
    interaction.setOperable([]);

    expect(operations(intents)).toEqual([
      [BUTTON, "press"],
      [BUTTON, "release"],
    ]);
  });

  it("does not operate anything without a run, and then selects and drags as before", () => {
    const { interaction, intents } = setUp(false);
    const lever = operatingTargets(schematic, symbols)[0]!;
    const screen = toScreen(lever.center, interaction.view);

    const capture = interaction.pointerDown({
      x: screen.x,
      y: screen.y,
      button: 0,
      pointerId: 1,
    });

    expect(operations(intents)).toEqual([]);
    expect(capture).toBe(true);
    expect(interaction.selection).toEqual({ kind: "component", id: SWITCH });
  });

  it("selects and moves a part pressed on its body, not on its lever", () => {
    const { interaction, intents } = setUp();
    const view = interaction.view;
    // The contactor has no operating area at all; its coil box is a plain part.
    const body = toScreen({ x: 20 - 2, y: 0 }, view);

    const capture = interaction.pointerDown({
      x: body.x,
      y: body.y,
      button: 0,
      pointerId: 1,
    });

    expect(operations(intents)).toEqual([]);
    expect(capture).toBe(true);
    expect(interaction.selection).toEqual({
      kind: "component",
      id: CONTACTOR,
    });
  });
});
