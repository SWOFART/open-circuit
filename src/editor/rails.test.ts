import { describe, expect, it } from "vitest";
import cases from "../../tests/golden/stage-8-connectivity.json";
import type { GridPoint } from "../generated/GridPoint";
import type { Schematic } from "../generated/Schematic";
import { dividerSchematic } from "../store/fixtures";
import { symbols } from "../symbols";
import { analyzeGeometry } from "./connectivity";
import { hitTest, railHandleAt } from "./hitTest";
import { CanvasInteraction, type InteractionIntent } from "./interaction";
import {
  buildScene,
  LABEL_HALF_WIDTH,
  railLabelHalfWidth,
  RAIL_END_RADIUS,
} from "./scene";
import { toScreen } from "./view";

const RAIL = "00000000-0000-4000-8000-000000008001";
const RAIL_B = "00000000-0000-4000-8000-000000008002";
const WIRE = "00000000-0000-4000-8000-000000000a01";
const WIRE_B = "00000000-0000-4000-8000-000000000a02";
const JUNCTION = "00000000-0000-4000-8000-000000000a11";
const MARKER = "00000000-0000-4000-8000-000000000a21";

const p = (x: number, y: number): GridPoint => ({ x, y });

function rail(
  id: string,
  name: string,
  x: number,
  top: number,
  bottom: number,
) {
  return { id, name, start: p(x, top), end: p(x, bottom) };
}

function drawing(parts: Partial<Schematic>): Schematic {
  return {
    schemaVersion: 2,
    components: [],
    wires: [],
    junctions: [],
    referenceMarkers: [],
    rails: [],
    ...parts,
  };
}

function wire(id: string, ...points: [number, number][]) {
  return { id, points: points.map(([x, y]) => p(x, y)) };
}

const key = (point: GridPoint) => `${point.x},${point.y}`;

describe("rails in the drawn scene", () => {
  const schematic = drawing({ rails: [rail(RAIL, "L+", 4, 0, 10)] });
  const scene = buildScene(schematic, symbols, {
    geometry: analyzeGeometry(schematic, symbols),
  });

  it("draws a vertical line, a marker on each end, and the name above the top", () => {
    const own = scene.shapes.filter(
      (shape) => shape.owner.kind === "rail" && shape.owner.id === RAIL,
    );
    const line = own.find((shape) => shape.kind === "polyline");
    const ends = own.filter((shape) => shape.kind === "circle");
    const label = own.find((shape) => shape.kind === "label");

    expect(line).toMatchObject({ role: "rail", points: [p(4, 0), p(4, 10)] });
    expect(ends.map((end) => end.kind === "circle" && end.center)).toEqual([
      p(4, 0),
      p(4, 10),
    ]);
    expect(ends[0]).toMatchObject({ role: "railEnd", radius: RAIL_END_RADIUS });
    expect(label).toMatchObject({ text: "L+" });
    expect(label?.kind === "label" && label.position.y).toBeLessThan(0);
  });

  it("keeps the name as plain text and never as an identity", () => {
    const hostile = drawing({
      rails: [rail(RAIL, "<b>L1</b>", 0, 0, 5), rail(RAIL_B, "L1", 3, 0, 5)],
    });
    const hostileScene = buildScene(hostile, symbols);
    const labels = hostileScene.shapes.filter(
      (shape) => shape.kind === "label",
    );

    expect(labels.map((label) => label.kind === "label" && label.text)).toEqual(
      ["<b>L1</b>", "L1"],
    );
    // Two rails with one name stay two objects, told apart by ID only.
    expect(new Set(labels.map((label) => label.owner.id)).size).toBe(2);
  });

  it("sizes the label to its text and puts that extent in the bounds", () => {
    expect(railLabelHalfWidth("L+")).toBeLessThan(
      railLabelHalfWidth("SUPPLY_RAIL_24V_LONG_NAME"),
    );
    const long = drawing({
      rails: [rail(RAIL, "ABCDEFGHIJKLMNOPQRSTUVWXYZ012345", 0, 0, 5)],
    });
    const bounds = buildScene(long, symbols).bounds;

    expect(bounds.minX).toBeLessThan(-LABEL_HALF_WIDTH);
    expect(bounds.maxX).toBeGreaterThan(LABEL_HALF_WIDTH);
    // The line and the label above it are both inside.
    expect(bounds.minY).toBeLessThan(0);
    expect(bounds.maxY).toBeGreaterThanOrEqual(5);
  });

  it("fits the whole line even without any component", () => {
    expect(scene.bounds.minY).toBeLessThan(0);
    expect(scene.bounds.maxY).toBeGreaterThanOrEqual(10);
  });

  it("draws a rail that a diagnostic names as flagged, and the others as they are", () => {
    const two = drawing({
      rails: [rail(RAIL, "A", 0, 0, 5), rail(RAIL_B, "B", 3, 0, 5)],
    });
    const flagged = buildScene(two, symbols, {
      flaggedRails: new Set([RAIL]),
    });
    const flaggedShapes = flagged.shapes.filter(
      (shape) => shape.kind !== "label" && shape.flagged === true,
    );

    expect(flaggedShapes.length).toBe(3);
    expect(flaggedShapes.every((shape) => shape.owner.id === RAIL)).toBe(true);
  });
});

