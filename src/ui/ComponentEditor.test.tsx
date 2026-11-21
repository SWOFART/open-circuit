import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import catalogFixture from "../../tests/golden/stage-1-catalog.json";
import type { ComponentCatalog } from "../generated/ComponentCatalog";
import type { ComponentTypeId } from "../generated/ComponentTypeId";
import { SchematicDocumentStore } from "../store/document";
import { blankSchematic } from "../store/projectWorkflow";
import {
  initialSimulationState,
  type SimulationState,
} from "../store/simulation";
import { applyEditorCommand } from "./editorCommands";
import { ComponentEditor, type ComponentEditorProps } from "./ComponentEditor";
import { Inspector } from "./Inspector";

const catalog = catalogFixture as ComponentCatalog;
const state: SimulationState = { ...initialSimulationState(), catalog };

function drawn(types: readonly ComponentTypeId[]) {
  let next = 0x61;
  const store = new SchematicDocumentStore(
    blankSchematic,
    () => `00000000-0000-4000-8000-0000000000${(next++).toString(16)}`,
  );

  types.forEach((typeId, index) =>
    applyEditorCommand(
      store,
      {
        kind: "placeComponent",
        typeId,
        position: { x: index * 8, y: 0 },
        rotationDeg: 0,
      },
      catalog,
    ),
  );

  return store.getState();
}

function render(
  document: ReturnType<typeof drawn>,
  index: number,
  props: Partial<ComponentEditorProps> = {},
) {
  const component = document.schematic.components[index]!;

  return renderToStaticMarkup(
    <ComponentEditor
      state={state}
      schematic={document.schematic}
      labels={document.labels}
      componentId={component.id}
      position={{ left: 10, top: 20 }}
      enabled
      locked={false}
      onCommit={() => {}}
      onClear={() => {}}
      onClose={() => {}}
      {...props}
    />,
  );
}

describe("the component editor", () => {
  it("is a non-modal dialog named after the part, placed where it was asked to", () => {
    const html = render(drawn(["resistor"]), 0);

    expect(html).toContain('role="dialog"');
    expect(html).toContain('aria-modal="false"');
    expect(html).toContain('aria-label="Edit R1"');
    expect(html).toContain("left:10px");
    expect(html).toContain("top:20px");
    expect(html).toContain(">R1<");
    expect(html).toContain('aria-label="Close the editor of R1"');
    expect(html).toContain("Editing R1");
  });

  it("shows the parameters of the load or consumer from the catalog, with units and range", () => {
    const html = render(drawn(["pmdc_motor"]), 0);

    expect(html).toContain("M1 load torque");
    expect(html).toContain("M1 armature resistance");
    expect(html).toContain("A negative torque opposes positive rotation");
  });

  it("shows the allowed range where the catalog has one", () => {
    expect(render(drawn(["bjt_npn"]), 0)).toMatch(/Allowed range/);
  });

  it("shows the same fields as the inspector for that part", () => {
    const document = drawn(["resistor", "pmdc_motor", "capacitor"]);
    const inspector = renderToStaticMarkup(
      <Inspector
        state={state}
        schematic={document.schematic}
        labels={document.labels}
        onCommit={() => {}}
      />,
    );

    for (const index of [0, 1, 2]) {
      const editor = render(document, index);
      const labels = [...editor.matchAll(/<label[^>]*>([^<]+)<\/label>/g)].map(
        (match) => match[1],
      );

      expect(labels.length).toBeGreaterThan(0);
      for (const label of labels) {
        expect(inspector, label).toContain(`>${label}</label>`);
      }
    }
  });

  it("disables every field and says why for a part the assignment locks", () => {
    const html = render(drawn(["resistor"]), 0, { locked: true });

    expect(html).toContain("R1 is locked by the assignment");
    expect(html).toContain("cannot be edited");
    expect(html).toMatch(/<input[^>]*disabled=""/);
    expect(html).not.toMatch(/<input(?![^>]*disabled)[^>]*type="text"/);
  });

  it("leaves the fields editable otherwise, and disables them while a project operation runs", () => {
    const document = drawn(["resistor"]);

    expect(render(document, 0)).not.toMatch(/<input[^>]*disabled=""/);
    expect(render(document, 0, { enabled: false })).toMatch(
      /<input[^>]*disabled=""/,
    );
  });

  it("renders nothing for a part that is gone, or before the catalog has loaded", () => {
    const document = drawn(["resistor"]);

    expect(
      render(document, 0, {
        componentId: "00000000-0000-4000-8000-00000000ffff",
      }),
    ).toBe("");
    expect(render(document, 0, { state: { ...state, catalog: null } })).toBe(
      "",
    );
  });
});

describe("the designator field", () => {
  const withDesignators = (types: readonly ComponentTypeId[]) => {
    let next = 0x61;
    const store = new SchematicDocumentStore(
      blankSchematic,
      () => `00000000-0000-4000-8000-0000000000${(next++).toString(16)}`,
    );
    types.forEach((typeId, index) =>
      applyEditorCommand(
        store,
        {
          kind: "placeComponent",
          typeId,
          position: { x: index * 8, y: 0 },
          rotationDeg: 0,
        },
        catalog,
        true,
      ),
    );
    return store.getState();
  };

  it("is offered above the parameters with the stored designator in it", () => {
    const document = withDesignators(["contactor"]);
    const html = render(document, 0, { onSetDesignator: () => null });

    expect(html).toContain("Q1 designator");
    expect(html).toContain('value="Q1"');
    expect(html.indexOf("Q1 designator")).toBeLessThan(html.indexOf("Q1 coil"));
  });

  it("is not offered while an assignment is open", () => {
    const document = withDesignators(["contactor"]);

    expect(render(document, 0)).not.toContain("designator");
  });

  it("is disabled while a project operation runs", () => {
    const document = withDesignators(["resistor"]);
    const html = render(document, 0, {
      enabled: false,
      onSetDesignator: () => null,
    });

    expect(html).toMatch(/<input[^>]*placeholder="R1"[^>]*disabled=""/);
  });

  it("is in the inspector for every part, and only when it is offered", () => {
    const document = withDesignators(["resistor", "capacitor"]);
    const inspect = (offered: boolean) =>
      renderToStaticMarkup(
        <Inspector
          state={state}
          schematic={document.schematic}
          labels={document.labels}
          onCommit={() => {}}
          {...(offered ? { onSetDesignator: () => null } : {})}
        />,
      );

    expect(inspect(true)).toContain("R1 designator");
    expect(inspect(true)).toContain("C1 designator");
    expect(inspect(false)).not.toContain("designator");
  });
});
