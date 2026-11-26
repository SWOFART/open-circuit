import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import type { EditorStatus, EditorTool } from "../editor/interaction";
import { Palette, toolHint } from "./Palette";

const noTool = () => {};

function pressed(markup: string): string[] {
  return [...markup.matchAll(/aria-pressed="true"[^>]*>([^<]+)</g)].map(
    (match) => match[1] ?? "",
  );
}

function render(tool: EditorTool, drawerOpen = false, disabled = false) {
  return renderToStaticMarkup(
    <Palette
      tool={tool}
      drawerOpen={drawerOpen}
      onToggleDrawer={noTool}
      disabled={disabled}
      onSelectTool={noTool}
    />,
  );
}

describe("editor tools", () => {
  it("offers Select, Parts, DC rail and the drawing tools, and no part buttons", () => {
    const markup = render({ kind: "select" });

    for (const label of [
      "Select",
      "Parts",
      "DC rail",
      "Wire",
      "Junction",
      "Ground",
      "Note",
      "Terminal",
    ]) {
      expect(markup).toContain(`>${label}<`);
    }
    expect(markup.match(/<button/g)).toHaveLength(8);
    expect(markup).not.toContain("Resistor");
    expect(pressed(markup)).toEqual(["Select"]);
  });

  it("reports whether the drawer is open and what the button controls", () => {
    expect(render({ kind: "select" }, true)).toContain('aria-expanded="true"');
    expect(render({ kind: "select" }, false)).toContain(
      'aria-expanded="false"',
    );
    expect(render({ kind: "select" })).toContain(
      'aria-controls="parts-drawer"',
    );
  });

  it("disables every tool while a project operation is pending", () => {
    expect(
      render({ kind: "select" }, false, true).match(/disabled=""/g),
    ).toHaveLength(8);
  });

  it("keeps the Note tool visible but disabled, with the reason, when it is unavailable", () => {
    const markup = renderToStaticMarkup(
      <Palette
        tool={{ kind: "select" }}
        drawerOpen={false}
        onToggleDrawer={() => {}}
        noteUnavailable="Notes are not available in an assignment."
        onSelectTool={() => {}}
      />,
    );

    expect(markup).toMatch(
      /<button[^>]*disabled=""[^>]*title="Notes are not available in an assignment\."[^>]*>Note<\/button>/,
    );
  });

  it.each([
    [{ kind: "wire" }, "Wire"],
    [{ kind: "junction" }, "Junction"],
    [{ kind: "reference" }, "Ground"],
    [{ kind: "rail" }, "DC rail"],
    [{ kind: "note" }, "Note"],
  ] as const)("marks the %o tool", (tool, label) => {
    expect(pressed(render(tool))).toEqual([label]);
  });
});

describe("tool hint", () => {
  const status = (partial: Partial<EditorStatus>): EditorStatus => ({
    tool: { kind: "select" },
    routing: false,
    railStarted: false,
    rejected: null,
    selection: null,
    selections: [],
    ...partial,
  });

  it("explains each tool", () => {
    const texts = [
      status({}),
      status({ tool: { kind: "place", typeId: "resistor", rotationDeg: 0 } }),
      status({ tool: { kind: "wire" } }),
      status({ tool: { kind: "junction" } }),
      status({ tool: { kind: "reference" } }),
      status({ tool: { kind: "note" } }),
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