describe("selecting rails", () => {
  const point = (x: number, y: number) => p(x, y);

  it("picks the line, an end and the name, and nothing beyond the tolerance", () => {
    const scene = buildScene(
      drawing({ rails: [rail(RAIL, "L+", 4, 0, 10)] }),
      symbols,
    );

    expect(hitTest(scene, point(4, 5), 0.2)).toEqual({
      kind: "rail",
      id: RAIL,
    });
    expect(hitTest(scene, point(4.15, 5), 0.2)).toEqual({
      kind: "rail",
      id: RAIL,
    });
    expect(hitTest(scene, point(4, 10), 0.2)).toEqual({
      kind: "rail",
      id: RAIL,
    });
    // The name sits above the top end.
    expect(hitTest(scene, point(4, -0.8), 0.1)).toEqual({
      kind: "rail",
      id: RAIL,
    });
    expect(hitTest(scene, point(4.5, 5), 0.2)).toBeNull();
    expect(hitTest(scene, point(4, 10.5), 0.2)).toBeNull();
  });

  it("puts markers before components, components before rails, rails before wires", () => {
    const base = {
      rails: [rail(RAIL, "L+", 5, 0, 10)],
      wires: [wire(WIRE, [5, 3], [5, 8])],
    };
    // A rail and a wire on the same points: the rail wins.
    expect(
      hitTest(buildScene(drawing(base), symbols), point(5, 5), 0.2),
    ).toEqual({ kind: "rail", id: RAIL });

    // A junction or a reference marker at the point wins over the rail.
    expect(
      hitTest(
        buildScene(
          drawing({
            ...base,
            junctions: [{ id: JUNCTION, position: p(5, 5) }],
          }),
          symbols,
        ),
        point(5, 5),
        0.2,
      ),
    ).toEqual({ kind: "junction", id: JUNCTION });
    expect(
      hitTest(
        buildScene(
          drawing({
            ...base,
            referenceMarkers: [{ id: MARKER, position: p(5, 5) }],
          }),
          symbols,
        ),
        point(5, 5),
        0.2,
      ),
    ).toEqual({ kind: "referenceMarker", id: MARKER });

    // A component whose body covers the point wins over the rail.
    const withResistor = drawing({
      ...base,
      components: [
        {
          typeId: "resistor",
          id: "00000000-0000-4000-8000-000000000b01",
          position: p(5, 5),
          rotationDeg: 0,
          parameters: { resistanceOhms: 1000 },
        },
      ],
    });
    expect(
      hitTest(buildScene(withResistor, symbols), point(5, 5), 0.2),
    ).toEqual({
      kind: "component",
      id: "00000000-0000-4000-8000-000000000b01",
    });
  });

  it("takes the nearest rail, and the last drawn on an exact tie", () => {
    const scene = buildScene(
      drawing({
        rails: [rail(RAIL, "A", 4, 0, 10), rail(RAIL_B, "B", 4.3, 0, 10)],
      }),
      symbols,
    );

    expect(hitTest(scene, point(4.05, 5), 0.5)).toEqual({
      kind: "rail",
      id: RAIL,
    });
    expect(hitTest(scene, point(4.28, 5), 0.5)).toEqual({
      kind: "rail",
      id: RAIL_B,
    });
    // Equal distance: the one drawn last is on top.
    expect(hitTest(scene, point(4.15, 5), 0.5)).toEqual({
      kind: "rail",
      id: RAIL_B,
    });
  });

  it("does not pick a rail through a wire that only crosses it", () => {
    const scene = buildScene(
      drawing({
        rails: [rail(RAIL, "L+", 5, 0, 10)],
        wires: [wire(WIRE, [0, 5], [10, 5])],
      }),
      symbols,
    );

    // Away from the crossing each object is its own.
    expect(hitTest(scene, point(2, 5), 0.2)).toEqual({
      kind: "wire",
      id: WIRE,
    });
    expect(hitTest(scene, point(5, 8), 0.2)).toEqual({
      kind: "rail",
      id: RAIL,
    });
  });

  it("works at the legal extreme coordinates", () => {
    const top = -2147483648;
    const bottom = 2147483647;
    const scene = buildScene(
      drawing({ rails: [rail(RAIL, "WIDE", 7, top, bottom)] }),
      symbols,
    );

    expect(hitTest(scene, point(7, 0), 0.2)).toEqual({
      kind: "rail",
      id: RAIL,
    });
    expect(hitTest(scene, point(7, bottom), 0.2)).toEqual({
      kind: "rail",
      id: RAIL,
    });
    expect(hitTest(scene, point(8, 0), 0.2)).toBeNull();
    expect(Number.isFinite(scene.bounds.maxY - scene.bounds.minY)).toBe(true);
  });

  it("offers the selected rail's handles: top end, bottom end, and the line", () => {
    const line = { start: p(4, 0), end: p(4, 10) };

    expect(railHandleAt(line, point(4.1, -0.1), 0.3)).toBe("start");
    expect(railHandleAt(line, point(3.9, 10.1), 0.3)).toBe("end");
    expect(railHandleAt(line, point(4.1, 5), 0.3)).toBe("line");
    expect(railHandleAt(line, point(6, 5), 0.3)).toBeNull();
    // Near both ends of a short rail, the nearer end is the handle.
    const short = { start: p(4, 0), end: p(4, 2) };
    expect(railHandleAt(short, point(4, 0.8), 1.5)).toBe("start");
    expect(railHandleAt(short, point(4, 1.2), 1.5)).toBe("end");
  });
});

