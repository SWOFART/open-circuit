import { readFileSync, writeFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";
import catalogFixture from "../../tests/golden/stage-1-catalog.json";
import type { EditorCommand } from "../editor/commands";
import { buildGeometryEnvelope } from "../editor/geometry";
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

// Issue #69: the circuits of the Stage 4 fixtures (docs/contracts/stage-4-components.md
// section 6) are drawn here from a blank document with the editor's own commands, as the
// canvas issues them and the inspector edits them. The compile request of each is pinned in
// tests/golden/stage-4-drawn-*.request.json. `crates/app/tests/stage4_session.rs` compiles
// those files with the real Rust Engine and checks them against the fixtures' expected
// values, so the drawn document and the Engine's answer are tied together.
//
// To regenerate the files after a deliberate change to the symbols or the editor, run
// `UPDATE_GOLDEN=1 pnpm exec vitest run src/store/stage4Drawing.test.ts`.

const catalog = catalogFixture as ComponentCatalog;

function goldenPath(name: string): string {
  return fileURLToPath(
    new URL(
      `../../tests/golden/stage-4-drawn-${name}.request.json`,
      import.meta.url,
    ),
  );
}

function ids(): () => StableId {
  let next = 1;
  return () =>
    `00000000-0000-4000-8000-${(next++).toString(16).padStart(12, "0")}`;
}

type Rotation = 0 | 90 | 180 | 270;

/** A document being drawn: placing parts, editing parameters, and wiring pins. */
class Sketch {
  readonly store = new SchematicDocumentStore(blankSchematic, ids());

  private apply(command: EditorCommand): void {
    applyEditorCommand(this.store, command, catalog);
  }

  place(
    typeId: ComponentTypeId,
    position: GridPoint,
    rotationDeg: Rotation,
    edits: Readonly<Record<string, number>> = {},
  ): StableId {
    this.apply({ kind: "placeComponent", typeId, position, rotationDeg });
    const id = this.store.getState().schematic.components.at(-1)!.id;

    for (const [parameter, value] of Object.entries(edits)) {
      this.store.setParameter(id, parameter as ParameterId, value, catalog);
    }

    return id;
  }

  pin(componentId: StableId, pinId: PinId): GridPoint {
    const envelope = buildGeometryEnvelope(
      this.store.getState().schematic,
      symbols,
    );
    const pin = envelope.components
      .find((component) => component.componentId === componentId)
      ?.pins.find((candidate) => candidate.pinId === pinId);

    if (!pin) {
      throw new Error(`No pin ${pinId} on ${componentId}.`);
    }

    return pin.position;
  }

  wire(...points: GridPoint[]): void {
    this.apply({ kind: "addWire", points });
  }

  ground(position: GridPoint): void {
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

const p = (x: number, y: number): GridPoint => ({ x, y });

/** Ideal 5 V source, a 1 ohm start path, and the relay whose NO contact holds the coil. */
function relayCircuit() {
  const sketch = new Sketch();
  const source = sketch.place("dc_voltage_source", p(0, 8), 90, {
    voltageVolts: 5,
    internalResistanceOhms: 0,
  });
  const relay = sketch.place("relay_spdt", p(8, 4), 0);
  const start = sketch.place("resistor", p(14, 8), 90, { resistanceOhms: 1 });
  const at = (id: StableId, pin: PinId) => sketch.pin(id, pin);

  sketch.wire(at(source, "pin1"), p(0, 3), at(relay, "coil_positive"));
  sketch.wire(at(relay, "coil_negative"), p(5, 4), at(relay, "common"));
  sketch.wire(at(relay, "common"), p(9, 7), at(start, "pin1"));
  sketch.wire(at(source, "pin2"), at(start, "pin2"));
  sketch.wire(
    at(relay, "normally_open"),
    p(16, 3),
    p(16, 9),
    at(start, "pin2"),
  );
  sketch.ground(at(source, "pin2"));

  return { sketch, source, relay, start };
}

/** Ideal 2.02 V source, a fuse, and a 1 ohm load in series. */
function fuseCircuit() {
  const sketch = new Sketch();
  const source = sketch.place("dc_voltage_source", p(0, 4), 90, {
    voltageVolts: 2.02,
    internalResistanceOhms: 0,
  });
  const fuse = sketch.place("fuse", p(4, 0), 0);
  const load = sketch.place("resistor", p(8, 4), 90, { resistanceOhms: 1 });
  const at = (id: StableId, pin: PinId) => sketch.pin(id, pin);

  sketch.wire(at(source, "pin1"), p(0, 0), at(fuse, "pin1"));
  sketch.wire(at(fuse, "pin2"), p(5, 3), at(load, "pin1"));
  sketch.wire(at(load, "pin2"), at(source, "pin2"));
  sketch.ground(at(source, "pin2"));

  return { sketch, source, fuse, load };
}

/** Ideal 10 V source across the motor, which starts from rest and zero current. */
function motorCircuit() {
  const sketch = new Sketch();
  const source = sketch.place("dc_voltage_source", p(0, 4), 90, {
    voltageVolts: 10,
    internalResistanceOhms: 0,
  });
  const motor = sketch.place("pmdc_motor", p(6, 0), 0, {
    initialAngularVelocityRadiansPerSecond: 0,
    initialArmatureCurrentAmps: 0,
  });
  const at = (id: StableId, pin: PinId) => sketch.pin(id, pin);

  sketch.wire(at(source, "pin1"), p(0, 0), at(motor, "pin1"));
  sketch.wire(at(motor, "pin2"), p(7, 5), at(source, "pin2"));
  sketch.ground(at(source, "pin2"));

  return { sketch, source, motor };
}

/** A 10 V source with 1 Mohm internal resistance, and a 10 Mohm voltmeter across it. */
function voltmeterCircuit() {
  const sketch = new Sketch();
  const source = sketch.place("dc_voltage_source", p(0, 4), 90, {
    voltageVolts: 10,
    internalResistanceOhms: 1e6,
  });
  const meter = sketch.place("voltmeter", p(6, 0), 0);
  const at = (id: StableId, pin: PinId) => sketch.pin(id, pin);

  sketch.wire(at(source, "pin1"), p(0, 0), at(meter, "pin1"));
  sketch.wire(at(meter, "pin2"), p(7, 5), at(source, "pin2"));
  sketch.ground(at(source, "pin2"));

  return { sketch, source, meter };
}

/** An ideal 10 V source, a 10 ohm load, and a 1 ohm ammeter in series. */
function ammeterCircuit() {
  const sketch = new Sketch();
  const source = sketch.place("dc_voltage_source", p(0, 4), 90, {
    voltageVolts: 10,
    internalResistanceOhms: 0,
  });
  const meter = sketch.place("ammeter", p(4, 0), 0, {
    burdenResistanceOhms: 1,
  });
  const load = sketch.place("resistor", p(8, 4), 90, { resistanceOhms: 10 });
  const at = (id: StableId, pin: PinId) => sketch.pin(id, pin);

  sketch.wire(at(source, "pin1"), p(0, 0), at(meter, "pin1"));
  sketch.wire(at(meter, "pin2"), p(5, 3), at(load, "pin1"));
  sketch.wire(at(load, "pin2"), at(source, "pin2"));
  sketch.ground(at(source, "pin2"));

  return { sketch, source, meter, load };
}

/** Whether `point` lies on the segment from `a` to `b` (the wires are orthogonal). */
function onSegment(point: GridPoint, a: GridPoint, b: GridPoint): boolean {
  return (
    point.x >= Math.min(a.x, b.x) &&
    point.x <= Math.max(a.x, b.x) &&
    point.y >= Math.min(a.y, b.y) &&
    point.y <= Math.max(a.y, b.y)
  );
}

/** How many wires end at each pin, and any wire that passes through a pin it does not end at. */
function connections(sketch: Sketch, parts: readonly StableId[]) {
  const { schematic } = sketch.store.getState();
  const counts = new Map<string, number>();
  const passingThrough: string[] = [];

  for (const id of parts) {
    const component = schematic.components.find((item) => item.id === id)!;
    const entry = catalog.componentTypes.find(
      (item) => item.typeId === component.typeId,
    )!;

    for (const { id: pinId } of entry.pins) {
      const position = sketch.pin(id, pinId);
      const name = `${sketch.store.getState().labels.get(id)}.${pinId}`;
      let ends = 0;

      for (const wire of schematic.wires) {
        const first = wire.points[0]!;
        const last = wire.points[wire.points.length - 1]!;
        const endsHere =
          (first.x === position.x && first.y === position.y) ||
          (last.x === position.x && last.y === position.y);

        if (endsHere) {
          ends += 1;
        } else if (
          wire.points.some(
            (point, index) =>
              index > 0 && onSegment(position, wire.points[index - 1]!, point),
          )
        ) {
          passingThrough.push(`${name} under a wire`);
        }
      }

      counts.set(name, ends);
    }
  }

  return { counts: Object.fromEntries(counts), passingThrough };
}

function expectPinned(name: string, sketch: Sketch): void {
  const request = sketch.request();

  if (process.env.UPDATE_GOLDEN) {
    writeFileSync(goldenPath(name), `${JSON.stringify(request, null, 2)}\n`);
  }

  expect(request).toEqual(JSON.parse(readFileSync(goldenPath(name), "utf8")));
}

describe("drawing the self-latching relay circuit from a blank document", () => {
  const { sketch, source, relay, start } = relayCircuit();

  it("places an ideal 5 V source, a 1 ohm start path, and the relay with the catalog defaults", () => {
    const { components } = sketch.store.getState().schematic;

    expect(components.find((item) => item.id === source)).toMatchObject({
      parameters: { voltageVolts: 5, internalResistanceOhms: 0 },
    });
    expect(components.find((item) => item.id === start)).toMatchObject({
      parameters: { resistanceOhms: 1 },
    });
    expect(components.find((item) => item.id === relay)).toMatchObject({
      typeId: "relay_spdt",
      parameters: {
        coilResistanceOhms: 100,
        coilInductanceHenries: 0.01,
        pickupCurrentAmps: 0.04,
        dropoutCurrentAmps: 0.02,
        closedContactResistanceOhms: 0.1,
      },
    });
  });

  it("wires the coil to the source, the contact hold path to ground, and leaves the normally closed contact open", () => {
    const { counts, passingThrough } = connections(sketch, [
      source,
      relay,
      start,
    ]);

    expect(passingThrough).toEqual([]);
    expect(counts).toEqual({
      "V1.pin1": 1,
      "V1.pin2": 1,
      "K1.coil_positive": 1,
      "K1.coil_negative": 1,
      "K1.common": 2,
      "K1.normally_open": 1,
      "K1.normally_closed": 0,
      "R1.pin1": 1,
      "R1.pin2": 2,
    });
    expect(
      sketch.store.getState().schematic.referenceMarkers[0]?.position,
    ).toEqual(sketch.pin(source, "pin2"));
  });

  it("is the compile request pinned for the real Engine", () => {
    expectPinned("relay", sketch);
  });
});

describe("drawing the I2t fuse circuit from a blank document", () => {
  const { sketch, source, fuse, load } = fuseCircuit();

  it("places the source, the fuse with its catalog defaults, and the load", () => {
    const { components } = sketch.store.getState().schematic;

    expect(components.find((item) => item.id === fuse)).toMatchObject({
      typeId: "fuse",
      parameters: {
        intactResistanceOhms: 0.01,
        tripI2tAmpereSquaredSeconds: 0.004,
      },
    });
    expect(components.find((item) => item.id === source)).toMatchObject({
      parameters: { voltageVolts: 2.02, internalResistanceOhms: 0 },
    });
  });

  it("closes the loop with one wire at each pin", () => {
    const { counts, passingThrough } = connections(sketch, [
      source,
      fuse,
      load,
    ]);

    expect(passingThrough).toEqual([]);
    expect(Object.values(counts)).toEqual([1, 1, 1, 1, 1, 1]);
  });

  it("is the compile request pinned for the real Engine", () => {
    expectPinned("fuse", sketch);
  });
});

describe("drawing the motor startup circuit from a blank document", () => {
  const { sketch, source, motor } = motorCircuit();

  it("places the motor with the catalog defaults and explicit zero initial conditions", () => {
    const placed = sketch.store
      .getState()
      .schematic.components.find((item) => item.id === motor);

    expect(placed).toMatchObject({
      typeId: "pmdc_motor",
      parameters: {
        armatureResistanceOhms: 2,
        armatureInductanceHenries: 0.1,
        motorConstantNewtonMetersPerAmp: 0.1,
        rotorInertiaKilogramMetersSquared: 0.01,
        viscousFrictionNewtonMeterSecondsPerRadian: 0.01,
        loadTorqueNewtonMeters: 0,
        initialAngularVelocityRadiansPerSecond: 0,
        initialArmatureCurrentAmps: 0,
      },
    });
  });

  it("closes the loop with one wire at each pin", () => {
    const { counts, passingThrough } = connections(sketch, [source, motor]);

    expect(passingThrough).toEqual([]);
    expect(Object.values(counts)).toEqual([1, 1, 1, 1]);
  });

  it("is the compile request pinned for the real Engine", () => {
    expectPinned("motor", sketch);
  });
});

describe("drawing the meter circuits from a blank document", () => {
  it("wires the voltmeter across the source, with its catalog input resistance", () => {
    const { sketch, source, meter } = voltmeterCircuit();
    const { counts, passingThrough } = connections(sketch, [source, meter]);

    expect(passingThrough).toEqual([]);
    expect(Object.values(counts)).toEqual([1, 1, 1, 1]);
    expect(
      sketch.store
        .getState()
        .schematic.components.find((item) => item.id === meter),
    ).toMatchObject({ parameters: { inputResistanceOhms: 1e7 } });
    expectPinned("voltmeter", sketch);
  });

  it("wires the ammeter in series with the load, pin1 toward the source's positive pin", () => {
    const { sketch, source, meter, load } = ammeterCircuit();
    const { counts, passingThrough } = connections(sketch, [
      source,
      meter,
      load,
    ]);

    expect(passingThrough).toEqual([]);
    expect(Object.values(counts)).toEqual([1, 1, 1, 1, 1, 1]);
    // The source's positive pin and the ammeter's pin1 are joined by one wire.
    const joined = sketch.store
      .getState()
      .schematic.wires.find(
        (wire) =>
          JSON.stringify(wire.points[0]) ===
            JSON.stringify(sketch.pin(source, "pin1")) &&
          JSON.stringify(wire.points.at(-1)) ===
            JSON.stringify(sketch.pin(meter, "pin1")),
      );
    expect(joined).toBeDefined();
    expectPinned("ammeter", sketch);
  });
});
