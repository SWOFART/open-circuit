import { readFileSync, writeFileSync } from "node:fs";
import { expect, it } from "vitest";
import starters from "../../tests/golden/stage-10/starters.json";
import fixture from "../../tests/golden/stage-1-catalog.json";
import type { EditorCommand } from "../editor/commands";
import { buildGeometryEnvelope, resolvePinPositions } from "../editor/geometry";
import type { CompileRequest } from "../generated/CompileRequest";
import type { ComponentCatalog } from "../generated/ComponentCatalog";
import type { ComponentTypeId } from "../generated/ComponentTypeId";
import type { GridPoint } from "../generated/GridPoint";
import type { ParameterId } from "../generated/ParameterId";
import type { PinId } from "../generated/PinId";
import type { StableId } from "../generated/StableId";
import { symbols } from "../symbols";
import { applyEditorCommand } from "../ui/editorCommands";
import { SchematicDocumentStore } from "./document";
import { blankSchematic } from "./projectWorkflow";

// The direct-on-line and star-delta starters of Stage 10, drawn from a blank document with the
// editor's own commands (place a part, edit its parameters, draw wires, mark the reference) and
// the real symbols. The parts and their parameters are those of the independent fixture
// tests/golden/stage-10/starters.json; the wiring is drawn here, from the connection lists below,
// with the new symbols' pins. crates/app/tests/stage10_acceptance.rs proves with the Engine's own
// connectivity that each drawing joins exactly the pins of the fixture and runs it through the
// bridge against the independent analytical and ngspice answers.

const catalog = fixture as ComponentCatalog;
const p = (x: number, y: number): GridPoint => ({ x, y });

/** The pins that one net joins, each as `<part number>.<pin>`; parts are numbered from 1. */
type Net = readonly string[];

const DIRECT_ON_LINE: readonly Net[] = [
  ["1.l1", "3.l1"],
  ["1.l2", "3.l2"],
  ["1.l3", "3.l3"],
  [
    "1.n",
    "2.pin2",
    "3.no_1",
    "3.no_2",
    "5.a2",
    "5.nc_1",
    "5.nc_2",
    "5.no_1",
    "5.no_2",
    "6.pin2",
  ],
  ["2.pin1", "3.nc_1"],
  ["3.nc_2", "4.pin1"],
  ["3.t1", "5.l1"],
  ["3.t2", "5.l2"],
  ["3.t3", "5.l3"],
  ["4.pin2", "5.a1", "6.pin1"],
  ["5.t1", "7.u1", "8.pin1"],
  ["5.t2", "7.v1", "9.pin1"],
  ["5.t3", "7.w1", "10.pin1"],
  ["7.u2", "7.v2", "7.w2", "8.pin2", "9.pin2", "10.pin2"],
];

/** The same starter with two phases swapped on the way to the overload relay. */
const DIRECT_ON_LINE_SWAPPED: readonly Net[] = DIRECT_ON_LINE.map((net) =>
  net.map((pin) => (pin === "1.l2" ? "1.l3" : pin === "1.l3" ? "1.l2" : pin)),
).map((net) => [...net].sort(netOrder));

const STAR_DELTA: readonly Net[] = [
  ["1.l1", "3.l1"],
  ["1.l2", "3.l2"],
  ["1.l3", "3.l3"],
  [
    "1.n",
    "2.pin2",
    "3.no_1",
    "3.no_2",
    "5.a2",
    "5.nc_1",
    "5.nc_2",
    "5.no_1",
    "5.no_2",
    "6.pin2",
    "9.a2",
    "9.no_1",
    "9.no_2",
    "10.pin2",
    "11.a2",
    "11.no_1",
    "11.no_2",
    "12.pin2",
  ],
  ["2.pin1", "3.nc_1", "9.nc_1", "11.nc_1"],
  ["3.nc_2", "4.pin1"],
  ["3.t1", "5.l1"],
  ["3.t2", "5.l2"],
  ["3.t3", "5.l3"],
  ["4.pin2", "5.a1", "6.pin1"],
  ["5.t1", "11.t3", "13.u1", "14.pin1"],
  ["5.t2", "11.t1", "13.v1", "15.pin1"],
  ["5.t3", "11.t2", "13.w1", "16.pin1"],
  ["7.pin1", "11.nc_2"],
  ["7.pin2", "9.a1", "10.pin1"],
  ["8.pin1", "9.nc_2"],
  ["8.pin2", "11.a1", "12.pin1"],
  ["9.l1", "11.l1", "13.u2", "14.pin2"],
  ["9.l2", "11.l2", "13.v2", "15.pin2"],
  ["9.l3", "11.l3", "13.w2", "16.pin2"],
  ["9.t1", "9.t2", "9.t3"],
];

