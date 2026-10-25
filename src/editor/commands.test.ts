import { describe, expect, it } from "vitest";
import schematicFixture from "../../tests/golden/stage-1-divider.schematic.json";
import type { Schematic } from "../generated/Schematic";
import { symbols } from "../symbols";
import {
  commandForIntent,
  dragPreviewSchematic,
  placementPreviewSchematic,
} from "./commands";
import { followingWireEdits } from "./transform";

const schematic = schematicFixture as Schematic;
const R1 = "00000000-0000-4000-8000-000000000002";
const MIDDLE_WIRE = "00000000-0000-4000-8000-000000000005";
const GROUND = "00000000-0000-4000-8000-000000000007";
const MISSING = "00000000-0000-4000-8000-0000000000ff";

describe("turning canvas intents into document commands", () => {
  it("places the chosen type at the snapped point with its rotation", () => {
    expect(
      commandForIntent(
        {
          kind: "place",
          typeId: "resistor",
          position: { x: 2, y: -3 },
          rotationDeg: 90,
        },
        schematic,
        symbols,
      ),
    ).toEqual({
      kind: "placeComponent",
      typeId: "resistor",
      position: { x: 2, y: -3 },
      rotationDeg: 90,
    });
  });

  it("moves a component by the drag offset and brings its wires along", () => {
    const command = commandForIntent(
      { kind: "move", componentId: R1, delta: { x: 0, y: 2 } },
      schematic,
      symbols,
    );

    expect(command).toEqual({
      kind: "transformComponent",
      componentId: R1,
      position: { x: 4, y: 2 },
      rotationDeg: 0,
      wires: followingWireEdits(
        schematic,
        R1,
        { position: { x: 4, y: 2 }, rotationDeg: 0 },
        symbols,
      ),
    });
  });

  it("rotates a component clockwise in place", () => {
    const command = commandForIntent(
      { kind: "rotate", componentId: R1 },
      schematic,
      symbols,
    );

    expect(command).toMatchObject({
      kind: "transformComponent",
      componentId: R1,
      position: { x: 4, y: 0 },
      rotationDeg: 90,
    });
    expect(
      command?.kind === "transformComponent" && command.wires,
    ).toHaveLength(2);
  });

  it("deletes a selected component", () => {
    expect(
      commandForIntent(
        { kind: "delete", target: { kind: "component", id: R1 } },
        schematic,
        symbols,
      ),
    ).toEqual({ kind: "deleteComponent", componentId: R1 });
  });

  it("leaves other selected objects to their own tools", () => {
    for (const target of [
      { kind: "wire", id: MIDDLE_WIRE },
      { kind: "referenceMarker", id: GROUND },
    ] as const) {
      expect(
        commandForIntent({ kind: "delete", target }, schematic, symbols),
      ).toBeNull();
    }
  });

  it("ignores intents for a component that no longer exists", () => {
    expect(
      commandForIntent(
        { kind: "move", componentId: MISSING, delta: { x: 1, y: 0 } },
        schematic,
        symbols,
      ),
    ).toBeNull();
    expect(
      commandForIntent(
        { kind: "rotate", componentId: MISSING },
        schematic,
        symbols,
      ),
    ).toBeNull();
    expect(
      commandForIntent(
        { kind: "delete", target: { kind: "component", id: MISSING } },
        schematic,
        symbols,
      ),
    ).toBeNull();
  });

  it("passes undo and redo through", () => {
    expect(commandForIntent({ kind: "undo" }, schematic, symbols)).toEqual({
      kind: "undo",
    });
    expect(commandForIntent({ kind: "redo" }, schematic, symbols)).toEqual({
      kind: "redo",
    });
  });
});

describe("previews", () => {
  it("shows a dragged component and its rubber-band wires before release", () => {
    const preview = dragPreviewSchematic(
      schematic,
      { componentId: R1, delta: { x: 0, y: 2 } },
      symbols,
    );

    expect(preview.components.find((item) => item.id === R1)?.position).toEqual(
      { x: 4, y: 2 },
    );
    expect(
      preview.wires.find((wire) => wire.id === MIDDLE_WIRE)?.points,
    ).toEqual([
      { x: 5, y: 2 },
      { x: 7, y: 2 },
      { x: 7, y: 0 },
    ]);
    // Nothing is committed by drawing a preview.
    expect(
      schematic.components.find((item) => item.id === R1)?.position,
    ).toEqual({ x: 4, y: 0 });
  });

  it("draws a placement preview as a lone component", () => {
    const preview = placementPreviewSchematic(
      "dc_voltage_source",
      { x: -2, y: 5 },
      270,
    );

    expect(preview.components).toHaveLength(1);
    expect(preview.components[0]).toMatchObject({
      typeId: "dc_voltage_source",
      position: { x: -2, y: 5 },
      rotationDeg: 270,
    });
    expect(preview.wires).toEqual([]);
  });
});
