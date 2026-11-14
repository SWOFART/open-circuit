import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import type { Diagnostic } from "../generated/Diagnostic";
import type { Schematic } from "../generated/Schematic";
import { RailPanel, parseGridInteger } from "./RailPanel";

const A = "00000000-0000-4000-8000-000000009101";
const B = "00000000-0000-4000-8000-000000009102";

const rails: Schematic["rails"] = [
  { id: A, name: "L+", start: { x: 0, y: 0 }, end: { x: 0, y: 20 } },
  { id: B, name: "0V", start: { x: 12, y: 2 }, end: { x: 12, y: 22 } },
];

const noop = () => null;

function render(props: Partial<Parameters<typeof RailPanel>[0]> = {}): string {
  return renderToStaticMarkup(
    <RailPanel
      rails={rails}
      selectedId={null}
      diagnostics={[]}
      onSelect={() => {}}
      onCreate={noop}
      onEdit={noop}
      onDelete={() => {}}
      {...props}
    />,
  );
}

describe("the DC rail panel", () => {
  it("says what a rail is, and what it is not", () => {
    const html = render();

    expect(html).toContain(">DC rails<");
    expect(html).toContain("carries no voltage of its own");
    expect(html).toContain("L1, L2, L3, N or PE are plain labels");
    expect(html).toContain("no phase, neutral or");
    // No AC, three-phase or protective-earth controls.
    expect(html).not.toMatch(
      /phase selector|PE safety|AC source|three-phase preset/i,
    );
  });

  it("lists every rail as a button the keyboard can reach, and marks the selected one", () => {
    const html = render({ selectedId: B });

    expect(html).toContain("L+ (x 0, y 0 to 20)");
    expect(html).toContain("0V (x 12, y 2 to 22)");
    const selected = html.slice(html.indexOf("0V (x 12"));
    expect(html.slice(0, html.indexOf("0V (x 12"))).toContain(
      'aria-pressed="false"',
    );
    expect(html).toContain('aria-pressed="true"');
    expect(selected.length).toBeGreaterThan(0);
  });

  it("shows the selected rail's four properties with their committed values", () => {
    const html = render({ selectedId: A });

    for (const [label, value] of [
      ["Rail name", "L+"],
      ["Rail x", "0"],
      ["Rail top y", "0"],
      ["Rail bottom y", "20"],
    ]) {
      expect(html).toContain(`>${label}</label>`);
      const field = html.slice(html.indexOf(`>${label}</label>`));
      expect(field.slice(0, 300)).toContain(`value="${value}"`);
    }
    expect(html).toContain("Delete rail");
    expect(html).toContain('aria-label="Rail properties"');
  });

  it("shows no properties when no rail is selected", () => {
    const html = render({ selectedId: null });

    expect(html).not.toContain("Rail properties");
    expect(html).not.toContain("Delete rail");
  });

  it("offers a keyboard form to create a rail, with the next default name", () => {
    const html = render();

    expect(html).toContain('aria-label="Create a rail"');
    expect(html).toContain(">New rail name</label>");
    expect(html).toContain(">New rail x</label>");
    expect(html).toContain(">New rail top y</label>");
    expect(html).toContain(">New rail bottom y</label>");
    expect(html).toContain("Create rail");
    // RAIL1 is free (the rails are L+ and 0V).
    const name = html.slice(html.indexOf(">New rail name</label>"));
    expect(name.slice(0, 300)).toContain('value="RAIL1"');

    const taken = render({
      rails: [
        { id: A, name: "RAIL1", start: { x: 0, y: 0 }, end: { x: 0, y: 5 } },
      ],
    });
    const next = taken.slice(taken.indexOf(">New rail name</label>"));
    expect(next.slice(0, 300)).toContain('value="RAIL2"');
  });

  it("is switched off in an assignment, and says how to get it back", () => {
    const html = render({
      selectedId: A,
      unavailable:
        "DC rails are not available in an assignment. Leave assignment mode to edit DC rails.",
    });

    expect(html).toContain("Leave assignment mode to edit DC rails.");
    // Every input and button is disabled except the list that only selects.
    const controls = html.match(/<(input|button)[^>]*>/g) ?? [];
    const editing = controls.filter(
      (tag) =>
        !tag.includes("aria-pressed") &&
        !tag.includes('type="button" class="toolbar-button" aria-pressed'),
    );
    expect(editing.length).toBeGreaterThan(0);
    for (const tag of editing) {
      expect(tag).toContain('disabled=""');
    }
  });

  it("names the affected rail in the Engine's words under its properties", () => {
    const diagnostic: Diagnostic = {
      code: "DuplicateRailName",
      severity: "error",
      localizationKey: "diagnostic.duplicateRailName",
      references: [
        { kind: "rail", railId: A },
        { kind: "rail", railId: B },
      ],
    };

    const html = render({ selectedId: A, diagnostics: [diagnostic] });

    expect(html).toContain('role="alert"');
    expect(html).toContain("DC rail names must be unique");
    // A diagnostic that does not name the selected rail is not shown under it.
    expect(
      render({
        selectedId: A,
        diagnostics: [
          { ...diagnostic, references: [{ kind: "rail", railId: B }] },
        ],
      }),
    ).not.toContain('role="alert"');
  });

  it("shows a name as plain text, never as markup", () => {
    const html = render({
      rails: [
        {
          id: A,
          name: "<img src=x onerror=alert(1)>",
          start: { x: 0, y: 0 },
          end: { x: 0, y: 5 },
        },
      ],
      selectedId: A,
    });

    expect(html).not.toContain("<img");
    expect(html).toContain("&lt;img");
  });
});

describe("rail coordinates typed in a field", () => {
  it.each([
    ["12", 12],
    ["-3", -3],
    [" 7 ", 7],
    ["0", 0],
  ])("reads %j as %i", (text, value) => {
    expect(parseGridInteger(text)).toBe(value);
  });

  it.each(["", "1.5", "1e3", "abc", "--1", "0x10", "+4"])(
    "refuses %j",
    (text) => {
      expect(parseGridInteger(text)).toBeNull();
    },
  );
});
