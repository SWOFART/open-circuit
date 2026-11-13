import { describe, expect, it } from "vitest";
import type { ComponentSpec } from "../generated/ComponentSpec";
import type { ComponentTypeId } from "../generated/ComponentTypeId";
import type { PinId } from "../generated/PinId";
import type { Schematic } from "../generated/Schematic";
import { symbols } from "../symbols";
import { placementPreviewSchematic } from "./commands";
import { resolvePinPositions, type Rotation } from "./geometry";
import { hitTest } from "./hitTest";
import { buildScene } from "./scene";
import { followingWireEdits } from "./transform";

const DIODE = "00000000-0000-4000-8000-0000000000e1";
const NPN = "00000000-0000-4000-8000-0000000000e2";
const PNP = "00000000-0000-4000-8000-0000000000e3";
const WIRE = "00000000-0000-4000-8000-0000000000e4";

const STAGE_7: readonly ComponentTypeId[] = ["diode", "bjt_npn", "bjt_pnp"];

const junction = { saturationCurrentAmps: 1e-14 };
const transistor = {
  ...junction,
  forwardBeta: 100,
  reverseBeta: 1,
};

function schematic(): Schematic {
  return {
    schemaVersion: 2,
    components: [
      {
        typeId: "diode",
        id: DIODE,
        position: { x: 10, y: 10 },
        rotationDeg: 0,
        parameters: { ...junction, emissionCoefficient: 1 },
      },
      {
        typeId: "bjt_npn",
        id: NPN,
        position: { x: 30, y: 10 },
        rotationDeg: 0,
        parameters: transistor,
      },
      {
        typeId: "bjt_pnp",
        id: PNP,
        position: { x: 50, y: 10 },
        rotationDeg: 0,
        parameters: transistor,
      },
    ],
    // Ends on the NPN's base pin at (28, 10).
    wires: [
      {
        id: WIRE,
        points: [
          { x: 20, y: 10 },
          { x: 28, y: 10 },
        ],
      },
    ],
    junctions: [],
    referenceMarkers: [],
    rails: [],
  };
}

function component(typeId: ComponentTypeId, rotationDeg: Rotation = 0) {
  return {
    ...schematic().components.find((item) => item.typeId === typeId),
    rotationDeg,
  } as ComponentSpec;
}

function pinAt(typeId: ComponentTypeId, pinId: string, rotationDeg: Rotation) {
  return resolvePinPositions(
    { ...component(typeId, rotationDeg), position: { x: 0, y: 0 } },
    symbols,
  ).find((pin) => pin.pinId === pinId)!.position;
}

