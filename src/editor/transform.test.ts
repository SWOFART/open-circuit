import { describe, expect, it } from "vitest";
import schematicFixture from "../../tests/golden/stage-1-divider.schematic.json";
import type { GridPoint } from "../generated/GridPoint";
import type { Schematic } from "../generated/Schematic";
import { symbols } from "../symbols";
import { resolvePinPositions, type Rotation } from "./geometry";
import {
  applyComponentTransform,
  followingWireEdits,
  rotateClockwise,
  simplifyPath,
} from "./transform";

const schematic = schematicFixture as Schematic;
const V1 = "00000000-0000-4000-8000-000000000001";
const R1 = "00000000-0000-4000-8000-000000000002";
const LOOP_WIRE = "00000000-0000-4000-8000-000000000004";
const MIDDLE_WIRE = "00000000-0000-4000-8000-000000000005";
const TOP_WIRE = "00000000-0000-4000-8000-000000000006";

function expectOrthogonalPath(points: readonly GridPoint[]): void {
  expect(points.length).toBeGreaterThanOrEqual(2);
  for (let index = 1; index < points.length; index += 1) {
    const a = points[index - 1];
    const b = points[index];
    if (!a || !b) throw new Error("missing point");
    // Exactly one coordinate changes: no diagonal and no zero-length segment.
    expect((a.x === b.x) !== (a.y === b.y)).toBe(true);
  }
}

describe("rotation", () => {
  it("steps clockwise through the four rotations", () => {
    expect(
      [0, 90, 180, 270].map((r) => rotateClockwise(r as Rotation)),
    ).toEqual([90, 180, 270, 0]);
  });
});

describe("path simplification", () => {
  it("drops repeated points and straight-through points", () => {
    expect(
      simplifyPath([
        { x: 0, y: 0 },
        { x: 0, y: 0 },
        { x: 2, y: 0 },
        { x: 4, y: 0 },
        { x: 4, y: 3 },
      ]),
    ).toEqual([
      { x: 0, y: 0 },
      { x: 4, y: 0 },
      { x: 4, y: 3 },
    ]);
  });

  it("keeps the turning point of a path that doubles back", () => {
    const doubled = [
      { x: 0, y: 0 },
      { x: 5, y: 0 },
      { x: 2, y: 0 },
    ];

    expect(simplifyPath(doubled)).toEqual(doubled);
  });

  it("collapses a path whose points are all equal", () => {
    expect(
      simplifyPath([
        { x: 3, y: 3 },
        { x: 3, y: 3 },
      ]),
    ).toEqual([{ x: 3, y: 3 }]);
  });
});

