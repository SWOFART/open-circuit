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

// Issue #114: editor commands and the real symbol geometry produce the semiconductor circuits
// consumed by crates/app/tests/stage7_drawn.rs, which runs them on the real Engine and compares
// with the answers of tests/golden/stage-7-*. Regenerate deliberately with UPDATE_GOLDEN=1.
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

  request(): CompileRequest {
    const schematic = this.store.getState().schematic;
    return {
      schematic,
      geometry: buildGeometryEnvelope(schematic, symbols),
      documentRevision: "rev-1",
    };
  }
}

/** A source, a 1 kOhm resistor, and the default diode in series (the `diode-*` fixtures). */
function diodeLoop(sourceVolts: number) {
  const sketch = new Sketch();
  const source = sketch.place("dc_voltage_source", p(0, 5), 90, {
    voltageVolts: sourceVolts,
    internalResistanceOhms: 0,
  });
  const resistor = sketch.place("resistor", p(6, 0), 0, {
    resistanceOhms: 1000,
  });
  const diode = sketch.place("diode", p(12, 5), 90);
  sketch.wire(
    sketch.pin(source, "pin1"),
    p(0, 0),
    sketch.pin(resistor, "pin1"),
  );
  sketch.wire(
    sketch.pin(resistor, "pin2"),
    p(12, 0),
    sketch.pin(diode, "anode"),
  );
  sketch.wire(
    sketch.pin(diode, "cathode"),
    p(12, 10),
    p(0, 10),
    sketch.pin(source, "pin2"),
  );
  sketch.apply({
    kind: "addReferenceMarker",
    position: sketch.pin(source, "pin2"),
  });
  return sketch;
}

/**
 * The common-emitter stage of the `npn-*` and `pnp-*` fixtures: 1 kOhm from the supply to the
 * collector, a base source through a base resistor, the emitter at the reference. `sign` is -1
 * for the PNP, which has every source negated.
 */
function commonEmitter(
  typeId: "bjt_npn" | "bjt_pnp",
  sign: 1 | -1,
  { baseVolts = 5, baseOhms = 430_000 as number | null } = {},
) {
  const sketch = new Sketch();
  const supply = sketch.place("dc_voltage_source", p(0, 5), 90, {
    voltageVolts: 10 * sign,
    internalResistanceOhms: 0,
  });
  const bias = sketch.place("dc_voltage_source", p(0, 20), 90, {
    voltageVolts: baseVolts * sign,
    internalResistanceOhms: 0,
  });
  const collectorResistor = sketch.place("resistor", p(19, 6), 90, {
    resistanceOhms: 1000,
  });
  const baseResistor =
    baseOhms === null
      ? null
      : sketch.place("resistor", p(10, 12), 0, { resistanceOhms: baseOhms });
  const transistor = sketch.place(typeId, p(18, 12), 0);

  // Supply positive to the top of the collector resistor, over the top of the drawing.
  sketch.wire(
    sketch.pin(supply, "pin1"),
    p(0, -3),
    p(19, -3),
    sketch.pin(collectorResistor, "pin1"),
  );
  sketch.wire(
    sketch.pin(collectorResistor, "pin2"),
    sketch.pin(transistor, "collector"),
  );
  // Base source positive through the base resistor to the base.
  if (baseResistor === null) {
    // The `*-ideal-source` variant: a wire where the base resistor was.
    sketch.wire(
      sketch.pin(bias, "pin1"),
      p(16, 19),
      sketch.pin(transistor, "base"),
    );
  } else {
    sketch.wire(
      sketch.pin(bias, "pin1"),
      p(9, 19),
      sketch.pin(baseResistor, "pin1"),
    );
    sketch.wire(
      sketch.pin(baseResistor, "pin2"),
      sketch.pin(transistor, "base"),
    );
  }
  // The negative terminals and the emitter, all one node, at the bottom of the drawing.
  sketch.wire(
    sketch.pin(supply, "pin2"),
    p(-3, 6),
    p(-3, 21),
    sketch.pin(bias, "pin2"),
  );
  sketch.wire(
    sketch.pin(bias, "pin2"),
    p(0, 28),
    p(22, 28),
    p(22, 14),
    sketch.pin(transistor, "emitter"),
  );
  sketch.apply({
    kind: "addReferenceMarker",
    position: sketch.pin(bias, "pin2"),
  });
  return sketch;
}

