import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import catalogFixture from "../../tests/golden/stage-1-catalog.json";
import type { EditorStatus } from "../editor/interaction";
import type { ComponentCatalog } from "../generated/ComponentCatalog";
import { symbols } from "../symbols";
import { Palette, toolHint } from "./Palette";

const catalog = catalogFixture as ComponentCatalog;
const placeableTypes = catalog.componentTypes.filter(
  (entry) => symbols[entry.typeId],
);
const noTool = () => {};

function pressed(markup: string): string[] {
  return [...markup.matchAll(/aria-pressed="true"[^>]*>([^<]+)</g)].map(
    (match) => match[1] ?? "",
  );
}

describe("component palette", () => {
  it("offers Select, supported catalog types, and the drawing tools", () => {
    const markup = renderToStaticMarkup(
      <Palette
        catalog={catalog}
        tool={{ kind: "select" }}
        onSelectTool={noTool}
      />,
    );

    for (const label of [
      "Select",
      "Resistor",
      "DC voltage source",
      "Wire",
      "Junction",
      "Ground",
    ]) {
      expect(markup).toContain(`>${label}<`);
    }
    expect(markup.match(/<button/g)).toHaveLength(placeableTypes.length + 4);
    expect(markup).not.toContain("Capacitor");
    expect(markup).not.toContain("Inductor");
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

  it("disables every editor tool while a project operation is pending", () => {
    const markup = renderToStaticMarkup(
      <Palette
        catalog={catalog}
        tool={{ kind: "select" }}
        disabled
        onSelectTool={noTool}
      />,
    );

    expect(markup.match(/disabled=""/g)).toHaveLength(
      placeableTypes.length + 4,
    );
  });

  it.each([
    [{ kind: "wire" }, "Wire"],
    [{ kind: "junction" }, "Junction"],
    [{ kind: "reference" }, "Ground"],
  ] as const)("marks the %o tool", (tool, label) => {
    const markup = renderToStaticMarkup(
      <Palette catalog={catalog} tool={tool} onSelectTool={noTool} />,
    );

    expect(pressed(markup)).toEqual([label]);
  });

  it("offers the drawing tools before the catalog has loaded", () => {
    const markup = renderToStaticMarkup(
      <Palette
        catalog={null}
        tool={{ kind: "select" }}
        onSelectTool={noTool}
      />,
    );

    expect(markup.match(/<button/g)).toHaveLength(4);
    expect(markup).not.toContain("Resistor");
  });
});

describe("tool hint", () => {
  const status = (partial: Partial<EditorStatus>): EditorStatus => ({
    tool: { kind: "select" },
    routing: false,
    rejected: null,
    ...partial,
  });

  it("explains each tool", () => {
    const texts = [
      status({}),
      status({ tool: { kind: "place", typeId: "resistor", rotationDeg: 0 } }),
      status({ tool: { kind: "wire" } }),
      status({ tool: { kind: "junction" } }),
      status({ tool: { kind: "reference" } }),
    ].map((value) => toolHint(value).text);

    expect(new Set(texts).size).toBe(texts.length);
    expect(texts.every((text) => text.length > 0)).toBe(true);
  });

  it("describes how to finish a wire once a route has started", () => {
    const idle = toolHint(status({ tool: { kind: "wire" } }));
    const routing = toolHint(status({ tool: { kind: "wire" }, routing: true }));

    expect(idle.text).toContain("start");
    expect(routing.text).toContain("Enter");
    expect(routing.text).toContain("Shift");
    expect(routing.warning).toBe(false);
  });

  it("warns when a click was refused, and says why", () => {
    const junction = toolHint(
      status({ tool: { kind: "junction" }, rejected: "crossing" }),
    );
    const reference = toolHint(
      status({ tool: { kind: "reference" }, rejected: "attachment" }),
    );

    expect(junction).toMatchObject({ warning: true });
    expect(junction.text).toContain("crossing");
    expect(reference).toMatchObject({ warning: true });
    expect(reference.text).toContain("pin or a wire");
  });
});