function netOrder(a: string, b: string): number {
  const [partA = 0, pinA = ""] = [Number.parseInt(a, 10), a.split(".")[1]];
  const [partB = 0, pinB = ""] = [Number.parseInt(b, 10), b.split(".")[1]];
  return partA - partB || String(pinA).localeCompare(String(pinB));
}

/**
 * How a pin leaves its part when the default (straight up for a pin on the upper half or the
 * middle, straight down for one on the lower half) would run through another pin of the same
 * part: sideways first, then up.
 */
const ESCAPES: Readonly<Record<string, number>> = {
  // The middle phase terminal of the star source shares its column with the two others.
  "1.l2": 4,
};

class Drawing {
  private next = 1;
  private readonly store = new SchematicDocumentStore(
    blankSchematic,
    () => `00000000-0000-4000-8000-${String(this.next++).padStart(12, "0")}`,
  );
  private readonly ids: StableId[] = [];
  private cursor = 0;

  constructor(private readonly escapes = ESCAPES) {}

  private apply(command: EditorCommand) {
    applyEditorCommand(this.store, command, catalog);
  }

  /** Places the next part in the row, `gap` grid units after the previous one. */
  place(
    typeId: ComponentTypeId,
    parameters: Readonly<Record<string, number>>,
  ): void {
    const symbol = symbols[typeId]!;
    const xs = symbol.pins.map((pin) => pin.local.x);
    const origin = p(this.cursor - Math.min(...xs), ROW_Y);

    this.cursor += Math.max(...xs) - Math.min(...xs) + GAP;
    this.apply({
      kind: "placeComponent",
      typeId,
      position: origin,
      rotationDeg: 0,
    });
    const component = this.store.getState().schematic.components.at(-1)!;
    expect(component.typeId).toBe(typeId);
    for (const [key, value] of Object.entries(parameters)) {
      this.store.setParameter(component.id, key as ParameterId, value, catalog);
    }
    this.ids.push(component.id);
  }

  /** The grid position of `<part>.<pin>`, where parts are numbered from 1. */
  pin(reference: string): GridPoint {
    const [part, pin] = reference.split(".");
    const id = this.ids[Number(part) - 1]!;
    const component = this.store
      .getState()
      .schematic.components.find((item) => item.id === id)!;

    return resolvePinPositions(component, symbols).find(
      (item) => item.pinId === (pin as PinId),
    )!.position;
  }