/**
 * A source, a 1 kOhm resistor, a 10 H inductor and the default diode in series: the current
 * rises through the inductor and settles at the `diode-forward` answer.
 */
function diodeInductor() {
  const sketch = new Sketch();
  const source = sketch.place("dc_voltage_source", p(0, 5), 90, {
    voltageVolts: 5,
    internalResistanceOhms: 0,
  });
  const resistor = sketch.place("resistor", p(6, 0), 0, {
    resistanceOhms: 1000,
  });
  const inductor = sketch.place("inductor", p(12, 0), 0, {
    inductanceHenries: 10,
    initialCurrentAmps: 0,
  });
  const diode = sketch.place("diode", p(18, 5), 90);
  sketch.wire(
    sketch.pin(source, "pin1"),
    p(0, 0),
    sketch.pin(resistor, "pin1"),
  );
  sketch.wire(sketch.pin(resistor, "pin2"), sketch.pin(inductor, "pin1"));
  sketch.wire(
    sketch.pin(inductor, "pin2"),
    p(18, 0),
    sketch.pin(diode, "anode"),
  );
  sketch.wire(
    sketch.pin(diode, "cathode"),
    p(18, 10),
    p(0, 10),
    sketch.pin(source, "pin2"),
  );
  sketch.apply({
    kind: "addReferenceMarker",
    position: sketch.pin(source, "pin2"),
  });
  return sketch;
}

/** The ideal source straight across the diode: the `diode-ideal-source` failure fixture. */
function diodeAcrossSource() {
  const sketch = new Sketch();
  const source = sketch.place("dc_voltage_source", p(0, 5), 90, {
    voltageVolts: 5,
    internalResistanceOhms: 0,
  });
  const diode = sketch.place("diode", p(12, 5), 90);
  sketch.wire(
    sketch.pin(source, "pin1"),
    p(0, 0),
    p(12, 0),
    sketch.pin(diode, "anode"),
  );
  sketch.wire(
    sketch.pin(diode, "cathode"),
    p(12, 10),
    p(0, 10),
    sketch.pin(source, "pin2"),
  );
  sketch.apply({
    kind: "addReferenceMarker",
    position: sketch.pin(source, "pin2"),
  });
  return sketch;
}

/**
 * The wrong wiring of the NPN stage: the collector and the emitter exchanged, so the 1 kOhm
 * load hangs on the emitter and the collector goes to the reference (`npn-reverse-active`).
 * The transistor is turned half a turn, which puts the base on its right.
 */
function exchangedNpn() {
  const sketch = new Sketch();
  const supply = sketch.place("dc_voltage_source", p(0, 5), 90, {
    voltageVolts: 10,
    internalResistanceOhms: 0,
  });
  const bias = sketch.place("dc_voltage_source", p(30, 20), 90, {
    voltageVolts: 5,
    internalResistanceOhms: 0,
  });
  const load = sketch.place("resistor", p(17, 6), 90, { resistanceOhms: 1000 });
  const baseResistor = sketch.place("resistor", p(24, 12), 0, {
    resistanceOhms: 430_000,
  });
  const transistor = sketch.place("bjt_npn", p(18, 12), 180);

  sketch.wire(
    sketch.pin(supply, "pin1"),
    p(0, -3),
    p(17, -3),
    sketch.pin(load, "pin1"),
  );
  sketch.wire(sketch.pin(load, "pin2"), sketch.pin(transistor, "emitter"));
  sketch.wire(sketch.pin(transistor, "base"), sketch.pin(baseResistor, "pin1"));
  sketch.wire(
    sketch.pin(baseResistor, "pin2"),
    p(26, 12),
    p(26, 19),
    sketch.pin(bias, "pin1"),
  );
  // The collector, the base source's negative terminal and the supply's, one node.
  sketch.wire(
    sketch.pin(transistor, "collector"),
    p(17, 28),
    p(30, 28),
    sketch.pin(bias, "pin2"),
  );
  sketch.wire(
    sketch.pin(supply, "pin2"),
    p(-3, 6),
    p(-3, 31),
    p(33, 31),
    p(33, 21),
    sketch.pin(bias, "pin2"),
  );
  sketch.apply({
    kind: "addReferenceMarker",
    position: sketch.pin(bias, "pin2"),
  });
  return sketch;
}

