import { describe, expect, it } from "vitest";
import type { ComponentSpec } from "../generated/ComponentSpec";
import type { ComponentTypeId } from "../generated/ComponentTypeId";
import type { GridPoint } from "../generated/GridPoint";
import type { PinId } from "../generated/PinId";
import type { Schematic } from "../generated/Schematic";
import { symbols } from "../symbols";
import { placementPreviewSchematic } from "./commands";
import { resolvePinPositions, type Rotation } from "./geometry";
import { hitTest } from "./hitTest";
import { buildScene } from "./scene";
import { followingWireEdits } from "./transform";

const RELAY = "00000000-0000-4000-8000-0000000000d1";
const FUSE = "00000000-0000-4000-8000-0000000000d2";
const MOTOR = "00000000-0000-4000-8000-0000000000d3";
const VOLTMETER = "00000000-0000-4000-8000-0000000000d4";
const AMMETER = "00000000-0000-4000-8000-0000000000d5";
const WIRE = "00000000-0000-4000-8000-0000000000d6";

const STAGE_4: readonly ComponentTypeId[] = [
  "relay_spdt",
  "fuse",
  "pmdc_motor",
  "voltmeter",
  "ammeter",
];

/** The relay's normally open pin is at (10, -1) when the relay sits at (7, 0). */
function schematic(): Schematic {
  return {
    schemaVersion: 1,
    components: [
      {
        typeId: "relay_spdt",
        id: RELAY,
        position: { x: 7, y: 0 },
        rotationDeg: 0,
        parameters: {
          coilResistanceOhms: 100,
          coilInductanceHenries: 0.01,
          pickupCurrentAmps: 0.04,
          dropoutCurrentAmps: 0.02,
          closedContactResistanceOhms: 0.1,
        },
      },
      {
        typeId: "fuse",
        id: FUSE,
        position: { x: 20, y: 0 },
        rotationDeg: 0,
        parameters: {
          intactResistanceOhms: 0.01,
          tripI2tAmpereSquaredSeconds: 0.004,
        },
      },
      {
        typeId: "pmdc_motor",
        id: MOTOR,
        position: { x: 26, y: 0 },
        rotationDeg: 0,
        parameters: {
          armatureResistanceOhms: 2,
          armatureInductanceHenries: 0.1,
          motorConstantNewtonMetersPerAmp: 0.1,
          rotorInertiaKilogramMetersSquared: 0.01,
          viscousFrictionNewtonMeterSecondsPerRadian: 0.01,
          loadTorqueNewtonMeters: 0,
        },
      },
      {
        typeId: "voltmeter",
        id: VOLTMETER,
        position: { x: 32, y: 0 },
        rotationDeg: 0,
        parameters: { inputResistanceOhms: 1e7 },
      },
      {
        typeId: "ammeter",
        id: AMMETER,
        position: { x: 38, y: 0 },
        rotationDeg: 0,
        parameters: { burdenResistanceOhms: 0.1 },
      },
    ],
    wires: [
      {
        id: WIRE,
        points: [
          { x: 10, y: -1 },
          { x: 15, y: -1 },
        ],
      },
    ],
    junctions: [],
    referenceMarkers: [],
  };
}

function component(typeId: ComponentTypeId, rotationDeg: Rotation = 0) {
  return {
    ...schematic().components.find((item) => item.typeId === typeId),
    rotationDeg,
  } as ComponentSpec;
}

