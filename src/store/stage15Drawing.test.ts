import { expect, it } from "vitest";
import catalogFixture from "../../tests/golden/stage-1-catalog.json";
import expectedGeometry from "../../tests/golden/stage-1-divider.geometry.json";
import drawnFixture from "../../tests/golden/stage-1-divider.schematic.json";
import assignmentText from "../../tests/golden/stage-15-divider-scored.ocassignment?raw";
import { buildGeometryEnvelope } from "../editor/geometry";
import type { Assignment } from "../generated/Assignment";
import type { ComponentCatalog } from "../generated/ComponentCatalog";
import type { Schematic } from "../generated/Schematic";
import { symbols } from "../symbols";
import { applyEditorCommand } from "../ui/editorCommands";
import { SchematicDocumentStore } from "./document";
import { blankSchematic } from "./projectWorkflow";

it("draws the scored divider using the editor commands and resolves its probe points", () => {
  let nextId = 1;
  const document = new SchematicDocumentStore(
    blankSchematic,
    () =>
      `00000000-0000-4000-8000-${(nextId++).toString(16).padStart(12, "0")}`,
  );
  const drawing = drawnFixture as Schematic;
  const catalog = catalogFixture as ComponentCatalog;

  for (const part of drawing.components) {
    applyEditorCommand(
      document,
      {
        kind: "placeComponent",
        typeId: part.typeId,
        position: part.position,
        rotationDeg: part.rotationDeg,
      },
      catalog,
    );
  }
  for (const { points } of drawing.wires) {
    applyEditorCommand(document, { kind: "addWire", points }, catalog);
  }
  for (const { position } of drawing.referenceMarkers) {
    applyEditorCommand(
      document,
      { kind: "addReferenceMarker", position },
      catalog,
    );
  }

  const schematic = document.getState().schematic;
  expect(schematic).toEqual(drawing);
  const geometry = buildGeometryEnvelope(schematic, symbols);
  expect(geometry).toEqual(expectedGeometry);
  const assignment = JSON.parse(assignmentText) as Assignment;
  expect(assignment.startingCircuit).toEqual({
    schemaVersion: 1,
    components: schematic.components,
    wires: schematic.wires,
    junctions: schematic.junctions,
    referenceMarkers: schematic.referenceMarkers,
  });
});