describe("wires following a moved component", () => {
  it("re-routes only the wire endpoints that sat on R1's pins", () => {
    const edits = followingWireEdits(
      schematic,
      R1,
      { position: { x: 4, y: 2 }, rotationDeg: 0 },
      symbols,
    );

    expect(edits).toEqual([
      {
        id: LOOP_WIRE,
        points: [
          { x: -1, y: 0 },
          { x: -1, y: 4 },
          { x: 3, y: 4 },
          { x: 3, y: 2 },
        ],
      },
      {
        id: MIDDLE_WIRE,
        points: [
          { x: 5, y: 2 },
          { x: 7, y: 2 },
          { x: 7, y: 0 },
        ],
      },
    ]);
  });

  it("ends every followed wire exactly on the new pin positions", () => {
    const placement = { position: { x: 4, y: 0 }, rotationDeg: 90 as const };
    const edits = followingWireEdits(schematic, R1, placement, symbols);
    const component = schematic.components.find((item) => item.id === R1);
    if (!component) throw new Error("R1 missing");
    const newPins = resolvePinPositions({ ...component, ...placement }, symbols)
      .map((pin) => `${pin.position.x},${pin.position.y}`)
      .sort();
    const endpoints = edits
      .flatMap((edit) => [edit.points[0], edit.points[edit.points.length - 1]])
      .filter((point) => point && newPins.includes(`${point.x},${point.y}`));

    expect(endpoints).toHaveLength(2);
  });

  it.each([
    { delta: { x: 0, y: 0 }, rotationDeg: 90 },
    { delta: { x: 0, y: 0 }, rotationDeg: 180 },
    { delta: { x: 0, y: 0 }, rotationDeg: 270 },
    { delta: { x: 3, y: -2 }, rotationDeg: 0 },
    { delta: { x: -5, y: 7 }, rotationDeg: 90 },
    { delta: { x: 1, y: 1 }, rotationDeg: 270 },
  ] as const)(
    "keeps every path orthogonal for each component (delta $delta.x,$delta.y, $rotationDeg°)",
    ({ delta, rotationDeg }) => {
      for (const component of schematic.components) {
        const edits = followingWireEdits(
          schematic,
          component.id,
          {
            position: {
              x: component.position.x + delta.x,
              y: component.position.y + delta.y,
            },
            rotationDeg,
          },
          symbols,
        );

        for (const edit of edits) {
          expectOrthogonalPath(edit.points);
        }
      }
    },
  );

  it("routes a rotated component's wires around its body and other pin", () => {
    // Move R1 down first, so both of its wires arrive horizontally.
    const down = { position: { x: 4, y: 2 }, rotationDeg: 0 as const };
    const moved = applyComponentTransform(
      schematic,
      R1,
      down,
      followingWireEdits(schematic, R1, down, symbols),
    );
    const turned = { position: { x: 4, y: 2 }, rotationDeg: 90 as const };

    const edits = followingWireEdits(moved, R1, turned, symbols);

    // Pins are now (4, 1) and (4, 3). Going straight down from (4, 1) would
    // run through the body and over pin2, shorting R1; the other bend wins.
    expect(edits).toEqual([
      {
        id: LOOP_WIRE,
        points: [
          { x: -1, y: 0 },
          { x: -1, y: 4 },
          { x: 3, y: 4 },
          { x: 3, y: 1 },
          { x: 4, y: 1 },
        ],
      },
      {
        id: MIDDLE_WIRE,
        points: [
          { x: 4, y: 3 },
          { x: 7, y: 3 },
          { x: 7, y: 0 },
        ],
      },
    ]);
  });

  it("leaves wires alone when the pins do not move", () => {
    expect(
      followingWireEdits(
        schematic,
        R1,
        { position: { x: 4, y: 0 }, rotationDeg: 0 },
        symbols,
      ),
    ).toEqual([]);
  });

  it("does not follow a pin that only touches a wire's interior", () => {
    // A resistor whose pin1 lies inside the top wire's horizontal run.
    const touching: Schematic = {
      ...schematic,
      components: [
        {
          typeId: "resistor",
          id: "00000000-0000-4000-8000-0000000000f1",
          position: { x: 6, y: -4 },
          rotationDeg: 0,
          parameters: { resistanceOhms: 1000 },
        },
      ],
    };

    expect(
      followingWireEdits(
        touching,
        "00000000-0000-4000-8000-0000000000f1",
        { position: { x: 6, y: -6 }, rotationDeg: 0 },
        symbols,
      ),
    ).toEqual([]);
  });

  it("moves both ends of a wire attached to the same component", () => {
    const looped: Schematic = {
      ...schematic,
      components: [
        {
          typeId: "resistor",
          id: "00000000-0000-4000-8000-0000000000f1",
          position: { x: 0, y: 0 },
          rotationDeg: 0,
          parameters: { resistanceOhms: 1000 },
        },
      ],
      wires: [
        {
          id: "00000000-0000-4000-8000-0000000000f2",
          points: [
            { x: -1, y: 0 },
            { x: -1, y: 2 },
            { x: 1, y: 2 },
            { x: 1, y: 0 },
          ],
        },
      ],
    };

    expect(
      followingWireEdits(
        looped,
        "00000000-0000-4000-8000-0000000000f1",
        { position: { x: 0, y: -1 }, rotationDeg: 0 },
        symbols,
      ),
    ).toEqual([
      {
        id: "00000000-0000-4000-8000-0000000000f2",
        points: [
          { x: -1, y: -1 },
          { x: -1, y: 2 },
          { x: 1, y: 2 },
          { x: 1, y: -1 },
        ],
      },
    ]);
  });

  it("leaves the untouched part of a followed wire exactly as drawn", () => {
    // The wire runs out to x = 12 and doubles back to end at x = 10, where a
    // T-contact could sit; only the end at R1's pin2 is re-routed.
    const doubled: Schematic = {
      ...schematic,
      wires: [
        {
          id: "00000000-0000-4000-8000-0000000000f2",
          points: [
            { x: 5, y: 0 },
            { x: 12, y: 0 },
            { x: 12, y: 2 },
            { x: 12, y: 1 },
            { x: 10, y: 1 },
          ],
        },
      ],
    };

    expect(
      followingWireEdits(
        doubled,
        R1,
        { position: { x: 4, y: -2 }, rotationDeg: 0 },
        symbols,
      ),
    ).toEqual([
      {
        id: "00000000-0000-4000-8000-0000000000f2",
        points: [
          { x: 5, y: -2 },
          { x: 12, y: -2 },
          // The old joint at (12, 0) is now inside a straight run; the run
          // still reaches x = 12, y = 2 and turns back as drawn.
          { x: 12, y: 2 },
          { x: 12, y: 1 },
          { x: 10, y: 1 },
        ],
      },
    ]);
  });

  it("skips a wire that would collapse to a point", () => {
    const short: Schematic = {
      ...schematic,
      wires: [
        {
          id: "00000000-0000-4000-8000-0000000000f2",
          points: [
            { x: 5, y: 0 },
            { x: 6, y: 0 },
          ],
        },
      ],
    };

    // R1's pin2 moves from (5, 0) onto the wire's other end at (6, 0).
    expect(
      followingWireEdits(
        short,
        R1,
        { position: { x: 5, y: 0 }, rotationDeg: 0 },
        symbols,
      ),
    ).toEqual([]);
  });

  it("returns nothing for an unknown component", () => {
    expect(
      followingWireEdits(
        schematic,
        "00000000-0000-4000-8000-0000000000ff",
        { position: { x: 0, y: 0 }, rotationDeg: 0 },
        symbols,
      ),
    ).toEqual([]);
  });
});

describe("applying a transform for previews", () => {
  it("moves the component and replaces only the edited wires", () => {
    const placement = { position: { x: 1, y: 3 }, rotationDeg: 180 as const };
    const edits = followingWireEdits(schematic, V1, placement, symbols);
    const preview = applyComponentTransform(schematic, V1, placement, edits);

    expect(preview.components.find((item) => item.id === V1)).toMatchObject(
      placement,
    );
    expect(preview.wires.find((wire) => wire.id === TOP_WIRE)?.points).toEqual(
      edits.find((edit) => edit.id === TOP_WIRE)?.points,
    );
    expect(preview.wires.find((wire) => wire.id === MIDDLE_WIRE)).toBe(
      schematic.wires.find((wire) => wire.id === MIDDLE_WIRE),
    );
    // The original is untouched.
    expect(
      schematic.components.find((item) => item.id === V1)?.position,
    ).toEqual({ x: 0, y: 0 });
  });
});
