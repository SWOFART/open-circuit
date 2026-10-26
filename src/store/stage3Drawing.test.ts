import { readFileSync, writeFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";
import catalogFixture from "../../tests/golden/stage-1-catalog.json";
import type { EditorCommand } from "../editor/commands";
import { buildGeometryEnvelope } from "../editor/geometry";
import type { CompileRequest } from "../generated/CompileRequest";
import type { ComponentCatalog } from "../generated/ComponentCatalog";
import type { GridPoint } from "../generated/GridPoint";
import type { PinId } from "../generated/PinId";
import type { StableId } from "../generated/StableId";
import { symbols } from "../symbols";
import { applyEditorCommand } from "../ui/editorCommands";
import { SchematicDocumentStore } from "./document";
import { blankSchematic } from "./projectWorkflow";

// Issue #50: the RC charge circuit of docs/physics/models.md ("Stage 3 acceptance") is
// drawn here from a blank document with the editor's own commands, exactly as the canvas
// issues them, and the compile request that results is pinned in
// tests/golden/stage-3-drawn-rc.request.json. `crates/app/tests/stage3_session.rs`
// compiles that same file with the real Rust Engine and checks the traces against the
// analytical curve, so the drawn document and the Engine's answer are tied together.
//
// To regenerate the file after a deliberate change to the symbols or the editor, run
// `UPDATE_GOLDEN=1 pnpm exec vitest run src/store/stage3Drawing.test.ts`.

const catalog = catalogFixture as ComponentCatalog;
const goldenPath = fileURLToPath(
  new URL("../../tests/golden/stage-3-drawn-rc.request.json", import.meta.url),
);

function ids(): () => StableId {
  let next = 1;
  return () =>
    `00000000-0000-4000-8000-${(next++).toString(16).padStart(12, "0")}`;
}

function pinPosition(
  store: SchematicDocumentStore,
  componentId: StableId,
  pinId: PinId,
): GridPoint {
  const envelope = buildGeometryEnvelope(store.getState().schematic, symbols);
  const pin = envelope.components
    .find((component) => component.componentId === componentId)
    ?.pins.find((candidate) => candidate.pinId === pinId);

  if (!pin) {
    throw new Error(`No pin ${pinId} on ${componentId}.`);
  }

  return pin.position;
}

/** An orthogonal route from `from` to `to`, bending once when they are not aligned. */
function route(from: GridPoint, to: GridPoint): GridPoint[] {
  return from.x === to.x || from.y === to.y
    ? [from, to]
    : [from, { x: from.x, y: to.y }, to];
}

/** Draws the circuit. Returns the component IDs in the order they were placed. */
function drawRcCharge(store: SchematicDocumentStore) {
  const apply = (command: EditorCommand) =>
    applyEditorCommand(store, command, catalog);
  const placed = (
    typeId: "dc_voltage_source" | "resistor" | "capacitor",
    at: GridPoint,
    rotationDeg: 0 | 90 | 180 | 270,
  ) => {
    apply({ kind: "placeComponent", typeId, position: at, rotationDeg });
    const components = store.getState().schematic.components;

    return components[components.length - 1]!.id;
  };

  const source = placed("dc_voltage_source", { x: 0, y: 4 }, 90);
  const resistor = placed("resistor", { x: 4, y: 0 }, 0);
  const capacitor = placed("capacitor", { x: 8, y: 4 }, 90);

  // The inspector's edits: an ideal 10 V source, 1 kOhm, 10 uF, and an explicit 0 V start.
  store.setParameter(source, "voltageVolts", 10, catalog);
  store.setParameter(source, "internalResistanceOhms", 0, catalog);
  store.setParameter(resistor, "resistanceOhms", 1000, catalog);
  store.setParameter(capacitor, "capacitanceFarads", 1e-5, catalog);
  store.setParameter(capacitor, "initialVoltageVolts", 0, catalog);

  const at = (id: StableId, pin: PinId) => pinPosition(store, id, pin);

  // Source + (pin1) to R1, R1 to C1, C1 back to the source - (pin2).
  for (const [from, to] of [
    [at(source, "pin1"), at(resistor, "pin1")],
    [at(resistor, "pin2"), at(capacitor, "pin1")],
    [at(capacitor, "pin2"), at(source, "pin2")],
  ] as const) {
    apply({ kind: "addWire", points: route(from, to) });
  }

  // The reference marker on the ground node.
  apply({ kind: "addReferenceMarker", position: at(source, "pin2") });

  return { source, resistor, capacitor };
}

function compileRequestOf(store: SchematicDocumentStore): CompileRequest {
  const schematic = store.getState().schematic;

  return {
    schematic,
    geometry: buildGeometryEnvelope(schematic, symbols),
    documentRevision: "rev-1",
  };
}

describe("drawing the RC charge circuit from a blank document", () => {
  const store = new SchematicDocumentStore(blankSchematic, ids());
  const { source, resistor, capacitor } = drawRcCharge(store);

  it("places an ideal source, a resistor, and a capacitor with the documented values", () => {
    const { components } = store.getState().schematic;

    expect(components).toHaveLength(3);
    expect(components.find((item) => item.id === source)).toMatchObject({
      typeId: "dc_voltage_source",
      parameters: { voltageVolts: 10, internalResistanceOhms: 0 },
    });
    expect(components.find((item) => item.id === resistor)).toMatchObject({
      typeId: "resistor",
      parameters: { resistanceOhms: 1000 },
    });
    expect(components.find((item) => item.id === capacitor)).toMatchObject({
      typeId: "capacitor",
      parameters: { capacitanceFarads: 1e-5, initialVoltageVolts: 0 },
    });
  });

  it("closes the loop: every pin is the end of exactly one wire, and one marker grounds the negative pin", () => {
    const { schematic } = store.getState();
    const endpoints = schematic.wires.flatMap((wire) => [
      wire.points[0]!,
      wire.points[wire.points.length - 1]!,
    ]);
    const key = (point: GridPoint) => `${point.x},${point.y}`;

    for (const id of [source, resistor, capacitor]) {
      for (const pin of ["pin1", "pin2"] as const) {
        const position = pinPosition(store, id, pin);

        expect(
          endpoints.filter((point) => key(point) === key(position)),
        ).toHaveLength(1);
      }
    }
    expect(schematic.referenceMarkers).toHaveLength(1);
    expect(schematic.referenceMarkers[0]?.position).toEqual(
      pinPosition(store, source, "pin2"),
    );
  });

  it("lets no wire run through a pin it does not end at", () => {
    const { schematic } = store.getState();
    const onSegment = (point: GridPoint, a: GridPoint, b: GridPoint) =>
      point.x >= Math.min(a.x, b.x) &&
      point.x <= Math.max(a.x, b.x) &&
      point.y >= Math.min(a.y, b.y) &&
      point.y <= Math.max(a.y, b.y);

    for (const id of [source, resistor, capacitor]) {
      for (const pin of ["pin1", "pin2"] as const) {
        const position = pinPosition(store, id, pin);
        const touching = schematic.wires.filter((wire) =>
          wire.points.some(
            (point, index) =>
              index > 0 && onSegment(position, wire.points[index - 1]!, point),
          ),
        );

        // Exactly the wire that ends at the pin touches it: a wire passing through it
        // would connect two nets that the drawing keeps apart.
        expect(touching).toHaveLength(1);
      }
    }
  });

  it("is the compile request pinned for the real Engine", () => {
    const request = compileRequestOf(store);

    if (process.env.UPDATE_GOLDEN) {
      writeFileSync(goldenPath, `${JSON.stringify(request, null, 2)}\n`);
    }

    expect(request).toEqual(JSON.parse(readFileSync(goldenPath, "utf8")));
  });
});

describe("drawing an incompatible combined initial state", () => {
  const conflictPath = fileURLToPath(
    new URL(
      "../../tests/golden/stage-3-drawn-rc-conflict.request.json",
      import.meta.url,
    ),
  );

  // A second capacitor placed on the first one's pins: the two are in parallel, and the
  // explicit 0 V and 5 V cannot both hold across the same two nodes.
  const store = new SchematicDocumentStore(blankSchematic, ids());
  const { capacitor } = drawRcCharge(store);
  applyEditorCommand(
    store,
    {
      kind: "placeComponent",
      typeId: "capacitor",
      position: { x: 8, y: 4 },
      rotationDeg: 90,
    },
    catalog,
  );
  const parallel = store.getState().schematic.components.at(-1)!.id;
  store.setParameter(parallel, "initialVoltageVolts", 5, catalog);

  it("puts the second capacitor on the same two pins as the first", () => {
    for (const pin of ["pin1", "pin2"] as const) {
      expect(pinPosition(store, parallel, pin)).toEqual(
        pinPosition(store, capacitor, pin),
      );
    }
  });

  it("keeps both explicit voltages in the document", () => {
    const voltages = store
      .getState()
      .schematic.components.filter((item) => item.typeId === "capacitor")
      .map((item) =>
        item.typeId === "capacitor"
          ? item.parameters.initialVoltageVolts
          : undefined,
      );

    expect(voltages).toEqual([0, 5]);
  });

  it("is the compile request pinned for the real Engine", () => {
    const request = compileRequestOf(store);

    if (process.env.UPDATE_GOLDEN) {
      writeFileSync(conflictPath, `${JSON.stringify(request, null, 2)}\n`);
    }

    expect(request).toEqual(JSON.parse(readFileSync(conflictPath, "utf8")));
  });
});
