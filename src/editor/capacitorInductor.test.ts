import { describe, expect, it } from "vitest";
import type { ComponentSpec } from "../generated/ComponentSpec";
import type { Schematic } from "../generated/Schematic";
import { symbols } from "../symbols";
import { placementPreviewSchematic } from "./commands";
import { resolvePinPositions, type Rotation } from "./geometry";
import { hitTest } from "./hitTest";
import { buildScene } from "./scene";
import { followingWireEdits } from "./transform";

const CAPACITOR = "00000000-0000-4000-8000-0000000000c1";
const INDUCTOR = "00000000-0000-4000-8000-0000000000c2";
const WIRE = "00000000-0000-4000-8000-0000000000c3";

function schematic(): Schematic {
  return {
    schemaVersion: 1,
    components: [
      {
        typeId: "capacitor",
        id: CAPACITOR,
        position: { x: 4, y: 0 },
        rotationDeg: 0,
        parameters: { capacitanceFarads: 1e-5 },
      },
      {
        typeId: "inductor",
        id: INDUCTOR,
        position: { x: 10, y: 0 },
        rotationDeg: 0,
        parameters: { inductanceHenries: 0.1 },
      },
    ],
    wires: [
      {
        id: WIRE,
        points: [
          { x: 5, y: 0 },
          { x: 9, y: 0 },
        ],
      },
    ],
    junctions: [],
    referenceMarkers: [],
  };
}

describe("drawing capacitors and inductors", () => {
  const scene = buildScene(schematic(), symbols);

  it("draws each part from its own symbol, owned by the part", () => {
    for (const id of [CAPACITOR, INDUCTOR]) {
      const own = scene.shapes.filter(
        (shape) => shape.owner.kind === "component" && shape.owner.id === id,
      );

      expect(own.length).toBeGreaterThan(3);
    }
    // The coil is drawn as humps, not as the resistor's box.
    const coil = scene.shapes.filter(
      (shape) => shape.owner.id === INDUCTOR && shape.kind === "polyline",
    );
    expect(coil.length).toBe(symbols.inductor?.primitives.length);
  });

  it("selects a part by clicking inside its symbol", () => {
    expect(hitTest(scene, { x: 4, y: 0.2 }, 0.1)).toEqual({
      kind: "component",
      id: CAPACITOR,
    });
    expect(hitTest(scene, { x: 10, y: -0.1 }, 0.1)).toEqual({
      kind: "component",
      id: INDUCTOR,
    });
    expect(hitTest(scene, { x: 7, y: 0 }, 0.1)).toEqual({
      kind: "wire",
      id: WIRE,
    });
  });

  it("previews each type before it is placed", () => {
    for (const typeId of ["capacitor", "inductor"] as const) {
      const preview = buildScene(
        placementPreviewSchematic(typeId, { x: 2, y: 2 }, 90),
        symbols,
      );

      expect(preview.shapes.length).toBeGreaterThan(3);
    }
  });
});

describe("moving and rotating capacitors and inductors", () => {
  it.each([
    ["capacitor", CAPACITOR, 5, 0],
    ["inductor", INDUCTOR, 9, 0],
  ] as const)(
    "carries the wire end that sat on the %s's pin when it turns",
    (_, id, pinX, pinY) => {
      const turned = followingWireEdits(
        schematic(),
        id,
        {
          position: id === CAPACITOR ? { x: 4, y: 0 } : { x: 10, y: 0 },
          rotationDeg: 90,
        },
        symbols,
      );

      expect(turned).toHaveLength(1);
      const [edit] = turned;
      const ends = [edit?.points[0], edit?.points[edit.points.length - 1]];
      // The old pin position is no longer an end of the wire.
      expect(ends).not.toContainEqual({ x: pinX, y: pinY });
    },
  );

  it.each([0, 90, 180, 270] as const)(
    "keeps both pins on the grid at %i degrees",
    (rotationDeg: Rotation) => {
      for (const typeId of ["capacitor", "inductor"] as const) {
        const part = {
          ...schematic().components.find((item) => item.typeId === typeId),
          rotationDeg,
        } as ComponentSpec;
        const [pin1, pin2] = resolvePinPositions(part, symbols);

        expect(pin1?.pinId).toBe("pin1");
        expect(pin2?.pinId).toBe("pin2");
        expect(
          Math.abs((pin1?.position.x ?? 0) - (pin2?.position.x ?? 0)) +
            Math.abs((pin1?.position.y ?? 0) - (pin2?.position.y ?? 0)),
        ).toBe(2);
      }
    },
  );
});