describe("drawing the Stage 4 devices", () => {
  const scene = buildScene(schematic(), symbols);

  it("draws each device from its own symbol, owned by the device", () => {
    for (const [typeId, id] of [
      ["relay_spdt", RELAY],
      ["fuse", FUSE],
      ["pmdc_motor", MOTOR],
      ["voltmeter", VOLTMETER],
      ["ammeter", AMMETER],
    ] as const) {
      const own = scene.shapes.filter(
        (shape) =>
          shape.owner.kind === "component" &&
          shape.owner.id === id &&
          shape.kind !== "label",
      );

      expect(own).toHaveLength(symbols[typeId]!.primitives.length);
    }
  });

  it("selects a device by clicking on its symbol", () => {
    // The left edge of the relay's coil box, the letter M, the meters' letters, and the
    // fuse's body.
    expect(hitTest(scene, { x: 5, y: 0 }, 0.1)).toEqual({
      kind: "component",
      id: RELAY,
    });
    expect(hitTest(scene, { x: 25.7, y: 0 }, 0.1)).toEqual({
      kind: "component",
      id: MOTOR,
    });
    expect(hitTest(scene, { x: 32, y: 0.28 }, 0.1)).toEqual({
      kind: "component",
      id: VOLTMETER,
    });
    expect(hitTest(scene, { x: 38, y: -0.28 }, 0.1)).toEqual({
      kind: "component",
      id: AMMETER,
    });
    expect(hitTest(scene, { x: 20, y: 0.25 }, 0.1)).toEqual({
      kind: "component",
      id: FUSE,
    });
    expect(hitTest(scene, { x: 12, y: -1 }, 0.1)).toEqual({
      kind: "wire",
      id: WIRE,
    });
  });

  it("previews each device before it is placed, turned or not", () => {
    for (const typeId of STAGE_4) {
      for (const rotationDeg of [0, 90] as const) {
        const preview = buildScene(
          placementPreviewSchematic(typeId, { x: 2, y: 2 }, rotationDeg),
          symbols,
        );

        expect(preview.shapes.length).toBeGreaterThanOrEqual(
          symbols[typeId]!.primitives.length,
        );
      }
    }
  });

  it("draws the relay's five pins at distinct places", () => {
    const pins = resolvePinPositions(component("relay_spdt"), symbols);

    expect(pins.map((pin) => pin.pinId)).toEqual([
      "coil_positive",
      "coil_negative",
      "common",
      "normally_open",
      "normally_closed",
    ]);
    expect(pins.find((pin) => pin.pinId === "normally_open")?.position).toEqual(
      {
        x: 10,
        y: -1,
      },
    );
  });
});

describe("moving and rotating the Stage 4 devices", () => {
  it.each([90, 180, 270] as const)(
    "carries the wire end that sat on the relay's normally open pin when it turns %i degrees",
    (rotationDeg) => {
      const turned = followingWireEdits(
        schematic(),
        RELAY,
        { position: { x: 7, y: 0 }, rotationDeg },
        symbols,
      );

      expect(turned).toHaveLength(1);
      const [edit] = turned;
      const pin = resolvePinPositions(
        { ...component("relay_spdt"), position: { x: 7, y: 0 }, rotationDeg },
        symbols,
      ).find((candidate) => candidate.pinId === ("normally_open" as PinId))!;
      const ends = [edit!.points[0], edit!.points[edit!.points.length - 1]];

      // The wire end follows the pin to its new place.
      expect(ends).toContainEqual(pin.position);
      expect(ends).not.toContainEqual({ x: 10, y: -1 });
    },
  );

  it.each([0, 90, 180, 270] as const)(
    "keeps every pin of every device on the grid, in catalog order, at %i degrees",
    (rotationDeg: Rotation) => {
      for (const typeId of STAGE_4) {
        const pins = resolvePinPositions(
          component(typeId, rotationDeg),
          symbols,
        );

        expect(pins.map((pin) => pin.pinId)).toEqual(
          symbols[typeId]?.pins.map((pin) => pin.id),
        );
        for (const pin of pins) {
          expect(Number.isInteger(pin.position.x)).toBe(true);
          expect(Number.isInteger(pin.position.y)).toBe(true);
        }
        expect(
          new Set(
            pins.map(
              (pin: { position: GridPoint }) =>
                `${pin.position.x},${pin.position.y}`,
            ),
          ).size,
        ).toBe(pins.length);
      }
    },
  );
});
