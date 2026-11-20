import { describe, expect, it } from "vitest";
import type { ComponentSpec } from "../generated/ComponentSpec";
import type { Schematic } from "../generated/Schematic";
import { EditorCore } from "./EditorCore";
import type { EditorStatus } from "./interaction";
import { emptyDocumentation } from "./documentation";

const FIRST = "00000000-0000-4000-8000-0000000000c1";
const ADDED = "00000000-0000-4000-8000-0000000000c2";

function resistor(id: string, x: number): ComponentSpec {
  return {
    typeId: "resistor",
    id,
    position: { x, y: 0 },
    rotationDeg: 0,
    parameters: { resistanceOhms: 1000 },
  } as unknown as ComponentSpec;
}

function drawing(...components: ComponentSpec[]): Schematic {
  return {
    schemaVersion: 3,
    components,
    wires: [],
    junctions: [],
    referenceMarkers: [],
    rails: [],
    documentation: emptyDocumentation(),
  };
}

function editor() {
  const statuses: EditorStatus[] = [];
  const core = new EditorCore({
    onCommand: () => {},
    onStatusChange: (status) => statuses.push(status),
  });
  return { core, selection: () => statuses.at(-1)?.selection ?? null };
}

describe("selecting an object the scene does not contain yet", () => {
  it("completes the selection when the next scene contains the object", () => {
    const { core, selection } = editor();
    core.setSchematic(drawing(resistor(FIRST, 0)));

    // The drawer places a part and selects it in the same turn, before the scene is rebuilt.
    core.select({ kind: "component", id: ADDED });
    expect(selection()).toBeNull();

    core.setSchematic(drawing(resistor(FIRST, 0), resistor(ADDED, 10)));
    expect(selection()).toEqual({ kind: "component", id: ADDED });
  });

  it("selects at once an object the scene already contains", () => {
    const { core, selection } = editor();
    core.setSchematic(drawing(resistor(FIRST, 0)));

    core.select({ kind: "component", id: FIRST });

    expect(selection()).toEqual({ kind: "component", id: FIRST });
  });

  it("applies a pending selection only once, and never for an object that did not appear", () => {
    const { core, selection } = editor();
    core.setSchematic(drawing(resistor(FIRST, 0)));
    core.select({ kind: "component", id: ADDED });

    core.setSchematic(drawing(resistor(FIRST, 0)));
    core.setSchematic(drawing(resistor(FIRST, 0), resistor(ADDED, 10)));

    expect(selection()).toBeNull();
  });

  it("forgets a pending selection when the selection is cleared", () => {
    const { core, selection } = editor();
    core.setSchematic(drawing(resistor(FIRST, 0)));
    core.select({ kind: "component", id: ADDED });
    core.select(null);

    core.setSchematic(drawing(resistor(FIRST, 0), resistor(ADDED, 10)));

    expect(selection()).toBeNull();
  });
});