/**
 * The switched nonlinear RC of the `rc-diode-*` fixtures: 5 V, a switch, 10 kOhm, the diode
 * and 1 uF from 0 V. Reversed, the diode's anode is on the capacitor side.
 */
function diodeRc(reversed: boolean) {
  const sketch = new Sketch();
  const source = sketch.place("dc_voltage_source", p(0, 5), 90, {
    voltageVolts: 5,
    internalResistanceOhms: 0,
  });
  const contact = sketch.place("switch_spst", p(4, 0));
  const resistor = sketch.place("resistor", p(9, 0), 0, {
    resistanceOhms: 10_000,
  });
  const diode = sketch.place("diode", p(13, 0), reversed ? 180 : 0);
  const capacitor = sketch.place("capacitor", p(18, 5), 90, {
    capacitanceFarads: 1e-6,
    initialVoltageVolts: 0,
  });
  sketch.wire(sketch.pin(source, "pin1"), p(0, 0), sketch.pin(contact, "pin1"));
  sketch.wire(sketch.pin(contact, "pin2"), sketch.pin(resistor, "pin1"));
  sketch.wire(
    sketch.pin(resistor, "pin2"),
    sketch.pin(diode, reversed ? "cathode" : "anode"),
  );
  sketch.wire(
    sketch.pin(diode, reversed ? "anode" : "cathode"),
    p(18, 0),
    sketch.pin(capacitor, "pin1"),
  );
  sketch.wire(
    sketch.pin(capacitor, "pin2"),
    p(18, 10),
    p(0, 10),
    sketch.pin(source, "pin2"),
  );
  sketch.apply({
    kind: "addReferenceMarker",
    position: sketch.pin(source, "pin2"),
  });
  return sketch;
}

describe("Stage 7 canonical circuits drawn from blank documents", () => {
  for (const [name, draw] of [
    ["diode-forward", () => diodeLoop(5)],
    ["diode-reverse", () => diodeLoop(-5)],
    ["diode-inductor", () => diodeInductor()],
    ["diode-ideal-source", () => diodeAcrossSource()],
    ["npn-cutoff", () => commonEmitter("bjt_npn", 1, { baseVolts: 0 })],
    ["npn-active", () => commonEmitter("bjt_npn", 1)],
    ["npn-saturation", () => commonEmitter("bjt_npn", 1, { baseOhms: 4300 })],
    ["npn-reverse-active", () => exchangedNpn()],
    ["npn-ideal-source", () => commonEmitter("bjt_npn", 1, { baseOhms: null })],
    ["pnp-cutoff", () => commonEmitter("bjt_pnp", -1, { baseVolts: 0 })],
    ["pnp-active", () => commonEmitter("bjt_pnp", -1)],
    ["pnp-saturation", () => commonEmitter("bjt_pnp", -1, { baseOhms: 4300 })],
    ["rc-diode-charging", () => diodeRc(false)],
    ["rc-diode-reversed", () => diodeRc(true)],
  ] as const) {
    it(`pins the ${name} compile request for the real Engine`, () => {
      const sketch = draw();
      const request = sketch.request();
      expect(request.schematic.components.length).toBeGreaterThanOrEqual(2);
      expect(request.schematic.wires.length).toBeGreaterThanOrEqual(2);
      expect(sketch.store.getState().canUndo).toBe(true);
      const path = fileURLToPath(
        new URL(
          `../../tests/golden/stage-7-drawn-${name}.request.json`,
          import.meta.url,
        ),
      );
      const text = `${JSON.stringify(request, null, 2)}\n`;
      if (process.env.UPDATE_GOLDEN) writeFileSync(path, text);
      expect(readFileSync(path, "utf8")).toBe(text);
    });
  }
});
