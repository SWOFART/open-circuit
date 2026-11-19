import { readFileSync, writeFileSync } from "node:fs";
import { it, expect } from "vitest";
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

const catalog = fixture as ComponentCatalog;
const p = (x: number, y: number): GridPoint => ({ x, y });

class Drawing {
  private next = 1;
  private readonly store = new SchematicDocumentStore(
    blankSchematic,
    () =>
      `00000000-0000-4000-8000-${(this.next++).toString(16).padStart(12, "0")}`,
  );

  private apply(command: EditorCommand) {
    applyEditorCommand(this.store, command, catalog);
  }

  place(
    typeId: ComponentTypeId,
    position: GridPoint,
    parameters: Partial<Record<ParameterId, number>>,
    rotationDeg: 0 | 90 = 0,
  ) {
    this.apply({ kind: "placeComponent", typeId, position, rotationDeg });
    const component = this.store.getState().schematic.components.at(-1)!;
    expect(component.typeId).toBe(typeId);
    for (const [key, value] of Object.entries(parameters)) {
      this.store.setParameter(component.id, key as ParameterId, value, catalog);
    }
    return component.id;
  }

  pin(componentId: StableId, pin: PinId) {
    const component = this.store
      .getState()
      .schematic.components.find((item) => item.id === componentId)!;
    return resolvePinPositions(component, symbols).find(
      (item) => item.pinId === pin,
    )!.position;
  }

  wire(...points: GridPoint[]) {
    this.apply({ kind: "addWire", points });
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

const acParameters = {
  voltageRmsVolts: 10,
  frequencyHz: 50,
  phaseOffsetDegrees: 0,
  internalResistanceOhms: 0.1,
} as const;

function rc() {
  const drawing = new Drawing();
  const source = drawing.place("ac_voltage_source", p(0, 4), acParameters, 90);
  const resistor = drawing.place("resistor", p(4, 0), { resistanceOhms: 100 });
  const capacitor = drawing.place(
    "capacitor",
    p(8, 4),
    { capacitanceFarads: 0.0001 },
    90,
  );
  drawing.wire(
    drawing.pin(source, "pin1"),
    p(0, 0),
    drawing.pin(resistor, "pin1"),
  );
  drawing.wire(
    drawing.pin(resistor, "pin2"),
    p(8, 0),
    drawing.pin(capacitor, "pin1"),
  );
  drawing.wire(
    drawing.pin(capacitor, "pin2"),
    p(8, 8),
    p(0, 8),
    drawing.pin(source, "pin2"),
  );
  drawing.reference(drawing.pin(source, "pin2"));
  return drawing.request();
}

function wattmeter() {
  const drawing = new Drawing();
  const source = drawing.place("ac_voltage_source", p(0, 5), acParameters, 90);
  const meter = drawing.place("wattmeter", p(5, 2), {
    inputResistanceOhms: 10_000_000,
    burdenResistanceOhms: 0.1,
    frequencyHz: 50,
  });
  const resistor = drawing.place("resistor", p(11, 3), { resistanceOhms: 100 });
  const inductor = drawing.place(
    "inductor",
    p(15, 6),
    { inductanceHenries: 0.1 },
    90,
  );
  const ground = drawing.pin(source, "pin2");
  drawing.wire(drawing.pin(source, "pin1"), p(0, 3), drawing.pin(meter, "i1"));
  drawing.wire(drawing.pin(meter, "i2"), drawing.pin(resistor, "pin1"));
  drawing.wire(drawing.pin(meter, "v1"), p(3, 0), p(8, 0), p(8, 3));
  drawing.wire(
    drawing.pin(resistor, "pin2"),
    p(15, 3),
    drawing.pin(inductor, "pin1"),
  );
  drawing.wire(drawing.pin(inductor, "pin2"), p(15, 9), p(0, 9), ground);
  drawing.wire(drawing.pin(meter, "v2"), p(9, 1), p(9, 9));
  drawing.reference(ground);
  return drawing.request();
}

function star(resistances: readonly number[], connectedNeutral: boolean) {
  const drawing = new Drawing();
  const source = drawing.place("three_phase_source_star", p(0, 0), {
    phaseVoltageRmsVolts: 230,
    frequencyHz: 50,
    phaseOffsetDegrees: 0,
    internalResistanceOhms: 0.1,
    reversedSequence: 0,
  });
  const pinIds = ["l1", "l2", "l3"] as const;
  for (const [index, resistanceOhms] of resistances.entries()) {
    const y = 2 * index - 2;
    const resistor = drawing.place("resistor", p(6, y), { resistanceOhms });
    drawing.wire(
      drawing.pin(source, pinIds[index]!),
      drawing.pin(resistor, "pin1"),
    );
    drawing.wire(drawing.pin(resistor, "pin2"), p(10, y));
  }
  drawing.wire(p(10, -2), p(10, 0), p(10, 2));
  const neutral = drawing.pin(source, "n");
  if (connectedNeutral) {
    drawing.wire(neutral, p(-4, 0), p(-4, 5), p(10, 5), p(10, 2));
  }
  drawing.reference(neutral);
  return drawing.request();
}

function delta() {
  const drawing = new Drawing();
  const source = drawing.place("three_phase_source_delta", p(0, 0), {
    lineVoltageRmsVolts: 400,
    frequencyHz: 50,
    phaseOffsetDegrees: 0,
    internalResistanceOhms: 0.1,
    reversedSequence: 0,
  });
  const r12 = drawing.place("resistor", p(9, -5), { resistanceOhms: 100 });
  const r23 = drawing.place("resistor", p(9, 0), { resistanceOhms: 100 });
  const r31 = drawing.place("resistor", p(9, 5), { resistanceOhms: 100 });
  drawing.wire(
    drawing.pin(source, "l1"),
    p(4, -2),
    p(4, -5),
    drawing.pin(r12, "pin1"),
  );
  drawing.wire(drawing.pin(source, "l2"), drawing.pin(r23, "pin1"));
  drawing.wire(
    drawing.pin(source, "l3"),
    p(4, 2),
    p(4, 5),
    drawing.pin(r31, "pin1"),
  );
  drawing.wire(
    p(6, 0),
    p(6, -7),
    p(12, -7),
    p(12, -5),
    drawing.pin(r12, "pin2"),
  );
  drawing.wire(p(5, 5), p(5, 1), p(13, 1), p(13, 0), drawing.pin(r23, "pin2"));
  drawing.wire(
    p(4, -4),
    p(3, -4),
    p(3, 7),
    p(12, 7),
    p(12, 5),
    drawing.pin(r31, "pin2"),
  );
  drawing.reference(drawing.pin(source, "l3"));
  return drawing.request();
}

it.each([
  ["rc-50-phase-0", rc],
  ["wattmeter-rl", wattmeter],
  ["star-balanced-resistive-neutral", () => star([100, 100, 100], true)],
  ["star-unbalanced-neutral", () => star([100, 220, 470], true)],
  ["star-unbalanced-no-neutral", () => star([100, 220, 470], false)],
  ["delta-load-on-delta-source-balanced", delta],
] as const)("draws and pins %s from a blank document", (name, draw) => {
  const path = new URL(
    `../../tests/golden/stage-9-drawn-${name}.request.json`,
    import.meta.url,
  );
  const json = `${JSON.stringify(draw(), null, 2)}\n`;
  if (process.env.UPDATE_GOLDEN === "1") writeFileSync(path, json);
  expect(readFileSync(path, "utf8")).toBe(json);
});