describe("the Stage 7 symbols", () => {
  it("put the pins where the contract says, in catalog order", () => {
    expect(
      resolvePinPositions(component("diode"), symbols).map((pin) => [
        pin.pinId,
        pin.position,
      ]),
    ).toEqual([
      ["anode", { x: 9, y: 10 }],
      ["cathode", { x: 11, y: 10 }],
    ]);

    for (const typeId of ["bjt_npn", "bjt_pnp"] as const) {
      expect(
        resolvePinPositions(
          { ...component(typeId), position: { x: 0, y: 0 } },
          symbols,
        ).map((pin) => [pin.pinId, pin.position]),
      ).toEqual([
        ["collector", { x: 1, y: -2 }],
        ["base", { x: -2, y: 0 }],
        ["emitter", { x: 1, y: 2 }],
      ]);
    }
  });

  it("turn the pins a quarter turn clockwise on screen at a time", () => {
    // Screen y points down: (1, 0) turned 90 degrees clockwise is (0, 1).
    expect(pinAt("diode", "cathode", 90)).toEqual({ x: 0, y: 1 });
    expect(pinAt("diode", "cathode", 180)).toEqual({ x: -1, y: 0 });
    expect(pinAt("diode", "cathode", 270)).toEqual({ x: 0, y: -1 });
    expect(pinAt("bjt_npn", "base", 90)).toEqual({ x: 0, y: -2 });
    expect(pinAt("bjt_npn", "base", 180)).toEqual({ x: 2, y: 0 });
    expect(pinAt("bjt_npn", "collector", 90)).toEqual({ x: 2, y: 1 });
  });

  it.each([0, 90, 180, 270] as const)(
    "keep every pin on the grid and apart at %i degrees",
    (rotationDeg: Rotation) => {
      for (const typeId of STAGE_7) {
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
          new Set(pins.map((pin) => `${pin.position.x},${pin.position.y}`))
            .size,
        ).toBe(pins.length);
      }
    },
  );

  it("differ between NPN and PNP only in the arrow on the emitter", () => {
    const npn = symbols.bjt_npn!;
    const pnp = symbols.bjt_pnp!;

    expect(npn.pins).toEqual(pnp.pins);
    const different = npn.primitives.filter(
      (primitive, index) =>
        JSON.stringify(primitive) !== JSON.stringify(pnp.primitives[index]),
    );

    expect(npn.primitives).toHaveLength(pnp.primitives.length);
    expect(different).toHaveLength(1);

    // The NPN arrow ends on the emitter side (below the bar's middle) and points away from the
    // base; the PNP arrow's tip is the point nearest the base.
    const tipOf = (primitive: (typeof different)[number]) =>
      primitive.kind === "polyline" ? primitive.points[1]! : null;
    const npnTip = tipOf(different[0]!);
    const pnpTip = tipOf(
      pnp.primitives.find(
        (primitive, index) =>
          JSON.stringify(primitive) !== JSON.stringify(npn.primitives[index]),
      )!,
    );

    expect(npnTip!.x).toBeGreaterThan(pnpTip!.x);
  });

  it("are drawn from their own primitives, owned by the component", () => {
    const scene = buildScene(schematic(), symbols);

    for (const [typeId, id] of [
      ["diode", DIODE],
      ["bjt_npn", NPN],
      ["bjt_pnp", PNP],
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

  it("are selected by clicking on their drawing", () => {
    const scene = buildScene(schematic(), symbols);

    // The diode's cathode bar, the NPN's and the PNP's base bars.
    expect(hitTest(scene, { x: 10.4, y: 10.4 }, 0.1)).toEqual({
      kind: "component",
      id: DIODE,
    });
    expect(hitTest(scene, { x: 29.5, y: 10.8 }, 0.1)).toEqual({
      kind: "component",
      id: NPN,
    });
    expect(hitTest(scene, { x: 49.5, y: 9.2 }, 0.1)).toEqual({
      kind: "component",
      id: PNP,
    });
    // A wire where nothing else is.
    expect(hitTest(scene, { x: 24, y: 10 }, 0.1)).toEqual({
      kind: "wire",
      id: WIRE,
    });
    // Empty grid between the symbols.
    expect(hitTest(scene, { x: 40, y: 20 }, 0.1)).toBeNull();
  });

  it("are previewed before they are placed, turned or not", () => {
    for (const typeId of STAGE_7) {
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
});

describe("moving and rotating the Stage 7 symbols", () => {
  it.each([90, 180, 270] as const)(
    "carries the wire end on the NPN's base when it turns %i degrees",
    (rotationDeg) => {
      const turned = followingWireEdits(
        schematic(),
        NPN,
        { position: { x: 30, y: 10 }, rotationDeg },
        symbols,
      );

      expect(turned).toHaveLength(1);
      const base = resolvePinPositions(
        { ...component("bjt_npn"), position: { x: 30, y: 10 }, rotationDeg },
        symbols,
      ).find((pin) => pin.pinId === ("base" as PinId))!;
      const ends = [
        turned[0]!.points[0],
        turned[0]!.points[turned[0]!.points.length - 1],
      ];

      expect(ends).toContainEqual(base.position);
      expect(ends).not.toContainEqual({ x: 28, y: 10 });
    },
  );

  it("carries the wire end when the NPN moves", () => {
    const moved = followingWireEdits(
      schematic(),
      NPN,
      { position: { x: 34, y: 14 }, rotationDeg: 0 },
      symbols,
    );

    expect(moved).toHaveLength(1);
    const ends = [moved[0]!.points[0], moved[0]!.points.at(-1)];

    expect(ends).toContainEqual({ x: 32, y: 14 });
  });
});