  /**
   * Draws every net as a bus above or below the row of parts. A pin on the upper half or the
   * middle of its part runs straight up to the upper bus of its net, a pin on the lower half
   * straight down to the lower bus; a net with both has a channel to the left of the row that
   * joins its two buses. Every bus and every channel has a line of its own, so a wire only
   * ends where its net says, and no wire runs through a pin of another net.
   */
  route(nets: readonly Net[]) {
    const left = Math.min(
      ...this.ids.flatMap((_, index) =>
        this.pinsOf(index + 1).map((pin) => pin.x),
      ),
    );
    const top = ROW_Y - 5;
    const bottom = ROW_Y + 5;

    nets.forEach((net, lane) => {
      const upper = net.filter((pin) => this.isUpper(pin));
      const lower = net.filter((pin) => !this.isUpper(pin));
      const upperY = top - 2 * lane;
      const lowerY = bottom + 2 * lane;
      const channelX = left - 2 - 2 * lane;
      const stub = (pin: string) => this.escapes[pin] ?? 0;
      const dropX = (pin: string) => this.pin(pin).x + stub(pin);

      for (const pin of upper) {
        const at = this.pin(pin);
        this.apply({
          kind: "addWire",
          points:
            stub(pin) === 0
              ? [at, p(at.x, upperY)]
              : [at, p(dropX(pin), at.y), p(dropX(pin), upperY)],
        });
      }
      for (const pin of lower) {
        const at = this.pin(pin);
        this.apply({ kind: "addWire", points: [at, p(at.x, lowerY)] });
      }
      const upperXs = upper.map(dropX);
      const lowerXs = lower.map((pin) => this.pin(pin).x);

      if (upper.length > 0 && lower.length > 0) {
        this.apply({
          kind: "addWire",
          points: [
            p(Math.max(...upperXs), upperY),
            p(channelX, upperY),
            p(channelX, lowerY),
            p(Math.max(...lowerXs), lowerY),
          ],
        });
      } else if (upper.length > 1) {
        this.apply({
          kind: "addWire",
          points: [
            p(Math.min(...upperXs), upperY),
            p(Math.max(...upperXs), upperY),
          ],
        });
      } else if (lower.length > 1) {
        this.apply({
          kind: "addWire",
          points: [
            p(Math.min(...lowerXs), lowerY),
            p(Math.max(...lowerXs), lowerY),
          ],
        });
      }
    });
  }

  private pinsOf(part: number): GridPoint[] {
    const component = this.store
      .getState()
      .schematic.components.find((item) => item.id === this.ids[part - 1])!;

    return resolvePinPositions(component, symbols).map((pin) => pin.position);
  }

  private isUpper(reference: string): boolean {
    const part = Number(reference.split(".")[0]);
    const component = this.store
      .getState()
      .schematic.components.find((item) => item.id === this.ids[part - 1])!;

    return this.pin(reference).y <= component.position.y;
  }

  reference(position: GridPoint) {
    this.apply({ kind: "addReferenceMarker", position });
  }

  request(): CompileRequest {
    const schematic = this.store.getState().schematic;

    return {
      schematic,
      geometry: buildGeometryEnvelope(schematic, symbols),
      documentRevision: "rev-1",
    };
  }
}

const ROW_Y = 30;
const GAP = 8;

/** Draws the starter of the fixture `name` with the given connections. */
function starter(name: keyof typeof starters, nets: readonly Net[]) {
  const drawing = new Drawing();
  const parts = starters[name].request.schematic.components;

  for (const part of parts) {
    drawing.place(
      part.typeId as ComponentTypeId,
      part.parameters as unknown as Record<string, number>,
    );
  }
  drawing.route(nets);
  // The explicit reference of the drawn network is the star point of the supply.
  drawing.reference(drawing.pin("1.n"));
  return drawing.request();
}

const DRAWINGS = [
  ["direct-on-line", DIRECT_ON_LINE],
  ["direct-on-line-swapped", DIRECT_ON_LINE_SWAPPED],
  ["star-delta", STAR_DELTA],
  [
    "star-delta-swapped",
    STAR_DELTA.map((net) =>
      net.map((pin) =>
        pin === "1.l2" ? "1.l3" : pin === "1.l3" ? "1.l2" : pin,
      ),
    ).map((net) => [...net].sort(netOrder)),
  ],
] as const satisfies readonly (readonly [
  keyof typeof starters,
  readonly Net[],
])[];

it.each(DRAWINGS)(
  "draws %s from a blank document with the same parts as its fixture",
  (name, nets) => {
    const request = starter(name, nets);
    const fixtureParts = starters[name].request.schematic.components;
    const drawn = request.schematic.components;

    expect(drawn.map((part) => part.typeId)).toEqual(
      fixtureParts.map((part) => part.typeId),
    );
    expect(drawn.map((part) => part.parameters)).toEqual(
      fixtureParts.map((part) => part.parameters),
    );
    // The same IDs as the fixture's parts, so the fixture's operator events name the same
    // switches (the editor numbers parts in the order they are placed).
    expect(drawn.map((part) => part.id)).toEqual(
      fixtureParts.map((part) => part.id),
    );
    expect(request.schematic.referenceMarkers).toHaveLength(1);
    // Nothing is drawn that is not an input: no junction, no rail.
    expect(request.schematic.junctions).toEqual([]);
    expect(request.schematic.rails).toEqual([]);
  },
);

