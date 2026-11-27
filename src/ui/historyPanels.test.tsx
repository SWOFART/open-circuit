import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import type { HistorySummary } from "../store/document";
import { CanvasSearch } from "./CanvasSearch";
import { HistoryPanel } from "./HistoryPanel";
import { historyLabelText } from "./historyLabels";
import schematicFixture from "../../tests/golden/stage-1-divider.schematic.json";
import type { Schematic } from "../generated/Schematic";

// The history panel and the search field of docs/contracts/stage-16-editor-improvements.md
// sections 6 and 7, as markup (there is no browser in the unit tests).

const history = (over: Partial<HistorySummary> = {}): HistorySummary => ({
  past: [],
  future: [],
  dropped: false,
  ...over,
});
const render = (value: HistorySummary, disabled = false) =>
  renderToStaticMarkup(
    <HistoryPanel history={value} disabled={disabled} onGo={() => {}} />,
  );

describe("the text of a history entry", () => {
  it("reads a known label with its values", () => {
    expect(
      historyLabelText({ key: "history.move", values: { count: 3 } }),
    ).toBe("Move 3 objects");
    expect(
      historyLabelText({ key: "history.paste", values: { count: 1 } }),
    ).toBe("Paste 1 object");
    expect(
      historyLabelText({
        key: "history.setParameter",
        values: { part: "R1", parameter: "resistanceOhms" },
      }),
    ).toBe("Set resistanceOhms of R1");
    expect(
      historyLabelText({ key: "history.place", values: { name: "Resistor" } }),
    ).toBe("Place Resistor");
  });

  it("reads no label, or an unknown one, as the generic Edit", () => {
    expect(historyLabelText(null)).toBe("Edit");
    expect(historyLabelText({ key: "history.unheardOf" })).toBe("Edit");
  });
});

describe("the history panel", () => {
  it("says so when there is nothing in the history", () => {
    expect(render(history())).toContain("No edits yet.");
  });

  it("lists the newest first and marks the current entry", () => {
    const markup = render(
      history({
        past: [{ key: "history.drawWire" }, { key: "history.addNote" }],
      }),
    );

    expect(markup.indexOf("Add a note")).toBeLessThan(
      markup.indexOf("Draw a wire"),
    );
    // The newest done entry is the current one.
    expect(markup).toMatch(/aria-current="step"[^>]*>Add a note/);
    expect(markup).not.toContain("(undone)");
  });

  it("shows the undone entries first, dimmed and marked as undone", () => {
    const markup = render(
      history({
        past: [{ key: "history.drawWire" }],
        future: [{ key: "history.addNote" }, { key: "history.addJunction" }],
      }),
    );

    // Newest first: the furthest redo, the next redo, then the done entry.
    expect(markup.indexOf("Add a junction")).toBeLessThan(
      markup.indexOf("Add a note"),
    );
    expect(markup.indexOf("Add a note")).toBeLessThan(
      markup.indexOf("Draw a wire"),
    );
    expect(markup.match(/history-undone/g)).toHaveLength(2);
    expect(markup.match(/\(undone\)/g)).toHaveLength(2);
    expect(markup).toMatch(/aria-current="step"[^>]*>Draw a wire/);
  });

  it("reads an entry without a label as Edit", () => {
    expect(render(history({ past: [null] }))).toContain(">Edit<");
  });

  it("says when older entries were dropped, and for how many are kept", () => {
    const markup = render(history({ past: [null], dropped: true }));

    expect(markup).toContain("Older entries were dropped");
    expect(markup).toContain("1000");
    expect(render(history({ past: [null] }))).not.toContain("dropped");
  });

  it("disables every entry while a project operation runs", () => {
    const markup = render(history({ past: [null, null] }), true);

    expect(markup.match(/disabled=""/g)).toHaveLength(2);
  });
});

describe("the search field", () => {
  const schematic = schematicFixture as Schematic;
  const markup = renderToStaticMarkup(
    <CanvasSearch
      schematic={schematic}
      labels={new Map()}
      typeName={(typeId) => typeId}
      onChoose={() => {}}
      onClose={() => {}}
    />,
  );

  it("is a labelled combobox over a listbox of results, and shows nothing for an empty query", () => {
    expect(markup).toContain('role="search"');
    expect(markup).toContain('role="combobox"');
    expect(markup).toContain('aria-label="Search the drawing"');
    expect(markup).toContain('role="listbox"');
    expect(markup).not.toContain('role="option"');
    expect(markup).not.toContain("Nothing found");
  });
});
