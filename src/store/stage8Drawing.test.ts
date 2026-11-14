import { readFileSync, writeFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";
import catalogFixture from "../../tests/golden/stage-1-catalog.json";
import circuits from "../../tests/golden/stage-8-circuits.json";
import type { EditorCommand } from "../editor/commands";
import { buildGeometryEnvelope } from "../editor/geometry";
import type { CompileRequest } from "../generated/CompileRequest";
import type { ComponentCatalog } from "../generated/ComponentCatalog";
import type { ComponentTypeId } from "../generated/ComponentTypeId";
import type { GridPoint } from "../generated/GridPoint";
import type { ParameterId } from "../generated/ParameterId";
import type { PinId } from "../generated/PinId";
import type { Schematic } from "../generated/Schematic";
import type { StableId } from "../generated/StableId";
import { symbols } from "../symbols";
import { applyEditorCommand } from "../ui/editorCommands";
import { SchematicDocumentStore } from "./document";
import { blankSchematic } from "./projectWorkflow";

// Issue #134: the canonical START/STOP circuit drawn between two named rails from a blank
// document, with editor commands and the real symbol geometry. The pinned compile requests are
// replayed on the real Engine by crates/app/tests/stage8_session.rs against the independent
// observations of tests/golden/stage-8-circuits.json. Regenerate deliberately with
// UPDATE_GOLDEN=1.
const catalog = catalogFixture as ComponentCatalog;
const p = (x: number, y: number): GridPoint => ({ x, y });

class Sketch {
  private next = 1;
  readonly store = new SchematicDocumentStore(
    blankSchematic,
    () =>
      `00000000-0000-4000-8000-${(this.next++).toString(16).padStart(12, "0")}`,
  );

  apply(command: EditorCommand) {
    applyEditorCommand(this.store, command, catalog);
  }

  place(
    typeId: ComponentTypeId,
    position: GridPoint,
    rotationDeg: 0 | 90 | 180 | 270 = 0,
    parameters: Record<string, number> = {},
  ) {
    this.apply({ kind: "placeComponent", typeId, position, rotationDeg });
    const id = this.store.getState().schematic.components.at(-1)!.id;
    for (const [key, value] of Object.entries(parameters)) {
      this.store.setParameter(id, key as ParameterId, value, catalog);
    }
    return id;
  }

  pin(componentId: StableId, pinId: PinId): GridPoint {
    return this.request()
      .geometry.components.find((c) => c.componentId === componentId)!
      .pins.find((pin) => pin.pinId === pinId)!.position;
  }

  wire(...points: GridPoint[]) {
    this.apply({ kind: "addWire", points });
  }

  rail(name: string, x: number, top: number, bottom: number) {
    this.apply({ kind: "addRail", name, x, top, bottom });
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

/**
 * The START/STOP circuit between the rails L+ and 0V: an ideal 5 V source, an NC STOP and an NO
 * START in series with the contactor coil, and, when `sealIn` is set, the contactor's NO
 * contact across START. Parts first, then wires, then the explicit return reference, then the
 * rails, as a user building the circuit would.
 */
function startStop(sealIn: boolean) {
  const sketch = new Sketch();
  const source = sketch.place("dc_voltage_source", p(0, 5), 90, {
    voltageVolts: 5,
    internalResistanceOhms: 0,
  });
  const stop = sketch.place("push_button_nc", p(4, 0));
  const start = sketch.place("push_button_no", p(8, 0));
  const contactor = sketch.place("contactor", p(16, 5));

  sketch.wire(p(0, 0), sketch.pin(stop, "pin1"));
  sketch.wire(sketch.pin(stop, "pin2"), sketch.pin(start, "pin1"));
  sketch.wire(sketch.pin(start, "pin2"), p(14, 0), sketch.pin(contactor, "a1"));
  sketch.wire(p(14, 10), p(0, 10), sketch.pin(source, "pin2"));
  if (sealIn) {
    sketch.wire(
      sketch.pin(start, "pin1"),
      p(7, -3),
      p(18, -3),
      sketch.pin(contactor, "no_1"),
    );
    sketch.wire(
      sketch.pin(contactor, "no_2"),
      p(18, 9),
      p(11, 9),
      p(11, 0),
      sketch.pin(start, "pin2"),
    );
  }
  sketch.apply({
    kind: "addReferenceMarker",
    position: sketch.pin(source, "pin2"),
  });
  // L+ ends on the source's positive terminal and 0V on the contactor's coil return.
  sketch.rail("L+", 0, 0, sketch.pin(source, "pin1").y);
  sketch.rail("0V", 14, sketch.pin(contactor, "a2").y, 10);

  return sketch;
}

/** The Stage 1 divider between rails: a 5 V source with 0.1 ohm, and two 1 kohm resistors. */
function divider() {
  const sketch = new Sketch();
  const source = sketch.place("dc_voltage_source", p(0, 0), 0, {
    voltageVolts: 5,
    internalResistanceOhms: 0.1,
  });
  const first = sketch.place("resistor", p(4, 0), 0, { resistanceOhms: 1000 });
  const second = sketch.place("resistor", p(8, 0), 0, { resistanceOhms: 1000 });

  sketch.wire(p(-1, 4), p(3, 4), sketch.pin(first, "pin1"));
  sketch.wire(sketch.pin(first, "pin2"), sketch.pin(second, "pin1"));
  sketch.wire(p(9, -4), p(1, -4), sketch.pin(source, "pin2"));
  sketch.apply({
    kind: "addReferenceMarker",
    position: sketch.pin(source, "pin2"),
  });
  sketch.rail("L+", -1, sketch.pin(source, "pin1").y, 4);
  sketch.rail("0V", 9, -4, sketch.pin(second, "pin2").y);

  return sketch;
}

/** The RC hold circuit: 10 V, an NC button, 1 kohm and 100 uF between the rails. */
function rcHold() {
  const sketch = new Sketch();
  const source = sketch.place("dc_voltage_source", p(0, 5), 90, {
    voltageVolts: 10,
    internalResistanceOhms: 0,
  });
  const button = sketch.place("push_button_nc", p(4, 0));
  const resistor = sketch.place("resistor", p(8, 0), 0, {
    resistanceOhms: 1000,
  });
  const capacitor = sketch.place("capacitor", p(12, 5), 90, {
    capacitanceFarads: 0.0001,
    initialVoltageVolts: 0,
  });

  sketch.wire(p(0, 0), sketch.pin(button, "pin1"));
  sketch.wire(sketch.pin(button, "pin2"), sketch.pin(resistor, "pin1"));
  sketch.wire(
    sketch.pin(resistor, "pin2"),
    p(12, 0),
    sketch.pin(capacitor, "pin1"),
  );
  sketch.wire(p(12, 10), p(0, 10), sketch.pin(source, "pin2"));
  sketch.apply({
    kind: "addReferenceMarker",
    position: sketch.pin(source, "pin2"),
  });
  sketch.rail("L+", 0, 0, sketch.pin(source, "pin1").y);
  sketch.rail("0V", 12, sketch.pin(capacitor, "pin2").y, 10);

  return sketch;
}

/**
 * Two supplies that share nothing: each source and its resistor sit between their own pair of
 * rails, with no wire and no reference marker. Nothing joins them but a name would, and a name
 * joins nothing.
 */
function twoIslands(names = ["P0", "R0", "P10", "R10"]) {
  const sketch = new Sketch();
  const first = sketch.place("dc_voltage_source", p(0, 0), 0, {
    voltageVolts: 5,
    internalResistanceOhms: 0,
  });
  sketch.place("resistor", p(0, 10), 0, { resistanceOhms: 1000 });
  const second = sketch.place("dc_voltage_source", p(10, 0), 0, {
    voltageVolts: 10,
    internalResistanceOhms: 0,
  });
  sketch.place("resistor", p(10, 10), 0, { resistanceOhms: 1000 });

  sketch.rail(names[0]!, sketch.pin(first, "pin1").x, 0, 10);
  sketch.rail(names[1]!, sketch.pin(first, "pin2").x, 0, 10);
  sketch.rail(names[2]!, sketch.pin(second, "pin1").x, 0, 10);
  sketch.rail(names[3]!, sketch.pin(second, "pin2").x, 0, 10);

  return sketch;
}

/** A schematic without any object ID, so a drawing compares with a fixture by what it draws. */
function withoutIds(schematic: Schematic): unknown {
  return JSON.parse(
    JSON.stringify(schematic, (key, value) =>
      key === "id" ? undefined : value,
    ),
  );
}

const fixture = (id: string) =>
  circuits.circuits.find((item) => item.id === id)!
    .railSchematic as unknown as Schematic;

describe("the START/STOP circuit drawn between rails from a blank document", () => {
  for (const [name, fixtureId, draw] of [
    ["start-stop", "start-stop", () => startStop(true)],
    ["start-stop-no-seal-in", "start-stop-no-seal-in", () => startStop(false)],
    ["divider", "divider", () => divider()],
    ["rc-hold", "rc-hold", () => rcHold()],
    ["two-islands", "two-islands", () => twoIslands()],
    [
      "two-islands-N-PE",
      "two-islands-N-PE",
      () => twoIslands(["N", "PE", "L1", "L2"]),
    ],
  ] as const) {
    it(`draws ${name} as the independent fixture does and pins its compile request`, () => {
      const sketch = draw();
      const request = sketch.request();

      // What the editor commands built is, object for object, the fixture's drawing.
      expect(withoutIds(request.schematic)).toEqual(
        withoutIds(fixture(fixtureId)),
      );
      expect(request.schematic.rails.map((rail) => rail.name)).toEqual(
        fixture(fixtureId).rails.map((rail) => rail.name),
      );
      // Every step was one history entry, and none of them is an operator state.
      expect(sketch.store.getState().canUndo).toBe(true);
      expect(JSON.stringify(request.schematic)).not.toMatch(
        /pressed|picked|conducting|operator/,
      );

      const path = fileURLToPath(
        new URL(
          `../../tests/golden/stage-8-drawn-${name}.request.json`,
          import.meta.url,
        ),
      );
      const text = `${JSON.stringify(request, null, 2)}\n`;
      if (process.env.UPDATE_GOLDEN) writeFileSync(path, text);
      expect(readFileSync(path, "utf8")).toBe(text);
    });
  }

  it("gives the drawing the same component IDs as the fixture, so its observations apply", () => {
    const drawn = startStop(true).request().schematic;
    const expected = fixture("start-stop");

    expect(drawn.components.map((item) => item.id)).toEqual(
      expected.components.map((item) => item.id),
    );
  });

  it("undoes the whole drawing one step at a time, rails included", () => {
    const sketch = startStop(true);
    let steps = 0;

    while (sketch.store.undo()) steps += 1;

    // 4 parts and 4 parameter edits of the source and contactor are separate steps too.
    expect(steps).toBeGreaterThan(10);
    expect(sketch.store.getState().schematic.rails).toEqual([]);
    expect(sketch.store.getState().schematic.components).toEqual([]);
  });
});