it.each(DRAWINGS)("pins the editor-drawn request of %s", (name, nets) => {
  const path = new URL(
    `../../tests/golden/stage-10-drawn-${name}.request.json`,
    import.meta.url,
  );
  const json = `${JSON.stringify(starter(name, nets), null, 2)}\n`;
  if (process.env.UPDATE_GOLDEN === "1") writeFileSync(path, json);
  expect(readFileSync(path, "utf8")).toBe(json);
});

function timedStarDelta() {
  const drawing = new Drawing({ ...ESCAPES, "8.no1": -1, "8.no2": 1 });
  const relay = {
    coilResistanceOhms: 240,
    pickupCurrentAmps: 0.08,
    dropoutCurrentAmps: 0.04,
    closedResistanceOhms: 0.01,
  };
  starters["star-delta"].request.schematic.components.forEach((part, index) => {
    if (index === 6 || index === 7) {
      drawing.place("relay_on_delay", {
        ...relay,
        delaySeconds: index === 6 ? 0.09 : 0.12,
      });
    } else {
      drawing.place(
        part.typeId as ComponentTypeId,
        part.parameters as unknown as Record<string, number>,
      );
    }
  });
  const timerPins: Record<string, string> = {
    "7.pin1": "7.nc1",
    "7.pin2": "7.nc2",
    "8.pin1": "8.no1",
    "8.pin2": "8.no2",
  };
  drawing.route(
    STAR_DELTA.map((net) => [
      ...net.map((pin) => timerPins[pin] ?? pin),
      ...(net.includes("1.n") ? ["7.a2", "8.a2"] : []),
      ...(net.includes("5.a1") ? ["7.a1", "8.a1"] : []),
    ]),
  );
  drawing.reference(drawing.pin("1.n"));
  return drawing.request();
}

function emergencyStopChain() {
  const drawing = new Drawing({ "3.no1": -1, "3.no2": 1 });
  drawing.place("dc_voltage_source", {
    voltageVolts: 24,
    internalResistanceOhms: 0,
  });
  drawing.place("emergency_stop", { closedResistanceOhms: 0.01 });
  drawing.place("relay_on_delay", { delaySeconds: 0.003 });
  drawing.place("lamp", { resistanceOhms: 120 });
  drawing.route([
    ["1.pin1", "2.p", "3.no1"],
    ["1.pin2", "3.a2", "4.n"],
    ["2.n", "3.a1"],
    ["3.no2", "4.p"],
  ]);
  drawing.reference(drawing.pin("1.pin2"));
  return drawing.request();
}

function ledAmplifier() {
  const drawing = new Drawing({});
  drawing.place("dc_voltage_source", {
    voltageVolts: 3,
    internalResistanceOhms: 0,
  });
  drawing.place("operational_amplifier", {
    openLoopGain: 1000,
    limitVolts: 12,
    outputResistanceOhms: 1,
  });
  drawing.place("resistor", { resistanceOhms: 220 });
  drawing.place("led", {
    saturationCurrentAmps: 1e-14,
    idealityFactor: 2,
    litCurrentAmps: 0.001,
  });
  drawing.route([
    ["1.pin1", "2.plus"],
    ["1.pin2", "2.ref", "4.n"],
    ["2.minus", "2.out", "3.pin1"],
    ["3.pin2", "4.p"],
  ]);
  drawing.reference(drawing.pin("1.pin2"));
  return drawing.request();
}

it.each([
  ["timed-star-delta", timedStarDelta],
  ["emergency-stop-chain", emergencyStopChain],
  ["led-amplifier", ledAmplifier],
] as const)(
  "draws and pins Stage 14 %s from a blank document",
  (name, draw) => {
    const path = new URL(
      "../../tests/golden/stage-14/" + name + ".request.json",
      import.meta.url,
    );
    const content = JSON.stringify(draw(), null, 2) + "\n";
    if (process.env.UPDATE_GOLDEN === "1") writeFileSync(path, content);
    expect(readFileSync(path, "utf8")).toBe(content);
  },
);
