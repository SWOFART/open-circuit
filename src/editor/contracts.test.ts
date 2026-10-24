import { describe, expect, it } from "vitest";
import geometryFixture from "../../tests/golden/stage-1-divider.geometry.json";
import schematicFixture from "../../tests/golden/stage-1-divider.schematic.json";
import type { ComponentSpec } from "../generated/ComponentSpec";
import type { GeometryEnvelope } from "../generated/GeometryEnvelope";
import type { Schematic } from "../generated/Schematic";

function isComponentTypeId(value: string): value is ComponentSpec["typeId"] {
  return value === "resistor" || value === "dc_voltage_source";
}

describe("generated Stage 1 contract types", () => {
  it("accept the canonical schematic and pin geometry fixtures", () => {
    const componentIds: Schematic["components"][number]["id"][] =
      schematicFixture.components.map((component) => component.id);
    const geometryComponentIds: GeometryEnvelope["components"][number]["componentId"][] =
      geometryFixture.components.map((component) => component.componentId);
    const componentTypeIds = schematicFixture.components
      .map((component) => component.typeId)
      .filter(isComponentTypeId);

    expect(schematicFixture.schemaVersion).toBe(1);
    expect(componentIds).toEqual([
      "00000000-0000-4000-8000-000000000001",
      "00000000-0000-4000-8000-000000000002",
      "00000000-0000-4000-8000-000000000003",
    ]);
    expect(componentTypeIds).toEqual([
      "dc_voltage_source",
      "resistor",
      "resistor",
    ]);
    expect(geometryComponentIds).toEqual(componentIds);
  });
});