describe("connection hints with rails", () => {
  const hints = (schematic: Schematic) => analyzeGeometry(schematic, symbols);

  it("offers a junction where a wire crosses a rail's interior and nothing connects them", () => {
    const geometry = hints(
      drawing({
        rails: [rail(RAIL, "L+", 5, 0, 10)],
        wires: [wire(WIRE, [0, 5], [10, 5])],
      }),
    );

    expect(geometry.junctionSpots.map(key)).toEqual(["5,5"]);
    expect(geometry.isJunctionSpot(p(5, 5))).toBe(true);
    // A crossing alone is no connection: no dot.
    expect(geometry.connectionDots).toEqual([]);
  });

  it("offers no junction at a rail end, with a junction or marker present, or between two rails", () => {
    const rails = [rail(RAIL, "L+", 5, 0, 10)];
    const across = wire(WIRE, [0, 5], [10, 5]);

    expect(
      hints(drawing({ rails, wires: [wire(WIRE, [0, 0], [10, 0])] }))
        .junctionSpots,
    ).toEqual([]);
    expect(
      hints(
        drawing({
          rails,
          wires: [across],
          junctions: [{ id: JUNCTION, position: p(5, 5) }],
        }),
      ).junctionSpots,
    ).toEqual([]);
    expect(
      hints(
        drawing({
          rails,
          wires: [across],
          referenceMarkers: [{ id: MARKER, position: p(5, 5) }],
        }),
      ).junctionSpots,
    ).toEqual([]);
    expect(
      hints(
        drawing({
          rails: [rail(RAIL, "A", 5, 0, 10), rail(RAIL_B, "B", 5, 4, 14)],
        }),
      ).junctionSpots,
    ).toEqual([]);
    // Two wires crossing still need a junction, as before.
    expect(
      hints(
        drawing({
          wires: [wire(WIRE, [0, 5], [10, 5]), wire(WIRE_B, [5, 0], [5, 10])],
        }),
      ).junctionSpots.map(key),
    ).toEqual(["5,5"]);
  });

  it("lets a wire begin and end on every point of a rail, its ends included", () => {
    const geometry = hints(drawing({ rails: [rail(RAIL, "L+", 5, 0, 10)] }));

    for (const y of [0, 1, 5, 9, 10]) {
      expect(geometry.isAttachment(p(5, y))).toBe(true);
    }
    // Neither past the ends, beside the line, nor under the name is an attachment.
    for (const point of [p(5, -1), p(5, 11), p(4, 5), p(6, 5), p(5, -0.8)]) {
      expect(geometry.isAttachment(point)).toBe(false);
    }
  });

  it("draws a dot where a wire ends on a rail's interior and none at a plain corner", () => {
    const geometry = hints(
      drawing({
        rails: [rail(RAIL, "L+", 5, 0, 10)],
        wires: [wire(WIRE, [0, 5], [5, 5]), wire(WIRE_B, [0, 10], [5, 10])],
      }),
    );

    // Interior contact: three arms. At the rail's end it is only a corner.
    expect(geometry.connectionDots.map(key)).toEqual(["5,5"]);
  });

  it("treats a marker on a rail as attached, and one off every conductor as detached", () => {
    const geometry = hints(
      drawing({
        rails: [rail(RAIL, "L+", 5, 0, 10)],
        referenceMarkers: [
          { id: MARKER, position: p(5, 3) },
          { id: "00000000-0000-4000-8000-000000000a22", position: p(6, 3) },
        ],
      }),
    );

    expect([...geometry.detachedMarkers]).toEqual([
      "00000000-0000-4000-8000-000000000a22",
    ]);
  });

  it("keeps the rails by ID for the editor's handles", () => {
    const geometry = hints(drawing({ rails: [rail(RAIL, "L+", 5, 0, 10)] }));

    expect(geometry.rails.get(RAIL)).toEqual([p(5, 0), p(5, 10)]);
  });
});

