import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import catalogFixture from "../../tests/golden/stage-1-catalog.json";
import type { ComponentCatalog } from "../generated/ComponentCatalog";
import { Palette } from "./Palette";

const catalog = catalogFixture as ComponentCatalog;
const noTool = () => {};

function pressed(markup: string): string[] {
  return [...markup.matchAll(/aria-pressed="true"[^>]*>([^<]+)</g)].map(
    (match) => match[1] ?? "",
  );
}

describe("component palette", () => {
  it("offers Select and every catalog type by its display name", () => {
    const markup = renderToStaticMarkup(
      <Palette
        catalog={catalog}
        tool={{ kind: "select" }}
        onSelectTool={noTool}
      />,
    );

    expect(markup).toContain(">Select<");
    expect(markup).toContain(">Resistor<");
    expect(markup).toContain(">DC voltage source<");
    expect(markup.match(/<button/g)).toHaveLength(
      catalog.componentTypes.length + 1,
    );
    expect(pressed(markup)).toEqual(["Select"]);
  });

  it("marks the active placement tool", () => {
    const markup = renderToStaticMarkup(
      <Palette
        catalog={catalog}
        tool={{ kind: "place", typeId: "dc_voltage_source", rotationDeg: 90 }}
        onSelectTool={noTool}
      />,
    );

    expect(pressed(markup)).toEqual(["DC voltage source"]);
  });

  it("offers only Select until the catalog has loaded", () => {
    const markup = renderToStaticMarkup(
      <Palette
        catalog={null}
        tool={{ kind: "select" }}
        onSelectTool={noTool}
      />,
    );

    expect(markup.match(/<button/g)).toHaveLength(1);
    expect(markup).not.toContain("Resistor");
  });
});
