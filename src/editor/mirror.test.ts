import { describe, expect, it } from "vitest";
import type { ComponentSpec } from "../generated/ComponentSpec";
import { symbols } from "../symbols";
import {
  buildGeometryEnvelope,
  resolvePinPositions,
  rotateLocal,
} from "./geometry";
import type { Rotation } from "./geometry";
import { buildScene } from "./scene";
import { emptyDocumentation } from "./documentation";

// The mirror of docs/contracts/stage-16-editor-improvements.md section 5.2: a mirrored part is
// flipped about the vertical axis of its symbol, (x, y) to (-x, y), and then rotated. Every
// symbol of the registry passes the same geometry tests, so a drawn part and the geometry sent
// to the Engine always agree.

const ROTATIONS: readonly Rotation[] = [0, 90, 180, 270];
const TYPE_IDS = Object.keys(symbols) as ComponentSpec["typeId"][];
const ORIGIN = { x: 10, y: 20 };

function part(
  typeId: ComponentSpec["typeId"],
  rotationDeg: Rotation,
  mirrored: boolean,
): ComponentSpec {
  return {
    typeId,
    id: "00000000-0000-4000-8000-0000000000aa",
    position: ORIGIN,
    rotationDeg,
    ...(mirrored ? { mirrored: true } : {}),
    parameters: {},
  } as unknown as ComponentSpec;
}

describe("the pins of a mirrored part", () => {
  it("covers every symbol of the registry", () => {
    expect(TYPE_IDS.length).toBeGreaterThan(40);
  });

  it.each(TYPE_IDS)(
    "%s: a mirrored pin is at the mirrored place, for every rotation",
    (typeId) => {
      const { pins } = symbols[typeId]!;

      for (const rotation of ROTATIONS) {
        const mirrored = resolvePinPositions(
          part(typeId, rotation, true),
          symbols,
        );

        pins.forEach((pin, index) => {
          // rotate(mirror(local)), then the position.
          const expected = rotateLocal(
            { x: -pin.local.x, y: pin.local.y },
            rotation,
          );
          expect(mirrored[index]).toEqual({
            pinId: pin.id,
            position: { x: ORIGIN.x + expected.x, y: ORIGIN.y + expected.y },
          });
        });
      }
    },
  );

  it.each(TYPE_IDS)(
    "%s: mirror then mirror returns the original, and an unmirrored part is unchanged",
    (typeId) => {
      for (const rotation of ROTATIONS) {
        const plain = resolvePinPositions(
          part(typeId, rotation, false),
          symbols,
        );
        // The mirror of the mirror: pins of a part mirrored about the same vertical axis of its
        // symbol, then mirrored again, are the plain ones.
        const twice = resolvePinPositions(
          { ...part(typeId, rotation, true), mirrored: false },
          symbols,
        );

        expect(twice).toEqual(plain);
        // Mirrored pins, flipped back in the local frame, are the plain ones.
        const mirrored = resolvePinPositions(
          part(typeId, rotation, true),
          symbols,
        );
        mirrored.forEach((pin, index) => {
          const local = symbols[typeId]!.pins[index]!.local;
          const back = rotateLocal({ x: -local.x, y: local.y }, rotation);
          const plainLocal = rotateLocal(local, rotation);
          expect(pin.position).toEqual({
            x: ORIGIN.x + back.x,
            y: ORIGIN.y + back.y,
          });
          expect(plain[index]!.position).toEqual({
            x: ORIGIN.x + plainLocal.x,
            y: ORIGIN.y + plainLocal.y,
          });
        });
      }
    },
  );

  it("sends the mirrored positions to the Engine, with one pin each", () => {
    const envelope = buildGeometryEnvelope(
      {
        schemaVersion: 3,
        components: [part("diode", 90, true)],
        wires: [],
        junctions: [],
        referenceMarkers: [],
        rails: [],
        documentation: emptyDocumentation(),
      },
      symbols,
    );

    expect(envelope.components).toEqual([
      {
        componentId: "00000000-0000-4000-8000-0000000000aa",
        pins: resolvePinPositions(part("diode", 90, true), symbols),
      },
    ]);
  });

  it("draws the symbol flipped too, so the drawing and the pins agree", () => {
    const scene = (mirrored: boolean) =>
      buildScene(
        {
          schemaVersion: 3,
          components: [part("diode", 0, mirrored)],
          wires: [],
          junctions: [],
          referenceMarkers: [],
          rails: [],
          documentation: emptyDocumentation(),
        },
        symbols,
      );

    const plain = scene(false).bounds;
    const flipped = scene(true).bounds;
    // The symbol of a diode is not symmetric about its vertical axis, but its bounds mirror
    // about the origin's x: the same extent measured from the other side.
    expect(flipped.minX - ORIGIN.x).toBeCloseTo(ORIGIN.x - plain.maxX);
    expect(flipped.maxX - ORIGIN.x).toBeCloseTo(ORIGIN.x - plain.minX);
    expect(flipped.minY).toBeCloseTo(plain.minY);
  });
});