/**
 * The editor's hints against the Rust fixtures of the contract (section 4): a wire-to-rail
 * crossing is offered a junction exactly where the contract says the geometry does not
 * connect, and a marker that touches no conductor is the one the Engine calls detached.
 * The hints are drawing aids; the Engine's answer stays authoritative.
 */
describe("the editor's hints and the Rust fixtures of section 4", () => {
  const spots: Record<string, string[]> = {
    "interior-crossing": ["0,5"],
    "unique-rename": ["0,5"],
    "reference-on-rail": ["0,5"],
    "duplicate-name": ["0,5"],
    "invalid-name": ["0,5"],
    "reversed-rail": ["0,5"],
    "overflow-safe-extent": ["0,5"],
    "detached-reference": ["0,5"],
    "duplicate-disconnected-name": ["0,5"],
  };

  for (const fixture of cases.cases) {
    it(`reads ${fixture.id} as the contract does`, () => {
      const schematic = fixture.railSchematic as unknown as Schematic;
      const geometry = analyzeGeometry(schematic, symbols);

      expect(geometry.junctionSpots.map(key).sort()).toEqual(
        spots[fixture.id] ?? [],
      );
      expect(geometry.detachedMarkers.size).toBe(
        (fixture.errorCodes as string[]).includes("DetachedReferenceMarker")
          ? 1
          : 0,
      );

      // A spot is never marked with a connection dot (nothing joins there yet), and a point
      // that already has a junction is never offered again.
      for (const spot of geometry.junctionSpots) {
        expect(geometry.connectionDots.map(key)).not.toContain(key(spot));
      }
      for (const junction of schematic.junctions) {
        expect(geometry.isJunctionSpot(junction.position)).toBe(false);
      }
    });
  }

  it("covers each row of the contract's table of exact-grid contacts", () => {
    const ids = cases.cases.map((fixture) => fixture.id);

    for (const row of [
      "interior-crossing",
      "crossing-junction",
      "crossing-reference",
      "wire-end-rail-interior",
      "wire-end-rail-end",
      "rail-end-wire-interior",
      "collinear-wire-overlap",
      "pin-on-rail-interior",
      "rail-overlap",
      "rail-end-touch",
      "rail-gap",
      "reference-on-rail",
      "detached-reference",
      "global-reference-union",
    ]) {
      expect(ids).toContain(row);
    }
  });
});

