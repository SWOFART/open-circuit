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

// Issue #100: editor commands and actual symbol geometry produce the inputs consumed by
// crates/app/tests/stage6_acceptance.rs. Regenerate deliberately with UPDATE_GOLDEN=1.
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
    rotationDeg: 0 | 90 = 0,
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

function series(capacitor: boolean) {
  const sketch = new Sketch();
  const source = sketch.place("dc_voltage_source", p(0, 5), 90, {
    voltageVolts: 10,
    internalResistanceOhms: 0,
  });
  const contact = sketch.place(
    capacitor ? "push_button_nc" : "switch_spst",
    p(4, 0),
  );
  const resistor = sketch.place("resistor", p(8, 0), 0, {
    resistanceOhms: 1000,
  });
  const load = sketch.place(
    capacitor ? "capacitor" : "resistor",
    p(12, 5),
    90,
    capacitor
      ? { capacitanceFarads: 0.0001, initialVoltageVolts: 0 }
      : { resistanceOhms: 1000 },
  );
  sketch.wire(sketch.pin(source, "pin1"), p(0, 0), sketch.pin(contact, "pin1"));
  sketch.wire(sketch.pin(contact, "pin2"), sketch.pin(resistor, "pin1"));
  sketch.wire(sketch.pin(resistor, "pin2"), p(12, 0), sketch.pin(load, "pin1"));
  sketch.wire(
    sketch.pin(load, "pin2"),
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

function startStop(sealIn: boolean) {
  const sketch = new Sketch();
  const source = sketch.place("dc_voltage_source", p(0, 5), 90, {
    voltageVolts: 5,
    internalResistanceOhms: 0,
  });
  const stop = sketch.place("push_button_nc", p(4, 0));
  const start = sketch.place("push_button_no", p(8, 0));
  const contactor = sketch.place("contactor", p(16, 5));
  sketch.wire(sketch.pin(source, "pin1"), p(0, 0), sketch.pin(stop, "pin1"));
  sketch.wire(sketch.pin(stop, "pin2"), sketch.pin(start, "pin1"));
  sketch.wire(sketch.pin(start, "pin2"), p(14, 0), sketch.pin(contactor, "a1"));
  sketch.wire(
    sketch.pin(contactor, "a2"),
    p(14, 10),
    p(0, 10),
    sketch.pin(source, "pin2"),
  );
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
  return sketch;
}

describe("Stage 6 canonical circuits drawn from blank documents", () => {
  for (const [name, draw] of [
    ["switch", () => series(false)],
    ["button", () => series(true)],
    ["latching", () => startStop(true)],
    ["unlatched", () => startStop(false)],
  ] as const) {
    it(`pins the ${name} compile request for the real Engine`, () => {
      const sketch = draw();
      const request = sketch.request();
      expect(request.schematic.components).toHaveLength(4);
      expect(request.schematic.wires.length).toBeGreaterThanOrEqual(4);
      expect(sketch.store.getState().canUndo).toBe(true);
      const path = fileURLToPath(
        new URL(
          `../../tests/golden/stage-6-drawn-${name}.request.json`,
          import.meta.url,
        ),
      );
      const text = `${JSON.stringify(request, null, 2)}\n`;
      if (process.env.UPDATE_GOLDEN) writeFileSync(path, text);
      expect(readFileSync(path, "utf8")).toBe(text);
    });
  }
});