describe("the wire tool on rails", () => {
  function open(schematic: Schematic) {
    const intents: InteractionIntent[] = [];
    const interaction = new CanvasInteraction(
      () => {},
      (intent) => intents.push(intent),
    );
    interaction.setViewport({ width: 800, height: 600 });
    const geometry = analyzeGeometry(schematic, symbols);
    interaction.setScene(
      buildScene(schematic, symbols, { geometry }),
      geometry,
    );
    const pointer = (x: number, y: number) => ({
      ...toScreen({ x, y }, interaction.view),
      button: 0,
      pointerId: 1,
    });

    return {
      interaction,
      intents,
      click(x: number, y: number) {
        interaction.pointerDown(pointer(x, y));
        interaction.pointerUp(pointer(x, y));
      },
    };
  }

  const railDrawing = () =>
    drawing({
      ...dividerSchematic,
      schemaVersion: 2,
      components: [],
      wires: [],
      junctions: [],
      referenceMarkers: [],
      rails: [rail(RAIL, "L+", 5, 0, 10)],
    });

  it("ends a route on a rail's interior point at once, keeping the route orthogonal", () => {
    const { interaction, intents, click } = open(railDrawing());
    interaction.setTool({ kind: "wire" });

    click(0, 4);
    click(5, 4);

    expect(intents).toEqual([{ kind: "addWire", points: [p(0, 4), p(5, 4)] }]);
  });

  it("starts a route on a rail and bends it with orthogonal legs only", () => {
    const { interaction, intents, click } = open(railDrawing());
    interaction.setTool({ kind: "wire" });

    click(5, 6);
    click(9, 8);
    click(9, 8);

    const [intent] = intents;
    expect(intent?.kind).toBe("addWire");
    const points = intent?.kind === "addWire" ? intent.points : [];
    for (let index = 1; index < points.length; index += 1) {
      const a = points[index - 1]!;
      const b = points[index]!;
      expect(a.x === b.x || a.y === b.y).toBe(true);
    }
  });

  it("does not end a route where it merely crosses a rail", () => {
    const { interaction, intents, click } = open(
      drawing({ rails: [rail(RAIL, "L+", 5, 0, 10)] }),
    );
    interaction.setTool({ kind: "wire" });

    click(0, 4);
    click(9, 4);

    // The click at (9, 4) is not an attachment, so the route is still being drawn.
    expect(intents).toEqual([]);
    expect(interaction.status.routing).toBe(true);
  });

  it("offers the Junction tool a crossing of a wire and a rail, and a reference marker on a rail", () => {
    const crossing = open(
      drawing({
        rails: [rail(RAIL, "L+", 5, 0, 10)],
        wires: [wire(WIRE, [0, 5], [10, 5])],
      }),
    );
    crossing.interaction.setTool({ kind: "junction" });
    crossing.click(5, 5);
    expect(crossing.intents).toEqual([
      { kind: "addJunction", position: p(5, 5) },
    ]);

    const reference = open(drawing({ rails: [rail(RAIL, "0V", 5, 0, 10)] }));
    reference.interaction.setTool({ kind: "reference" });
    reference.click(5, 7);
    expect(reference.intents).toEqual([
      { kind: "addReferenceMarker", position: p(5, 7) },
    ]);
  });
});
